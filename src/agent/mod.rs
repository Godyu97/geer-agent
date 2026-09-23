//! 主业务：带工具的 REPL。可引用 `repl`、`tools` 与 `provider`。

use std::{error::Error, io};

use crate::{
    config::Config,
    prompt::{self, Prompt},
    provider::{ChatProvider, ToolSpec, openai::Provider},
    repl::{self, Session},
    tools::Tools,
};

const MAX_TOOL_ROUNDS: usize = 5;

pub(crate) async fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;
    let system_prompt = prompt::load(&config.bash_bin).await?;
    let mut prompt = Prompt::new(config.api, system_prompt);
    let mut agent = Agent {
        chat: Provider::new(&config),
        tools: Tools::new(config.tools_enabled, config.bash_bin.clone())?,
    };
    repl::run(&mut agent, &mut prompt).await
}

struct Agent {
    chat: Provider,
    tools: Tools,
}

impl Session for Agent {
    async fn handle_message<F>(
        &mut self,
        prompt: &mut Prompt,
        mut on_delta: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        run_tool_loop(&mut self.chat, &mut self.tools, prompt, &mut on_delta).await
    }

    fn reset(&mut self) {
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
    prompt: &mut Prompt,
    on_delta: &mut F,
) -> Result<(), Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let specs = tool_specs(tools);
    let mut used_tools = false;
    for round in 0..=MAX_TOOL_ROUNDS {
        let step = match chat
            .complete_step(prompt.messages(), &specs, &mut *on_delta)
            .await
        {
            Ok(step) => step,
            Err(error) => {
                if used_tools {
                    prompt.commit_turn();
                } else {
                    prompt.rollback_turn();
                }
                return Err(error);
            }
        };
        if step.calls.is_empty() {
            prompt.finish_turn(step);
            return Ok(());
        }
        if round == MAX_TOOL_ROUNDS {
            on_delta("\n[工具调用轮次过多，已停止]\n")?;
            prompt.commit_turn();
            return Ok(());
        }
        used_tools = true;
        let mut results = Vec::new();
        for call in &step.calls {
            on_delta(&format!("\n[调用工具 {}]\n", call.name))?;
            let output = tools.execute(&call.name, &call.args).await;
            results.push((call.clone(), output));
        }
        prompt.apply_tool_results(step, &results);
    }
    unreachable!("工具轮次循环已覆盖所有出口")
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, error::Error, io};

    use super::{MAX_TOOL_ROUNDS, run_tool_loop};
    use crate::{
        config::OpenAiApi,
        prompt::Prompt,
        provider::{ChatProvider, Messages, ModelStep, ToolCall, ToolSpec},
        tools::Tools,
    };

    struct FakeProvider {
        steps: VecDeque<Result<ModelStep, String>>,
        snapshots: Vec<serde_json::Value>,
    }

    impl ChatProvider for FakeProvider {
        async fn complete_step<F>(
            &mut self,
            messages: Messages,
            _tools: &[ToolSpec],
            _on_delta: F,
        ) -> Result<ModelStep, Box<dyn Error>>
        where
            F: FnMut(&str) -> io::Result<()>,
        {
            let Messages::Chat(messages) = messages else {
                panic!("测试只使用 Chat 消息");
            };
            self.snapshots.push(serde_json::to_value(messages)?);
            match self.steps.pop_front() {
                Some(Ok(step)) => Ok(step),
                Some(Err(error)) => Err(error.into()),
                None => panic!("没有更多模型步骤"),
            }
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
            output: Vec::new(),
        }
    }

    fn text_step(text: &str) -> ModelStep {
        ModelStep {
            text: text.to_owned(),
            calls: Vec::new(),
            output: Vec::new(),
        }
    }

    fn prompt() -> Prompt {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".to_owned());
        prompt.begin_turn("现在几点");
        prompt
    }

    async fn run(chat: &mut FakeProvider, prompt: &mut Prompt) -> Result<String, Box<dyn Error>> {
        let mut tools = Tools::new(true, "bash".into()).expect("工作目录存在");
        let mut printed = String::new();
        run_tool_loop(chat, &mut tools, prompt, &mut |delta| {
            printed.push_str(delta);
            Ok(())
        })
        .await?;
        Ok(printed)
    }

    fn fake(steps: Vec<Result<ModelStep, String>>) -> FakeProvider {
        FakeProvider {
            steps: steps.into(),
            snapshots: Vec::new(),
        }
    }

    #[tokio::test]
    async fn text_only_finishes_and_next_turn_sees_answer() {
        let mut chat = fake(vec![Ok(text_step("hi"))]);
        let mut prompt = prompt();
        run(&mut chat, &mut prompt).await.expect("纯文本应成功");
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 3);
        assert_eq!(chat.snapshots[0].as_array().expect("消息").len(), 2);
    }

    #[tokio::test]
    async fn one_tool_round_then_text() {
        let mut chat = fake(vec![Ok(tool_step()), Ok(text_step("晚上八点"))]);
        let mut prompt = prompt();
        let printed = run(&mut chat, &mut prompt).await.expect("一轮工具应成功");
        assert!(printed.contains("[调用工具 get_current_time]"));
        let messages = chat.snapshots[1].as_array().expect("第二次请求");
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[3]["tool_call_id"], "call_1");
    }

    #[tokio::test]
    async fn max_tool_rounds_commits_without_applying_last_calls() {
        let mut chat = fake((0..=MAX_TOOL_ROUNDS).map(|_| Ok(tool_step())).collect());
        let mut prompt = prompt();
        let printed = run(&mut chat, &mut prompt).await.expect("轮次用尽应停止");
        assert!(printed.contains("工具调用轮次过多"));
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 2 + MAX_TOOL_ROUNDS * 2);
    }

    #[tokio::test]
    async fn error_before_tools_rolls_back() {
        let mut chat = fake(vec![Err("boom".to_owned())]);
        let mut prompt = prompt();
        run(&mut chat, &mut prompt).await.expect_err("应失败");
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn error_after_tools_keeps_pairs() {
        let mut chat = fake(vec![Ok(tool_step()), Err("boom".to_owned())]);
        let mut prompt = prompt();
        run(&mut chat, &mut prompt).await.expect_err("应失败");
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 4);
    }
}
