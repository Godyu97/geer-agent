//! 主业务：带工具的 REPL。可引用 `repl`、`tools` 与 `provider`。

mod guard;

use serde_json::{Value, json};
use std::{
    error::Error,
    io,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, timeout_at};

use crate::{
    config::{Config, DEFAULT_RESOURCE_LIMITS, ResourceLimits},
    prompt::{self, Prompt},
    provider::{ChatProvider, TokenUsage, ToolSpec, openai::Provider},
    repl::{self, Session},
    tools::Tools,
};
use guard::{LoopGuard, StopReason, call_fingerprint, result_fingerprint};

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(0);

const DEFAULT_AGENT_BUDGET: AgentBudget = AgentBudget {
    soft_turn_limit: 12,
    max_turns: 30,
    max_tool_calls: 100,
    limits: DEFAULT_RESOURCE_LIMITS,
};
const SOFT_BUDGET_HINT: &str = "你已经使用了较多 Agent Turn。请检查剩余工作，优先完成用户请求，避免不必要的探索；只在完成或验证确有需要时继续调用工具，并在任务完成后立即给出最终回答。";
const FINALIZATION_FALLBACK: &str =
    "\n[工具执行预算已耗尽，无法生成有效的最终回答；部分工作可能尚未完成或验证。]\n";

#[derive(Clone, Copy, Debug)]
struct AgentBudget {
    soft_turn_limit: usize,
    max_turns: usize,
    max_tool_calls: usize,
    limits: ResourceLimits,
}

#[derive(Debug, Default, PartialEq)]
struct AgentMetrics {
    turns: usize,
    tool_calls: usize,
    input_tokens: u64,
    output_tokens: u64,
    estimated_cost_usd: Option<f64>,
    usage_complete: bool,
    duration_ms: u128,
    termination_reason: Option<TerminationReason>,
    tool_errors: usize,
    new_files_read: usize,
    files_modified: usize,
    unique_tool_results: usize,
    repeated_tool_results: usize,
    consecutive_no_progress_turns: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminationReason {
    Completed,
    MaxTurns,
    MaxToolCalls,
    MaxDuration,
    MaxTokens,
    MaxCost,
    UsageUnavailable,
    RepeatedToolLoop,
    ConsecutiveErrors,
    NoProgress,
    ProviderError,
}

impl TerminationReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::MaxTurns => "max_turns",
            Self::MaxToolCalls => "max_tool_calls",
            Self::MaxDuration => "max_duration",
            Self::MaxTokens => "max_tokens",
            Self::MaxCost => "max_cost",
            Self::UsageUnavailable => "usage_unavailable",
            Self::RepeatedToolLoop => "repeated_tool_loop",
            Self::ConsecutiveErrors => "consecutive_errors",
            Self::NoProgress => "no_progress",
            Self::ProviderError => "provider_error",
        }
    }
}

impl From<StopReason> for TerminationReason {
    fn from(reason: StopReason) -> Self {
        match reason {
            StopReason::RepeatedToolLoop => Self::RepeatedToolLoop,
            StopReason::ConsecutiveErrors => Self::ConsecutiveErrors,
            StopReason::NoProgress => Self::NoProgress,
        }
    }
}

#[derive(Debug)]
struct AgentRuntime {
    budget: AgentBudget,
    metrics: AgentMetrics,
    guard: LoopGuard,
    started_at: Instant,
    run_id: String,
    model: String,
}

