//! 主业务：带工具的 REPL。可引用 `repl`、`tools` 与 `provider`。

use std::{error::Error, io};

use crate::{
    config::Config,
    provider::{ChatProvider, ToolSpec, openai::Provider},
    repl::{self, Session},
    tools::Tools,
};

const MAX_TOOL_ROUNDS: usize = 5;

pub(crate) async fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;
    let mut agent = Agent {
        chat: Provider::new(&config),
        tools: Tools::new(config.tools_enabled)?,
    };
    repl::run(&mut agent).await
}

struct Agent {
    chat: Provider,
    tools: Tools,
}

impl Session for Agent {
    async fn handle_message<F>(
        &mut self,
        message: &str,
        mut on_delta: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        run_tool_loop(&mut self.chat, &mut self.tools, message, &mut on_delta).await
    }

    fn reset(&mut self) {
        self.chat.reset();
        self.tools.reset();
    }
}

fn tool_specs(tools: &Tools) -> Vec<ToolSpec> {
    tools
        .specs()
        .into_iter()
        .map(|spec| ToolSpec {
            name: spec.name.to_owned(),
            description: spec.description.to_owned(),
            parameters: spec.parameters,
        })
        .collect()
}

async fn run_tool_loop<P, F>(
    chat: &mut P,
    tools: &mut Tools,
    message: &str,
    on_delta: &mut F,
) -> Result<(), Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    chat.begin_turn(message);
    let specs = tool_specs(tools);
    let mut used_tools = false;
    for round in 0..=MAX_TOOL_ROUNDS {
        let step = match chat.complete_step(&specs, &mut *on_delta).await {
            Ok(step) => step,
            Err(error) => {
                if used_tools {
                    chat.commit_turn();
                } else {
                    chat.rollback_turn();
                }
                return Err(error);
            }
        };
        if step.calls.is_empty() {
            chat.finish_turn(step.text);
            return Ok(());
        }
        if round == MAX_TOOL_ROUNDS {
            on_delta("\n[工具调用轮次过多，已停止]\n")?;
            chat.commit_turn();
            return Ok(());
        }
        used_tools = true;
        let mut results = Vec::new();
        for call in step.calls {
            on_delta(&format!("\n[调用工具 {}]\n", call.name))?;
            let output = tools.execute(&call.name, &call.args).await;
            results.push((call, output));
        }
        chat.apply_tool_results(step.text, &results);
    }
    unreachable!("工具轮次循环已覆盖所有出口")
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, error::Error, io};

    use super::{MAX_TOOL_ROUNDS, run_tool_loop};
    use crate::{
        provider::{ChatProvider, ModelStep, ToolCall, ToolSpec},
        tools::Tools,
    };

    struct FakeProvider {
        steps: VecDeque<Result<ModelStep, String>>,
        events: Vec<String>,
    }

    impl ChatProvider for FakeProvider {
        fn begin_turn(&mut self, user_input: &str) {
            self.events.push(format!("begin:{user_input}"));
        }

        async fn complete_step<F>(
            &mut self,
            _tools: &[ToolSpec],
            _on_delta: F,
        ) -> Result<ModelStep, Box<dyn Error>>
        where
            F: FnMut(&str) -> io::Result<()>,
        {
            self.events.push("complete".to_owned());
            match self.steps.pop_front() {
                Some(Ok(step)) => Ok(step),
                Some(Err(error)) => Err(error.into()),
                None => panic!("没有更多模型步骤"),
            }
        }

        fn apply_tool_results(&mut self, _text: String, results: &[(ToolCall, String)]) {
            self.events.push(format!("apply:{}", results.len()));
        }

        fn finish_turn(&mut self, text: String) {
            self.events.push(format!("finish:{text}"));
        }

        fn commit_turn(&mut self) {
            self.events.push("commit".to_owned());
        }

        fn rollback_turn(&mut self) {
            self.events.push("rollback".to_owned());
        }

        fn reset(&mut self) {
            self.events.push("reset".to_owned());
        }
    }

    fn tool_step() -> ModelStep {
        ModelStep {
            text: String::new(),
            calls: vec![ToolCall {
                id: "call_1".to_owned(),
                name: "get_current_time".to_owned(),
                args: "{}".to_owned(),
            }],
        }
    }

    fn text_step(text: &str) -> ModelStep {
        ModelStep {
            text: text.to_owned(),
            calls: Vec::new(),
        }
    }

    async fn run(chat: &mut FakeProvider) -> Result<String, Box<dyn Error>> {
        let mut tools = Tools::new(true).expect("工作目录存在");
        let mut printed = String::new();
        run_tool_loop(chat, &mut tools, "现在几点", &mut |delta| {
            printed.push_str(delta);
            Ok(())
        })
        .await?;
        Ok(printed)
    }

    #[tokio::test]
    async fn text_only_finishes_without_commit_or_rollback() {
        let mut chat = FakeProvider {
            steps: VecDeque::from([Ok(text_step("hi"))]),
            events: Vec::new(),
        };
        let printed = run(&mut chat).await.expect("纯文本应成功");
        assert_eq!(printed, "");
        assert_eq!(chat.events, ["begin:现在几点", "complete", "finish:hi"]);
    }

    #[tokio::test]
    async fn one_tool_round_then_text() {
        let mut chat = FakeProvider {
            steps: VecDeque::from([Ok(tool_step()), Ok(text_step("晚上八点"))]),
            events: Vec::new(),
        };
        let printed = run(&mut chat).await.expect("一轮工具应成功");
        assert!(printed.contains("[调用工具 get_current_time]"));
        assert_eq!(
            chat.events,
            [
                "begin:现在几点",
                "complete",
                "apply:1",
                "complete",
                "finish:晚上八点"
            ]
        );
    }

    #[tokio::test]
    async fn max_tool_rounds_commits_without_applying_last_calls() {
        let mut steps = VecDeque::new();
        for _ in 0..=MAX_TOOL_ROUNDS {
            steps.push_back(Ok(tool_step()));
        }
        let mut chat = FakeProvider {
            steps,
            events: Vec::new(),
        };
        let printed = run(&mut chat).await.expect("轮次用尽应停止");
        assert!(printed.contains("工具调用轮次过多"));
        let apply_count = chat
            .events
            .iter()
            .filter(|event| *event == "apply:1")
            .count();
        assert_eq!(apply_count, MAX_TOOL_ROUNDS);
        assert_eq!(chat.events.last().map(String::as_str), Some("commit"));
    }

    #[tokio::test]
    async fn error_before_tools_rolls_back() {
        let mut chat = FakeProvider {
            steps: VecDeque::from([Err("boom".to_owned())]),
            events: Vec::new(),
        };
        let mut tools = Tools::new(true).expect("工作目录存在");
        let error = run_tool_loop(&mut chat, &mut tools, "hi", &mut |_| Ok(()))
            .await
            .expect_err("应失败");
        assert!(error.to_string().contains("boom"));
        assert_eq!(chat.events, ["begin:hi", "complete", "rollback"]);
    }

    #[tokio::test]
    async fn error_after_tools_commits() {
        let mut chat = FakeProvider {
            steps: VecDeque::from([Ok(tool_step()), Err("boom".to_owned())]),
            events: Vec::new(),
        };
        let mut tools = Tools::new(true).expect("工作目录存在");
        run_tool_loop(&mut chat, &mut tools, "hi", &mut |_| Ok(()))
            .await
            .expect_err("应失败");
        assert_eq!(
            chat.events,
            ["begin:hi", "complete", "apply:1", "complete", "commit"]
        );
    }
}