impl AgentRuntime {
    fn new(budget: AgentBudget, model: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos());
        Self {
            budget,
            metrics: AgentMetrics {
                usage_complete: true,
                ..AgentMetrics::default()
            },
            guard: LoopGuard::default(),
            started_at: Instant::now(),
            run_id: format!(
                "{}-{now}-{}",
                std::process::id(),
                NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed)
            ),
            model: model.to_owned(),
        }
    }

    fn record_turn(&mut self) {
        debug_assert!(self.metrics.turns < self.budget.max_turns);
        self.metrics.turns += 1;
    }

    fn remaining_turns(&self) -> usize {
        self.budget.max_turns.saturating_sub(self.metrics.turns)
    }

    fn soft_limit_reached(&self) -> bool {
        self.metrics.turns >= self.guard.soft_turn_limit(self.budget.soft_turn_limit)
    }

    fn remaining_tool_calls(&self) -> usize {
        self.budget
            .max_tool_calls
            .saturating_sub(self.metrics.tool_calls)
    }

    fn try_record_tool_calls(&mut self, count: usize) -> bool {
        if count > self.remaining_tool_calls() {
            return false;
        }
        self.metrics.tool_calls += count;
        true
    }

    #[cfg(test)]
    fn should_finalize(&self) -> bool {
        self.limit_reason().is_some()
    }

    fn limit_reason(&self) -> Option<TerminationReason> {
        if let Some(reason) = self.resource_reason() {
            return Some(reason);
        }
        if self.remaining_turns() <= 1 {
            return Some(TerminationReason::MaxTurns);
        }
        if self.remaining_tool_calls() == 0 {
            return Some(TerminationReason::MaxToolCalls);
        }
        None
    }

    fn resource_reason(&self) -> Option<TerminationReason> {
        if self.started_at.elapsed() >= self.budget.limits.max_duration {
            return Some(TerminationReason::MaxDuration);
        }
        if self
            .budget
            .limits
            .max_input_tokens
            .is_some_and(|max| self.metrics.input_tokens >= max)
            || self
                .budget
                .limits
                .max_output_tokens
                .is_some_and(|max| self.metrics.output_tokens >= max)
        {
            return Some(TerminationReason::MaxTokens);
        }
        if self.budget.limits.max_cost_usd.is_some_and(|max| {
            self.metrics
                .estimated_cost_usd
                .is_some_and(|cost| cost >= max)
        }) {
            return Some(TerminationReason::MaxCost);
        }
        None
    }

    fn record_usage(&mut self, usage: Option<TokenUsage>) -> Option<TerminationReason> {
        match usage {
            Some(usage) => {
                self.metrics.input_tokens = self.metrics.input_tokens.saturating_add(usage.input);
                self.metrics.output_tokens =
                    self.metrics.output_tokens.saturating_add(usage.output);
                if self.metrics.usage_complete
                    && let (Some(input_rate), Some(output_rate)) = (
                        self.budget.limits.input_usd_per_million,
                        self.budget.limits.output_usd_per_million,
                    )
                {
                    let cost = (usage.input as f64 * input_rate
                        + usage.output as f64 * output_rate)
                        / 1_000_000.0;
                    self.metrics.estimated_cost_usd =
                        Some(self.metrics.estimated_cost_usd.unwrap_or(0.0) + cost);
                }
            }
            None => {
                self.metrics.usage_complete = false;
                self.metrics.estimated_cost_usd = None;
                if self.budget.limits.requires_usage() {
                    return Some(TerminationReason::UsageUnavailable);
                }
            }
        }
        self.resource_reason()
    }

    fn finish(&mut self, reason: TerminationReason) -> AgentMetrics {
        self.metrics.duration_ms = self.started_at.elapsed().as_millis();
        self.metrics.termination_reason = Some(reason);
        self.metrics.tool_errors = self.guard.metrics.tool_errors;
        self.metrics.new_files_read = self.guard.metrics.new_files_read;
        self.metrics.files_modified = self.guard.metrics.files_modified;
        self.metrics.unique_tool_results = self.guard.metrics.unique_tool_results;
        self.metrics.repeated_tool_results = self.guard.metrics.repeated_tool_results;
        self.metrics.consecutive_no_progress_turns =
            self.guard.metrics.consecutive_no_progress_turns;
        let metrics = std::mem::take(&mut self.metrics);
        eprintln!("{}", run_record(&self.run_id, &self.model, &metrics));
        metrics
    }

    fn budget_hint(&self) -> Option<String> {
        let remaining = self.remaining_turns();
        if remaining <= 3 {
            return Some(format!(
                "本次请求还剩 {remaining} 个 Agent Turn。请只执行完成任务所必需的操作，避免可选探索，并尽快给出最终回答。"
            ));
        }
        self.soft_limit_reached()
            .then(|| SOFT_BUDGET_HINT.to_owned())
    }
}

fn run_record(run_id: &str, model: &str, metrics: &AgentMetrics) -> Value {
    json!({
        "event": "agent_run",
        "agentRunId": run_id,
        "model": model,
        "turns": metrics.turns,
        "toolCalls": metrics.tool_calls,
        "inputTokens": metrics.input_tokens,
        "outputTokens": metrics.output_tokens,
        "usageComplete": metrics.usage_complete,
        "estimatedCostUsd": metrics.estimated_cost_usd,
        "durationMs": metrics.duration_ms,
        "toolErrors": metrics.tool_errors,
        "newFilesRead": metrics.new_files_read,
        "filesModified": metrics.files_modified,
        "uniqueToolResults": metrics.unique_tool_results,
        "repeatedToolResults": metrics.repeated_tool_results,
        "consecutiveNoProgressTurns": metrics.consecutive_no_progress_turns,
        "terminationReason": metrics.termination_reason.map(TerminationReason::as_str),
    })
}

fn tool_record(
    run_id: &str,
    turn: usize,
    call: &crate::provider::ToolCall,
    execution: &crate::tools::ToolExecution,
) -> Value {
    json!({
        "event": "tool_call",
        "agentRunId": run_id,
        "turn": turn,
        "tool": call.name,
        "durationMs": execution.duration.as_millis(),
        "success": execution.output.success,
        "callFingerprint": format!("{:016x}", call_fingerprint(call)),
        "resultFingerprint": format!("{:016x}", result_fingerprint(&execution.output.text)),
    })
}

fn finalization_instruction(skipped_tool_batch: bool, reason: TerminationReason) -> String {
    let skipped = if skipped_tool_batch {
        if reason == TerminationReason::MaxToolCalls {
            "模型刚才请求的工具批次因会超过 Tool Call 预算而没有执行。"
        } else {
            "模型刚才请求的工具批次因资源预算或用量缺失而没有执行。"
        }
    } else {
        ""
    };
    format!(
        "工具执行已不可用。这是本次请求的最后一个 Agent Turn。不要请求或尝试调用任何工具。{skipped}请仅使用上下文中已有的信息给出最佳最终回答，并明确说明任何尚未完成或未经验证的工作。"
    )
}

pub(crate) async fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;
    let system_prompt = prompt::load(&config.bash_bin).await?;
    let mut prompt = Prompt::new(config.api, system_prompt);
    let mut agent = Agent {
        chat: Provider::new(&config),
        tools: Tools::new(config.tools_enabled, config.bash_bin.clone())?,
        budget: AgentBudget {
            limits: config.limits,
            ..DEFAULT_AGENT_BUDGET
        },
        model: config.model.clone(),
    };
    repl::run(&mut agent, &mut prompt).await
}

struct Agent {
    chat: Provider,
    tools: Tools,
    budget: AgentBudget,
    model: String,
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
        run_tool_loop_with_budget(
            &mut self.chat,
            &mut self.tools,
            prompt,
            &mut on_delta,
            self.budget,
            &self.model,
        )
        .await
        .map(|_| ())
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

async fn run_tool_loop_with_budget<P, F>(
    chat: &mut P,
    tools: &mut Tools,
    prompt: &mut Prompt,
    on_delta: &mut F,
    budget: AgentBudget,
    model: &str,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let specs = tool_specs(tools);
    let mut runtime = AgentRuntime::new(budget, model);
    let mut used_tools = false;

    loop {
        if let Some(reason) = runtime.limit_reason() {
            return finalize_without_tools(chat, prompt, on_delta, &mut runtime, false, reason)
                .await;
        }

        let mut hints = Vec::new();
        if let Some(hint) = runtime.budget_hint() {
            hints.push(hint);
        }
        if let Some(hint) = runtime.guard.hint() {
            hints.push(hint.to_owned());
        }
        let hint = (!hints.is_empty()).then(|| hints.join("\n\n"));
        let messages = match hint.as_deref() {
            Some(instruction) => prompt.messages_with_runtime_instruction(Some(instruction)),
            None => prompt.messages(),
        };
        let deadline = runtime.started_at + budget.limits.max_duration;
        let step = match timeout_at(
            deadline,
            chat.complete_step(messages, &specs, &mut *on_delta),
        )
        .await
        {
            Ok(Ok(step)) => {
                runtime.record_turn();
                step
            }
            Err(_) => {
                return finalize_without_tools(
                    chat,
                    prompt,
                    on_delta,
                    &mut runtime,
                    false,
                    TerminationReason::MaxDuration,
                )
                .await;
            }
            Ok(Err(error)) => {
                if used_tools {
                    prompt.commit_turn();
                } else {
                    prompt.rollback_turn();
                }
                runtime.finish(TerminationReason::ProviderError);
                return Err(error);
            }
        };
        if step.calls.is_empty() {
            runtime.record_usage(step.usage);
            prompt.finish_turn(step);
            return Ok(runtime.finish(TerminationReason::Completed));
        }

        if let Some(reason) = runtime.record_usage(step.usage) {
            return finalize_without_tools(chat, prompt, on_delta, &mut runtime, true, reason)
                .await;
        }

        if !runtime.try_record_tool_calls(step.calls.len()) {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                &mut runtime,
                true,
                TerminationReason::MaxToolCalls,
            )
            .await;
        }

        used_tools = true;
        for call in &step.calls {
            on_delta(&format!("\n[调用工具 {}]\n", call.name))?;
        }
        let calls: Vec<_> = step
            .calls
            .iter()
            .map(|call| (call.name.as_str(), call.args.as_str()))
            .collect();
        let executions = tools.execute_batch(&calls).await;
        for (call, execution) in step.calls.iter().zip(&executions) {
            eprintln!(
                "{}",
                tool_record(&runtime.run_id, runtime.metrics.turns, call, execution)
            );
        }
        let outputs: Vec<_> = executions
            .into_iter()
            .map(|execution| execution.output)
            .collect();
        let stop = runtime.guard.observe_turn(&step.calls, &outputs);
        let results: Vec<_> = step
            .calls
            .iter()
            .cloned()
            .zip(outputs.into_iter().map(|output| output.text))
            .collect();
        prompt.apply_tool_results(step, &results);
        if let Some(reason) = stop {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                &mut runtime,
                false,
                reason.into(),
            )
            .await;
        }
    }
}

async fn finalize_without_tools<P, F>(
    chat: &mut P,
    prompt: &mut Prompt,
    on_delta: &mut F,
    runtime: &mut AgentRuntime,
    skipped_tool_batch: bool,
    reason: TerminationReason,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let mut instruction = finalization_instruction(skipped_tool_batch, reason);
    instruction.push_str(&format!(" 停止原因：{}。", reason.as_str()));
    let step = chat
        .complete_step(
            prompt.messages_with_runtime_instruction(Some(&instruction)),
            &[],
            &mut *on_delta,
        )
        .await;

    match step {
        Ok(step) => {
            runtime.record_turn();
            runtime.record_usage(step.usage);
            // provider 已分别校验两种协议的最终文本；Responses 的文本保存在 output 而非 text。
            if step.calls.is_empty() {
                prompt.finish_turn(step);
            } else {
                prompt.commit_turn();
                on_delta(FINALIZATION_FALLBACK)?;
            }
        }
        Err(_) => {
            prompt.commit_turn();
            on_delta(FINALIZATION_FALLBACK)?;
        }
    }

    Ok(runtime.finish(reason))
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, error::Error, fs, io, time::SystemTime};

    use super::{
        AgentBudget, AgentMetrics, AgentRuntime, DEFAULT_AGENT_BUDGET, FINALIZATION_FALLBACK,
        SOFT_BUDGET_HINT, TerminationReason, run_record, run_tool_loop_with_budget, tool_record,
    };
    use crate::{
        config::{OpenAiApi, ResourceLimits},
        prompt::Prompt,
        provider::{ChatProvider, Messages, ModelStep, TokenUsage, ToolCall, ToolSpec},
        tools::{ToolExecution, ToolOutput, Tools},
    };

    struct FakeProvider {
        steps: VecDeque<Result<ModelStep, String>>,
        snapshots: Vec<serde_json::Value>,
        tool_counts: Vec<usize>,
    }

    impl ChatProvider for FakeProvider {
        async fn complete_step<F>(
            &mut self,
            messages: Messages,
            tools: &[ToolSpec],
            _on_delta: F,
        ) -> Result<ModelStep, Box<dyn Error>>
        where
            F: FnMut(&str) -> io::Result<()>,
        {
            let Messages::Chat(messages) = messages else {
                panic!("测试只使用 Chat 消息");
            };
            self.snapshots.push(serde_json::to_value(messages)?);
            self.tool_counts.push(tools.len());
            match self.steps.pop_front() {
                Some(Ok(step)) => Ok(step),
                Some(Err(error)) => Err(error.into()),
                None => panic!("没有更多模型步骤"),
            }
        }
    }

    fn tool_step() -> ModelStep {
        tool_step_with_calls(1, 1)
    }

    fn tool_step_with_calls(count: usize, start: usize) -> ModelStep {
        ModelStep {
            text: String::new(),
            calls: (start..start + count)
                .map(|index| ToolCall {
                    id: format!("call_{index}"),
                    name: "get_current_time".to_owned(),
                    args: "{}".to_owned(),
                })
                .collect(),
            output: Vec::new(),
            usage: None,
        }
    }

    fn tool_steps(count: usize) -> Vec<Result<ModelStep, String>> {
        (1..=count)
            .map(|index| Ok(tool_step_with_calls(1, index)))
            .collect()
    }

    fn named_step(name: &str, args: &str, id: usize) -> ModelStep {
        ModelStep {
            text: String::new(),
            calls: vec![ToolCall {
                id: format!("call_{id}"),
                name: name.to_owned(),
                args: args.to_owned(),
            }],
            output: Vec::new(),
            usage: None,
        }
    }

    fn text_step(text: &str) -> ModelStep {
        ModelStep {
            text: text.to_owned(),
            calls: Vec::new(),
            output: Vec::new(),
            usage: None,
        }
    }

    fn prompt() -> Prompt {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".to_owned());
        prompt.begin_turn("现在几点");
        prompt
    }

    async fn run(chat: &mut FakeProvider, prompt: &mut Prompt) -> Result<String, Box<dyn Error>> {
        run_with_budget(chat, prompt, DEFAULT_AGENT_BUDGET)
            .await
            .map(|(printed, _)| printed)
    }

    async fn run_with_budget(
        chat: &mut FakeProvider,
        prompt: &mut Prompt,
        budget: AgentBudget,
    ) -> Result<(String, AgentMetrics), Box<dyn Error>> {
        let mut tools = Tools::new(true, "bash".into()).expect("工作目录存在");
        let mut printed = String::new();
        let metrics = run_tool_loop_with_budget(
            chat,
            &mut tools,
            prompt,
            &mut |delta| {
                printed.push_str(delta);
                Ok(())
            },
            budget,
            "test-model",
        )
        .await?;
        Ok((printed, metrics))
    }

    fn fake(steps: Vec<Result<ModelStep, String>>) -> FakeProvider {
        FakeProvider {
            steps: steps.into(),
            snapshots: Vec::new(),
            tool_counts: Vec::new(),
        }
    }

    #[test]
    fn runtime_tracks_turn_boundaries() {
        let cases = [
            (11, 19, false, false),
            (12, 18, true, false),
            (27, 3, true, false),
            (28, 2, true, false),
            (29, 1, true, true),
            (30, 0, true, true),
        ];

        for (turns, remaining, soft, finalize) in cases {
            let mut runtime = AgentRuntime::new(DEFAULT_AGENT_BUDGET, "test-model");
            runtime.metrics.turns = turns;
            assert_eq!(runtime.remaining_turns(), remaining, "turns={turns}");
            assert_eq!(runtime.soft_limit_reached(), soft, "turns={turns}");
            assert_eq!(runtime.should_finalize(), finalize, "turns={turns}");
        }
    }

    #[test]
    fn runtime_tracks_tool_call_boundaries_atomically() {
        let mut runtime = AgentRuntime::new(DEFAULT_AGENT_BUDGET, "test-model");
        runtime.metrics.tool_calls = 99;

        assert_eq!(runtime.remaining_tool_calls(), 1);
        assert!(!runtime.try_record_tool_calls(2));
        assert_eq!(runtime.metrics.tool_calls, 99);
        assert!(runtime.try_record_tool_calls(1));
        assert_eq!(runtime.metrics.tool_calls, 100);
        assert!(runtime.should_finalize());
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
    async fn one_turn_counts_each_tool_call() {
        let mut chat = fake(vec![Ok(tool_step_with_calls(3, 1)), Ok(text_step("done"))]);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("多调用应成功");
        assert_eq!(metrics.turns, 2);
        assert_eq!(metrics.tool_calls, 3);
        assert_eq!(printed.matches("[调用工具 get_current_time]").count(), 3);
    }

    #[tokio::test]
    async fn parallel_reads_keep_result_order_around_write() {
        let dir = std::env::temp_dir().join(format!(
            "geer-agent-agent-batch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("时钟")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("创建目录");
        let first = dir.join("a.txt");
        let second = dir.join("b.txt");
        fs::write(&first, "before").expect("准备文件");
        fs::write(&second, "other").expect("准备文件");
        let path1 = serde_json::to_string(&first.to_string_lossy()).expect("路径 JSON");
        let path2 = serde_json::to_string(&second.to_string_lossy()).expect("路径 JSON");
        let calls = [
            ("read", format!("{{\"path\":{path1}}}")),
            ("read", format!("{{\"path\":{path2}}}")),
            (
                "write",
                format!("{{\"path\":{path1},\"content\":\"after\"}}"),
            ),
            ("read", format!("{{\"path\":{path1}}}")),
        ];
        let step = ModelStep {
            text: String::new(),
            calls: calls
                .iter()
                .enumerate()
                .map(|(index, (name, args))| ToolCall {
                    id: format!("call_{index}"),
                    name: (*name).to_owned(),
                    args: args.clone(),
                })
                .collect(),
            output: Vec::new(),
            usage: None,
        };
        let mut chat = fake(vec![Ok(step), Ok(text_step("done"))]);
        let mut tools = Tools::new(true, "bash".into()).expect("工作目录存在");
        tools.allow_all_for_test();
        let mut prompt = prompt();
        let metrics = run_tool_loop_with_budget(
            &mut chat,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            DEFAULT_AGENT_BUDGET,
            "test-model",
        )
        .await
        .expect("批次成功");
        assert_eq!((metrics.turns, metrics.tool_calls), (2, 4));
        let messages = chat.snapshots[1].as_array().expect("第二轮消息");
        assert_eq!(messages[3]["content"], "before");
        assert_eq!(messages[4]["content"], "other");
        assert!(
            messages[5]["content"]
                .as_str()
                .expect("写入结果")
                .contains("已写入")
        );
        assert_eq!(messages[6]["content"], "after");
        fs::remove_dir_all(dir).expect("清理目录");
    }

    #[tokio::test]
    async fn six_tool_turns_continue_to_final_text() {
        let mut steps = tool_steps(6);
        steps.push(Ok(text_step("done")));
        let mut chat = fake(steps);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("第六轮后应继续");
        assert_eq!(metrics.turns, 7);
        assert_eq!(metrics.tool_calls, 6);
        assert!(!printed.contains("工具调用轮次过多"));
        assert_eq!(chat.snapshots.len(), 7);
    }

    #[tokio::test]
    async fn over_budget_tool_batch_is_not_partially_executed() {
        let mut chat = fake(vec![
            Ok(tool_step_with_calls(99, 1)),
            Ok(tool_step_with_calls(2, 100)),
            Ok(text_step("done")),
        ]);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("超预算批次应转入收敛");
        assert_eq!(metrics.turns, 3);
        assert_eq!(metrics.tool_calls, 99);
        assert_eq!(printed.matches("[调用工具 get_current_time]").count(), 99);
        assert_eq!(chat.tool_counts, vec![5, 5, 0]);
        assert!(
            chat.snapshots[2][0]["content"]
                .as_str()
                .expect("system")
                .contains("工具批次因会超过 Tool Call 预算而没有执行")
        );
    }

    #[tokio::test]
    async fn exact_tool_budget_finalizes_after_complete_batch() {
        let mut chat = fake(vec![
            Ok(tool_step_with_calls(99, 1)),
            Ok(tool_step_with_calls(1, 100)),
            Ok(text_step("done")),
        ]);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("恰好耗尽后应转入收敛");
        assert_eq!(metrics.turns, 3);
        assert_eq!(metrics.tool_calls, 100);
        assert_eq!(printed.matches("[调用工具 get_current_time]").count(), 100);
        assert_eq!(chat.tool_counts, vec![5, 5, 0]);
    }

    #[tokio::test]
    async fn budget_hints_reflect_soft_and_remaining_turns() {
        let mut steps = tool_steps(28);
        steps.push(Ok(text_step("done")));
        let mut chat = fake(steps);
        let mut prompt = prompt();
        run(&mut chat, &mut prompt).await.expect("提示不应中断循环");

        let system = |index: usize| {
            chat.snapshots[index][0]["content"]
                .as_str()
                .expect("system")
        };
        assert!(!system(11).contains(SOFT_BUDGET_HINT));
        assert!(!system(12).contains(SOFT_BUDGET_HINT));
        assert!(system(15).contains(SOFT_BUDGET_HINT));
        assert!(system(27).contains("还剩 3 个 Agent Turn"));
        assert!(system(28).contains("还剩 2 个 Agent Turn"));
    }

    #[tokio::test]
    async fn thirtieth_turn_finalizes_without_tools() {
        let mut steps = tool_steps(29);
        steps.push(Ok(text_step("done")));
        let mut chat = fake(steps);
        let mut prompt = prompt();
        let (_, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("第 30 Turn 应收敛");

        assert_eq!(metrics.turns, 30);
        assert_eq!(metrics.tool_calls, 29);
        assert_eq!(chat.tool_counts.len(), 30);
        assert!(chat.tool_counts[..29].iter().all(|count| *count == 5));
        assert_eq!(chat.tool_counts[29], 0);
        assert!(
            chat.snapshots[29][0]["content"]
                .as_str()
                .expect("system")
                .contains("最后一个 Agent Turn")
        );
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        let messages = serde_json::to_value(messages).expect("消息可序列化");
        assert_eq!(
            messages.as_array().expect("消息").last().expect("最终回答")["content"],
            "done"
        );
    }

    #[tokio::test]
    async fn invalid_finalization_uses_local_fallback_without_executing_call() {
        let budget = AgentBudget {
            soft_turn_limit: 1,
            max_turns: 2,
            max_tool_calls: 100,
            ..DEFAULT_AGENT_BUDGET
        };
        let mut chat = fake(vec![Ok(tool_step()), Ok(tool_step_with_calls(1, 2))]);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, budget)
            .await
            .expect("无效收敛响应应本地降级");

        assert_eq!(metrics.turns, 2);
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(chat.tool_counts, vec![5, 0]);
        assert!(printed.contains(FINALIZATION_FALLBACK.trim()));
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 4, "不得保存没有结果的 Finalization 调用");
    }

    #[tokio::test]
    async fn failed_finalization_uses_local_fallback_and_keeps_pairs() {
        let budget = AgentBudget {
            soft_turn_limit: 1,
            max_turns: 2,
            max_tool_calls: 100,
            ..DEFAULT_AGENT_BUDGET
        };
        let mut chat = fake(vec![Ok(tool_step()), Err("boom".to_owned())]);
        let mut prompt = prompt();
        let (printed, metrics) = run_with_budget(&mut chat, &mut prompt, budget)
            .await
            .expect("收敛请求失败应本地降级");

        assert_eq!(metrics.turns, 1);
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(chat.tool_counts, vec![5, 0]);
        assert!(printed.contains(FINALIZATION_FALLBACK.trim()));
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 4, "已完成调用与结果必须保留");
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

    #[tokio::test]
    async fn repeated_call_warns_then_finalizes_without_tools() {
        let mut steps: Vec<_> = (1..=4)
            .map(|id| Ok(named_step("unknown", "{}", id)))
            .collect();
        steps.push(Ok(text_step("limited")));
        let mut chat = fake(steps);
        let mut prompt = prompt();
        let (_, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("空转应收敛");
        assert_eq!(metrics.tool_calls, 4);
        assert_eq!(
            metrics.termination_reason,
            Some(TerminationReason::RepeatedToolLoop)
        );
        assert_eq!(chat.tool_counts, vec![5, 5, 5, 5, 0]);
        assert!(
            chat.snapshots[3][0]["content"]
                .as_str()
                .expect("提示")
                .contains("最近三个工具调用都失败")
        );
        assert!(
            chat.snapshots[4][0]["content"]
                .as_str()
                .expect("收敛")
                .contains("repeated_tool_loop")
        );
    }

    #[tokio::test]
    async fn distinct_errors_stop_after_warning() {
        let mut steps: Vec<_> = (1..=4)
            .map(|id| Ok(named_step("unknown", &format!(r#"{{"id":{id}}}"#), id)))
            .collect();
        steps.push(Ok(text_step("limited")));
        let mut chat = fake(steps);
        let mut prompt = prompt();
        let (_, metrics) = run_with_budget(&mut chat, &mut prompt, DEFAULT_AGENT_BUDGET)
            .await
            .expect("连续错误应收敛");
        assert_eq!(
            metrics.termination_reason,
            Some(TerminationReason::ConsecutiveErrors)
        );
        assert_eq!(metrics.tool_errors, 4);
    }

    #[tokio::test]
    async fn token_cost_and_missing_usage_stop_before_tool_batch() {
        let cases = [
            (
                Some(TokenUsage {
                    input: 10,
                    output: 1,
                }),
                ResourceLimits {
                    max_input_tokens: Some(10),
                    ..ResourceLimits::default()
                },
                TerminationReason::MaxTokens,
            ),
            (
                Some(TokenUsage {
                    input: 100_000,
                    output: 1,
                }),
                ResourceLimits {
                    max_cost_usd: Some(0.05),
                    input_usd_per_million: Some(1.0),
                    output_usd_per_million: Some(1.0),
                    ..ResourceLimits::default()
                },
                TerminationReason::MaxCost,
            ),
            (
                None,
                ResourceLimits {
                    max_output_tokens: Some(10),
                    ..ResourceLimits::default()
                },
                TerminationReason::UsageUnavailable,
            ),
        ];
        for (usage, limits, reason) in cases {
            let mut first = tool_step();
            first.usage = usage;
            let mut chat = fake(vec![Ok(first), Ok(text_step("limited"))]);
            let mut prompt = prompt();
            let budget = AgentBudget {
                limits,
                ..DEFAULT_AGENT_BUDGET
            };
            let (_, metrics) = run_with_budget(&mut chat, &mut prompt, budget)
                .await
                .expect("应收敛");
            assert_eq!(metrics.termination_reason, Some(reason));
            assert_eq!(metrics.tool_calls, 0);
            assert_eq!(chat.tool_counts, vec![5, 0]);
        }
    }

    #[test]
    fn duration_limit_and_records_are_bounded_and_redacted() {
        let mut runtime = AgentRuntime::new(
            AgentBudget {
                limits: ResourceLimits {
                    max_duration: std::time::Duration::from_millis(1),
                    ..ResourceLimits::default()
                },
                ..DEFAULT_AGENT_BUDGET
            },
            "test-model",
        );
        runtime.started_at -= std::time::Duration::from_millis(2);
        assert_eq!(runtime.limit_reason(), Some(TerminationReason::MaxDuration));

        let call = ToolCall {
            id: "secret-id".to_owned(),
            name: "read".to_owned(),
            args: r#"{"path":"SECRET_PATH"}"#.to_owned(),
        };
        let execution = ToolExecution {
            output: ToolOutput {
                text: "SECRET_RESULT".to_owned(),
                success: true,
                changed: false,
            },
            duration: std::time::Duration::from_millis(7),
        };
        let tool_json = tool_record("run-1", 2, &call, &execution).to_string();
        assert!(tool_json.contains("callFingerprint"));
        assert!(!tool_json.contains("SECRET_PATH"));
        assert!(!tool_json.contains("SECRET_RESULT"));
        let run_json = run_record("run-1", "test-model", &AgentMetrics::default()).to_string();
        assert!(!run_json.contains("SECRET"));
    }
}
