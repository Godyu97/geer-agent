//! 主业务：提供会话能力，不选择具体界面。

mod guard;

use serde_json::{Value, json};
use std::{
    error::Error,
    io,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, timeout_at};
use uuid::Uuid;

use crate::{
    config::{CompactionConfig, Config, DEFAULT_RESOURCE_LIMITS, OpenAiApi, ResourceLimits},
    dao::{SessionStore, TraceStore},
    interaction::{Session, SessionScope, SessionStatus, Usage, emit_diagnostic},
    prompt::{self, Prompt},
    provider::{ChatProvider, TokenUsage, ToolSpec, openai::Provider},
    session::{
        DeletePreview, DeleteReport, SessionManager, SessionRuntime, Workspace, session_title,
    },
    tools::Tools,
    trace::{
        TraceCapture, TraceRecord, TraceStatus, TraceWriter, now_unix_ms, redact_json, redact_text,
    },
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
    summary_calls: usize,
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
        emit_diagnostic(run_record(&self.run_id, &self.model, &metrics).to_string());
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
        "summaryCalls": metrics.summary_calls,
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

pub(crate) async fn create() -> Result<Agent, Box<dyn Error>> {
    let config = Config::load()?;
    let shared_database =
        config.session_database.is_some() && config.session_database == config.trace_database;
    let session_store = if let Some(database) = &config.session_database {
        match SessionStore::connect(database).await {
            Ok(store) => Some(store),
            Err(_) => {
                emit_diagnostic("会话数据库连接或初始化失败，本次会话仅保存在内存中。");
                None
            }
        }
    } else {
        None
    };
    let trace_store = if shared_database {
        session_store.as_ref().map(SessionStore::trace_store)
    } else if let Some(database) = &config.trace_database {
        match TraceStore::connect(database).await {
            Ok(store) => Some(store),
            Err(_) => {
                emit_diagnostic("Trace 数据库连接或初始化失败，已关闭本次持久化。");
                None
            }
        }
    } else {
        None
    };
    let workspace = Workspace::current().map_err(io::Error::other)?;
    let session_runtime = session_store.map(|store| {
        SessionRuntime::new(
            store,
            config.api.as_str().to_owned(),
            config.model.clone(),
            &config.base_url,
            vec![
                config.api_key.clone(),
                config
                    .session_database
                    .as_ref()
                    .map_or(String::new(), |database| database.url.clone()),
                config
                    .trace_database
                    .as_ref()
                    .map_or(String::new(), |database| database.url.clone()),
            ],
        )
    });
    let prompt_context = prompt::PromptContext::load(&config.bash_bin).await?;
    let agent = Agent {
        chat: Provider::new(&config),
        tools: Tools::new(config.tools_enabled, config.bash_bin.clone())?,
        budget: AgentBudget {
            limits: config.limits,
            ..DEFAULT_AGENT_BUDGET
        },
        compaction: config.compaction,
        model: config.model.clone(),
        sessions: SessionManager::new(config.api, prompt_context, workspace, session_runtime),
        trace_store,
        api: config.api,
        api_key: config.api_key.clone(),
        base_url: config.base_url.clone(),
        trace_database_url: config
            .trace_database
            .as_ref()
            .map(|database| database.url.clone()),
        session_database_url: config
            .session_database
            .as_ref()
            .map(|database| database.url.clone()),
        turn_tokens: 0,
        total_tokens: 0,
        usage_complete: true,
    };
    Ok(agent)
}

pub(crate) struct Agent {
    chat: Provider,
    tools: Tools,
    budget: AgentBudget,
    compaction: CompactionConfig,
    model: String,
    sessions: SessionManager,
    trace_store: Option<TraceStore>,
    api: OpenAiApi,
    api_key: String,
    base_url: String,
    trace_database_url: Option<String>,
    session_database_url: Option<String>,
    turn_tokens: u64,
    total_tokens: u64,
    usage_complete: bool,
}

impl Agent {
    pub(crate) fn set_confirm(&mut self, confirm: impl FnMut(&str) -> io::Result<bool> + 'static) {
        self.tools.set_confirm(confirm);
    }

    #[cfg(any(feature = "gui", feature = "web"))]
    pub(crate) fn transcript(&self) -> Vec<crate::prompt::TranscriptEntry> {
        self.sessions.active.prompt.transcript()
    }

    #[cfg(any(feature = "gui", feature = "web"))]
    pub(crate) fn unsaved_ids(&self) -> Vec<String> {
        self.sessions.unsaved_ids()
    }

    #[cfg(any(feature = "gui", feature = "web"))]
    pub(crate) fn volatile_ids(&self) -> Vec<String> {
        self.sessions.volatile_ids()
    }

    fn record_ui_usage(&mut self, metrics: &AgentMetrics) {
        self.turn_tokens = metrics.input_tokens.saturating_add(metrics.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(self.turn_tokens);
        self.usage_complete &= metrics.usage_complete;
    }
}

struct TraceContext<'a> {
    session_id: &'a str,
    api: OpenAiApi,
    model: &'a str,
    api_key: &'a str,
    base_url: &'a str,
    database_url: Option<&'a str>,
    session_database_url: Option<&'a str>,
    store: Option<&'a TraceStore>,
}

impl Session for Agent {
    fn session_id(&self) -> &str {
        &self.sessions.active.id
    }

    fn workspace(&self) -> String {
        self.sessions.active.workspace.as_str()
    }

    fn status(&self) -> SessionStatus {
        let state = &self.sessions.active;
        let specs = tool_specs(&self.tools);
        SessionStatus {
            model: self.model.clone(),
            session_id: state.id.clone(),
            session_title: session_title(state.prompt.first_user_input()),
            workspace: state.workspace.as_str(),
            context_tokens: state
                .prompt
                .estimated_context_tokens(tool_specs_size(&specs), None)
                .saturating_add(state.context_token_bias),
            context_window_tokens: self.compaction.context_window_tokens,
            turn_tokens: self.turn_tokens,
            total_tokens: self.total_tokens,
            usage_complete: self.usage_complete,
        }
    }

    async fn handle_message<F, U>(
        &mut self,
        input: &str,
        mut on_delta: F,
        mut on_usage: U,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
        U: FnMut(Option<Usage>) -> io::Result<()>,
    {
        self.turn_tokens = 0;
        let state = &mut self.sessions.active;
        state.prompt.begin_turn(input);
        state.changed();
        if let Some(session) = state.runtime.as_mut() {
            session
                .save(
                    &state.id,
                    &state.workspace,
                    &mut state.prompt,
                    false,
                    state.context_token_bias,
                )
                .await;
        }
        let trace = TraceContext {
            session_id: &state.id,
            api: self.api,
            model: &self.model,
            api_key: &self.api_key,
            base_url: &self.base_url,
            database_url: self.trace_database_url.as_deref(),
            session_database_url: self.session_database_url.as_deref(),
            store: self.trace_store.as_ref(),
        };
        let turn_tokens = &mut self.turn_tokens;
        let total_tokens = &mut self.total_tokens;
        let usage_complete = &mut self.usage_complete;
        let result = run_tool_loop_with_session(
            &mut self.chat,
            &mut self.tools,
            &mut state.prompt,
            &mut on_delta,
            &mut |reported| {
                let usage = reported.map(|reported| Usage {
                    input: reported.input,
                    output: reported.output,
                });
                if let Some(usage) = usage {
                    let tokens = usage.input.saturating_add(usage.output);
                    *turn_tokens = turn_tokens.saturating_add(tokens);
                    *total_tokens = total_tokens.saturating_add(tokens);
                } else {
                    *usage_complete = false;
                }
                on_usage(usage)
            },
            self.budget,
            &self.model,
            Some(&trace),
            self.compaction,
            &mut state.context_token_bias,
            &state.workspace,
            state.runtime.as_mut(),
            &state.id,
        )
        .await;
        state.save().await;
        match result {
            Ok(metrics) => {
                self.usage_complete &= metrics.usage_complete;
                Ok(())
            }
            Err(error) => {
                self.usage_complete = false;
                Err(error)
            }
        }
    }

    async fn new_session(&mut self) -> String {
        let id = self.sessions.new_session().await;
        self.tools.reset();
        self.turn_tokens = 0;
        id
    }

    async fn set_workspace(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
        let workspace =
            Workspace::parse(path, &self.sessions.active.workspace).map_err(io::Error::other)?;
        let display = workspace.as_str();
        let Some(id) = self.sessions.set_workspace(workspace).await else {
            return Ok(format!("Workspace 未改变：{display}"));
        };
        self.tools.reset();
        self.turn_tokens = 0;
        Ok(format!("已切换 workspace：{display}\nSession ID: {id}"))
    }

    async fn flush(&mut self) -> String {
        let results = self.sessions.save_all().await;
        let failures = self.sessions.unsaved_ids();
        let mut report = if results.is_empty() {
            "没有待保存的会话。".to_owned()
        } else {
            results
                .iter()
                .map(|(id, status)| format!("{id} [{}]", status.label()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        if !failures.is_empty() {
            report.push_str(&format!("\n未保存的 Session ID: {}", failures.join(", ")));
        }
        report
    }

    async fn compact(&mut self) -> Result<String, Box<dyn Error>> {
        self.turn_tokens = 0;
        let max_source = self
            .compaction
            .context_window_tokens
            .saturating_sub(self.compaction.summary_tokens())
            .saturating_sub((self.compaction.context_window_tokens / 40).min(4096));
        let state = &mut self.sessions.active;
        let Some(plan) = state.prompt.prepare_compaction_bounded(
            self.compaction.recent_tokens(),
            true,
            max_source,
        ) else {
            return Ok("没有可压缩的完整历史片段。".to_owned());
        };
        let trace = TraceContext {
            session_id: &state.id,
            api: self.api,
            model: &self.model,
            api_key: &self.api_key,
            base_url: &self.base_url,
            database_url: self.trace_database_url.as_deref(),
            session_database_url: self.session_database_url.as_deref(),
            store: self.trace_store.as_ref(),
        };
        let mut runtime = AgentRuntime::new(self.budget, &self.model);
        let result = compact_once(
            &mut self.chat,
            &mut state.prompt,
            plan,
            self.compaction,
            &mut runtime,
            Some(&trace),
            &mut |_| Ok(()),
        )
        .await;
        let message = match result {
            Ok(Some((before, after, count))) => {
                state.context_token_bias = 0;
                state.changed();
                state.save().await;
                format!("已压缩 {count} 条旧消息，估算 {before} → {after} tokens。")
            }
            Ok(None) => "没有可压缩的历史。".to_owned(),
            Err(error) => {
                runtime.finish(TerminationReason::ProviderError);
                self.usage_complete = false;
                return Err(io::Error::other(error).into());
            }
        };
        let metrics = runtime.finish(TerminationReason::Completed);
        self.record_ui_usage(&metrics);
        Ok(message)
    }

    async fn sessions(&self, scope: SessionScope) -> Result<Vec<String>, Box<dyn Error>> {
        self.sessions
            .list(scope == SessionScope::All)
            .await
            .map_err(|error| io::Error::other(error).into())
    }

    async fn session_entries(
        &self,
        scope: SessionScope,
    ) -> Result<Vec<crate::session::SessionEntry>, Box<dyn Error>> {
        self.sessions
            .list_entries(scope == SessionScope::All)
            .await
            .map_err(|error| io::Error::other(error).into())
    }

    async fn preview_delete(&self, ids: &[String]) -> Result<DeletePreview, Box<dyn Error>> {
        self.sessions
            .preview_delete(ids)
            .await
            .map_err(|error| io::Error::other(error).into())
    }

    async fn delete_sessions(&mut self, ids: &[String]) -> Result<DeleteReport, Box<dyn Error>> {
        let report = self.sessions.delete(ids).await.map_err(io::Error::other)?;
        if report.new_session_id.is_some() {
            self.tools.reset();
            self.turn_tokens = 0;
        }
        Ok(report)
    }

    async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>> {
        let switched = self.sessions.open(id).await.map_err(io::Error::other)?;
        match switched {
            None => Ok("当前已是该会话。".to_owned()),
            Some(interrupted) => {
                self.tools.reset();
                self.turn_tokens = 0;
                let message = if interrupted {
                    "已恢复会话；上次工具执行可能已产生副作用，状态未确认。请核对后再继续。"
                        .to_owned()
                } else {
                    "已恢复会话。".to_owned()
                };
                Ok(format!("{message}\nWorkspace: {}", self.workspace()))
            }
        }
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

fn tool_specs_size(specs: &[ToolSpec]) -> usize {
    serde_json::to_vec(
        &specs
            .iter()
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_or(0, |value| value.len())
}

async fn compact_once<P: ChatProvider>(
    chat: &mut P,
    prompt: &mut Prompt,
    plan: crate::prompt::CompactionPlan,
    config: CompactionConfig,
    runtime: &mut AgentRuntime,
    trace: Option<&TraceContext<'_>>,
    on_usage: &mut dyn FnMut(Option<TokenUsage>) -> io::Result<()>,
) -> Result<Option<(u64, u64, usize)>, String> {
    if runtime.resource_reason().is_some() {
        return Err("请求资源预算已耗尽。".to_owned());
    }
    let before = prompt.estimated_context_tokens(0, None);
    let count = plan.old_items;
    let messages = prompt.compaction_messages(&plan);
    let request_id = Uuid::new_v4().to_string();
    let started_at_ms = now_unix_ms();
    let started = Instant::now();
    let mut capture = TraceCapture::new();
    runtime.metrics.summary_calls += 1;
    let result = timeout_at(
        runtime.started_at + runtime.budget.limits.max_duration,
        chat.summarize(messages, config.summary_tokens() as u32, &mut capture),
    )
    .await;
    let (status, usage, error) = match &result {
        Ok(Ok(summary)) => (TraceStatus::Completed, summary.usage, None),
        Ok(Err(error)) => (TraceStatus::Failed, None, Some(error.to_string())),
        Err(_) => (TraceStatus::TimedOut, None, Some("摘要调用超时".to_owned())),
    };
    persist_trace(
        &capture,
        trace,
        &request_id,
        &runtime.run_id,
        started_at_ms,
        started,
        status,
        usage,
        error,
    )
    .await;
    let summary = match result {
        Ok(Ok(summary)) => summary,
        Ok(Err(error)) => {
            return Err(trace.map_or_else(
                || error.to_string(),
                |trace| redact_error(&error.to_string(), trace),
            ));
        }
        Err(_) => return Err("摘要调用超时。".to_owned()),
    };
    let resource_reason = runtime.record_usage(summary.usage);
    on_usage(summary.usage).map_err(|error| error.to_string())?;
    if let Some(reason) = resource_reason {
        return Err(format!("摘要后资源预算已耗尽：{}。", reason.as_str()));
    }
    prompt.apply_compaction(plan, summary.text)?;
    let after = prompt.estimated_context_tokens(0, None);
    Ok(Some((before, after, count)))
}

#[cfg(test)]
async fn run_tool_loop_with_budget<P, F>(
    chat: &mut P,
    tools: &mut Tools,
    prompt: &mut Prompt,
    on_delta: &mut F,
    budget: AgentBudget,
    model: &str,
    trace: Option<&TraceContext<'_>>,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    run_tool_loop_with_context(
        chat,
        tools,
        prompt,
        on_delta,
        budget,
        model,
        trace,
        CompactionConfig::default(),
        &mut 0,
    )
    .await
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn run_tool_loop_with_context<P, F>(
    chat: &mut P,
    tools: &mut Tools,
    prompt: &mut Prompt,
    on_delta: &mut F,
    budget: AgentBudget,
    model: &str,
    trace: Option<&TraceContext<'_>>,
    compaction: CompactionConfig,
    context_token_bias: &mut u64,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let workspace = Workspace::current().map_err(io::Error::other)?;
    run_tool_loop_with_session(
        chat,
        tools,
        prompt,
        on_delta,
        &mut |_| Ok(()),
        budget,
        model,
        trace,
        compaction,
        context_token_bias,
        &workspace,
        None,
        "",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_tool_loop_with_session<P, F>(
    chat: &mut P,
    tools: &mut Tools,
    prompt: &mut Prompt,
    on_delta: &mut F,
    on_usage: &mut dyn FnMut(Option<TokenUsage>) -> io::Result<()>,
    budget: AgentBudget,
    model: &str,
    trace: Option<&TraceContext<'_>>,
    compaction: CompactionConfig,
    context_token_bias: &mut u64,
    workspace: &Workspace,
    mut session: Option<&mut SessionRuntime>,
    session_id: &str,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let specs = tool_specs(tools);
    let mut runtime = AgentRuntime::new(budget, model);
    let mut used_tools = false;
    let mut failed_prefix = None;
    let mut retried_overflow = false;

    loop {
        if let Some(reason) = runtime.limit_reason() {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                on_usage,
                &mut runtime,
                Finalization {
                    reason,
                    skipped_tool_batch: false,
                },
                trace,
            )
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
        let max_source = compaction
            .context_window_tokens
            .saturating_sub(compaction.summary_tokens())
            .saturating_sub((compaction.context_window_tokens / 40).min(4096));
        if compaction.auto {
            loop {
                let predicted = prompt
                    .estimated_context_tokens(tool_specs_size(&specs), hint.as_deref())
                    .saturating_add(*context_token_bias);
                if predicted < compaction.trigger_tokens() {
                    break;
                }
                let Some(plan) = prompt.prepare_compaction_bounded(
                    compaction.recent_tokens(),
                    false,
                    max_source,
                ) else {
                    break;
                };
                if failed_prefix == Some(plan.cut) {
                    break;
                }
                let cut = plan.cut;
                match compact_once(
                    chat,
                    prompt,
                    plan,
                    compaction,
                    &mut runtime,
                    trace,
                    on_usage,
                )
                .await
                {
                    Ok(Some((before, after, count))) => {
                        *context_token_bias = 0;
                        failed_prefix = None;
                        if let Some(session) = session.as_deref_mut() {
                            session
                                .save(session_id, workspace, prompt, false, *context_token_bias)
                                .await;
                        }
                        on_delta(&format!(
                            "\n[上下文压缩: {count} 条旧消息，估算 {before} → {after} tokens]\n"
                        ))?;
                    }
                    Ok(None) => break,
                    Err(message) => {
                        failed_prefix = Some(cut);
                        on_delta(&format!("\n[上下文压缩失败: {message}]\n"))?;
                        break;
                    }
                }
            }
        }
        let predicted = prompt
            .estimated_context_tokens(tool_specs_size(&specs), hint.as_deref())
            .saturating_add(*context_token_bias);
        if predicted > compaction.context_window_tokens {
            if used_tools {
                prompt.commit_turn();
            } else {
                prompt.rollback_turn();
            }
            runtime.finish(TerminationReason::ProviderError);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "当前输入超过上下文窗口，无法安全压缩。",
            )
            .into());
        }
        if let Some(reason) = runtime.resource_reason() {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                on_usage,
                &mut runtime,
                Finalization {
                    reason,
                    skipped_tool_batch: false,
                },
                trace,
            )
            .await;
        }
        let messages = match hint.as_deref() {
            Some(instruction) => prompt.messages_with_runtime_instruction(Some(instruction)),
            None => prompt.messages(),
        };
        let deadline = runtime.started_at + budget.limits.max_duration;
        let mut emitted = false;
        let step = match traced_step(
            chat,
            messages,
            &specs,
            &mut |delta| {
                emitted |= !delta.is_empty();
                on_delta(delta)
            },
            Some(deadline),
            trace,
            &runtime.run_id,
        )
        .await
        {
            Ok(Some(step)) => {
                runtime.record_turn();
                if let Some(usage) = step.usage {
                    *context_token_bias = usage.input.saturating_sub(predicted);
                }
                step
            }
            Ok(None) => {
                return finalize_without_tools(
                    chat,
                    prompt,
                    on_delta,
                    on_usage,
                    &mut runtime,
                    Finalization {
                        reason: TerminationReason::MaxDuration,
                        skipped_tool_batch: false,
                    },
                    trace,
                )
                .await;
            }
            Err(error) => {
                if !retried_overflow
                    && !emitted
                    && is_context_overflow(error.as_ref())
                    && let Some(plan) = prompt.prepare_compaction_bounded(0, false, max_source)
                    && let Ok(Some((before, after, count))) = compact_once(
                        chat,
                        prompt,
                        plan,
                        compaction,
                        &mut runtime,
                        trace,
                        on_usage,
                    )
                    .await
                {
                    *context_token_bias = 0;
                    if let Some(session) = session.as_deref_mut() {
                        session
                            .save(session_id, workspace, prompt, false, *context_token_bias)
                            .await;
                    }
                    retried_overflow = true;
                    on_delta(&format!(
                        "\n[上下文窗口溢出后压缩: {count} 条旧消息，估算 {before} → {after} tokens；重试当前模型请求]\n"
                    ))?;
                    continue;
                }
                if used_tools {
                    prompt.commit_turn();
                } else {
                    prompt.rollback_turn();
                }
                runtime.finish(TerminationReason::ProviderError);
                return Err(error);
            }
        };
        retried_overflow = false;
        if step.calls.is_empty() {
            runtime.record_usage(step.usage);
            on_usage(step.usage)?;
            prompt.finish_turn(step);
            return Ok(runtime.finish(TerminationReason::Completed));
        }

        let resource_reason = runtime.record_usage(step.usage);
        on_usage(step.usage)?;
        if let Some(reason) = resource_reason {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                on_usage,
                &mut runtime,
                Finalization {
                    reason,
                    skipped_tool_batch: true,
                },
                trace,
            )
            .await;
        }

        if !runtime.try_record_tool_calls(step.calls.len()) {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                on_usage,
                &mut runtime,
                Finalization {
                    reason: TerminationReason::MaxToolCalls,
                    skipped_tool_batch: true,
                },
                trace,
            )
            .await;
        }

        used_tools = true;
        if let Some(session) = session.as_deref_mut() {
            session
                .save(session_id, workspace, prompt, true, *context_token_bias)
                .await;
        }
        for call in &step.calls {
            on_delta(&format!("\n[调用工具 {}]\n", call.name))?;
        }
        let calls: Vec<_> = step
            .calls
            .iter()
            .map(|call| (call.name.as_str(), call.args.as_str()))
            .collect();
        let executions = tools.execute_batch_in(workspace.as_path(), &calls).await;
        for (call, execution) in step.calls.iter().zip(&executions) {
            emit_diagnostic(
                tool_record(&runtime.run_id, runtime.metrics.turns, call, execution).to_string(),
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
        if let Some(session) = session.as_deref_mut() {
            session
                .save(session_id, workspace, prompt, false, *context_token_bias)
                .await;
        }
        if let Some(reason) = stop {
            return finalize_without_tools(
                chat,
                prompt,
                on_delta,
                on_usage,
                &mut runtime,
                Finalization {
                    reason: reason.into(),
                    skipped_tool_batch: false,
                },
                trace,
            )
            .await;
        }
    }
}

struct Finalization {
    reason: TerminationReason,
    skipped_tool_batch: bool,
}

async fn finalize_without_tools<P, F>(
    chat: &mut P,
    prompt: &mut Prompt,
    on_delta: &mut F,
    on_usage: &mut dyn FnMut(Option<TokenUsage>) -> io::Result<()>,
    runtime: &mut AgentRuntime,
    finalization: Finalization,
    trace: Option<&TraceContext<'_>>,
) -> Result<AgentMetrics, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let mut instruction =
        finalization_instruction(finalization.skipped_tool_batch, finalization.reason);
    instruction.push_str(&format!(" 停止原因：{}。", finalization.reason.as_str()));
    let step = traced_step(
        chat,
        prompt.messages_with_runtime_instruction(Some(&instruction)),
        &[],
        on_delta,
        None,
        trace,
        &runtime.run_id,
    )
    .await;

    match step {
        Ok(Some(step)) => {
            runtime.record_turn();
            runtime.record_usage(step.usage);
            on_usage(step.usage)?;
            // provider 已分别校验两种协议的最终文本；Responses 的文本保存在 output 而非 text。
            if step.calls.is_empty() {
                prompt.finish_turn(step);
            } else {
                prompt.commit_turn();
                on_delta(FINALIZATION_FALLBACK)?;
            }
        }
        Err(_) | Ok(None) => {
            prompt.commit_turn();
            on_delta(FINALIZATION_FALLBACK)?;
        }
    }

    Ok(runtime.finish(finalization.reason))
}

async fn traced_step<P, F>(
    chat: &mut P,
    messages: crate::provider::Messages,
    tools: &[ToolSpec],
    on_delta: &mut F,
    deadline: Option<Instant>,
    trace: Option<&TraceContext<'_>>,
    agent_run_id: &str,
) -> Result<Option<crate::provider::ModelStep>, Box<dyn Error>>
where
    P: ChatProvider,
    F: FnMut(&str) -> io::Result<()>,
{
    let request_id = Uuid::new_v4().to_string();
    let started_at_ms = now_unix_ms();
    let started = Instant::now();
    let mut capture = TraceCapture::new();
    let result = match deadline {
        Some(deadline) => {
            timeout_at(
                deadline,
                chat.complete_step(messages, tools, &mut capture, &mut *on_delta),
            )
            .await
        }
        None => Ok(chat
            .complete_step(messages, tools, &mut capture, &mut *on_delta)
            .await),
    };
    let (status, usage, error) = match &result {
        Ok(Ok(step)) => (TraceStatus::Completed, step.usage, None),
        Ok(Err(error)) => {
            let status = if error
                .downcast_ref::<io::Error>()
                .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)
            {
                TraceStatus::TimedOut
            } else {
                TraceStatus::Failed
            };
            (status, None, Some(error.to_string()))
        }
        Err(_) => (
            TraceStatus::TimedOut,
            None,
            Some("Agent 调用时限已到".to_owned()),
        ),
    };
    persist_trace(
        &capture,
        trace,
        &request_id,
        agent_run_id,
        started_at_ms,
        started,
        status,
        usage,
        error,
    )
    .await;
    match result {
        Ok(Ok(step)) => Ok(Some(step)),
        Ok(Err(error)) => Err(error),
        Err(_) => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
async fn persist_trace(
    capture: &TraceCapture,
    trace: Option<&TraceContext<'_>>,
    request_id: &str,
    agent_run_id: &str,
    started_at_ms: i64,
    started: Instant,
    status: TraceStatus,
    usage: Option<TokenUsage>,
    error: Option<String>,
) {
    let Some(trace) = trace else { return };
    let Some(store) = trace.store else { return };
    let mut response = capture.response();
    let mut request = capture.request.clone();
    let secrets = [
        trace.api_key,
        trace.base_url,
        trace.database_url.unwrap_or_default(),
        trace.session_database_url.unwrap_or_default(),
    ];
    redact_json(&mut request, &secrets);
    if let Some(response) = &mut response {
        redact_json(response, &secrets);
    }
    let input_tokens = usage
        .and_then(|usage| i64::try_from(usage.input).ok())
        .or_else(|| {
            response
                .as_ref()
                .and_then(|response| response.pointer("/usage/input_tokens")?.as_i64())
        });
    let output_tokens = usage
        .and_then(|usage| i64::try_from(usage.output).ok())
        .or_else(|| {
            response
                .as_ref()
                .and_then(|response| response.pointer("/usage/output_tokens")?.as_i64())
        });
    let record = TraceRecord {
        request_id: request_id.to_owned(),
        session_id: trace.session_id.to_owned(),
        agent_run_id: agent_run_id.to_owned(),
        provider_response_id: capture.provider_response_id.clone(),
        api: trace.api.as_str().to_owned(),
        model: trace.model.to_owned(),
        started_at_ms,
        duration_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        attempts: capture.attempts(),
        status,
        input_tokens,
        output_tokens,
        request,
        response,
        error: error.map(|message| redact_error(&message, trace)),
    };
    if store.write_one(&record).await.is_err() {
        emit_diagnostic(format!("Trace 写入失败（request_id={request_id}）。"));
    }
}

fn redact_error(message: &str, trace: &TraceContext<'_>) -> String {
    redact_text(
        message,
        &[
            trace.api_key,
            trace.base_url,
            trace.database_url.unwrap_or_default(),
            trace.session_database_url.unwrap_or_default(),
        ],
    )
}

fn is_context_overflow(error: &(dyn Error + 'static)) -> bool {
    let mut source = Some(error);
    while let Some(current) = source {
        if let Some(async_openai::error::OpenAIError::ApiError(response)) =
            current.downcast_ref::<async_openai::error::OpenAIError>()
        {
            let code = response.api_error.code.as_deref().unwrap_or_default();
            let message = response.api_error.message.to_ascii_lowercase();
            return matches!(code, "context_length_exceeded" | "context_window_exceeded")
                || (response.status_code.as_u16() == 400
                    && (message.contains("context length")
                        || message.contains("context window")
                        || message.contains("maximum context")));
        }
        source = current.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::VecDeque, error::Error, fs, io, rc::Rc, time::SystemTime};

    use super::{
        Agent, AgentBudget, AgentMetrics, AgentRuntime, DEFAULT_AGENT_BUDGET,
        FINALIZATION_FALLBACK, SOFT_BUDGET_HINT, TerminationReason, TraceContext, run_record,
        run_tool_loop_with_budget, run_tool_loop_with_context, run_tool_loop_with_session,
        tool_record,
    };
    use crate::{
        config::{
            CompactionConfig, Config, OpenAiApi, ResourceLimits, TraceDatabase, TraceDatabaseConfig,
        },
        dao::{SessionStore, TraceStore},
        interaction::Session,
        prompt::{Prompt, PromptContext},
        provider::{
            ChatProvider, Messages, ModelStep, TokenUsage, ToolCall, ToolSpec, openai::Provider,
        },
        session::{SessionManager, SessionRuntime, Workspace},
        tools::{ToolExecution, ToolOutput, Tools},
        trace::{TraceCapture, TraceReader, TraceStatus},
    };

    struct FakeProvider {
        steps: VecDeque<Result<ModelStep, String>>,
        snapshots: Vec<serde_json::Value>,
        tool_counts: Vec<usize>,
        observed_usage_count: Option<Rc<Cell<usize>>>,
        usage_count_at_call: Vec<usize>,
    }

    impl ChatProvider for FakeProvider {
        async fn complete_step<F>(
            &mut self,
            messages: Messages,
            tools: &[ToolSpec],
            _capture: &mut crate::trace::TraceCapture,
            _on_delta: F,
        ) -> Result<ModelStep, Box<dyn Error>>
        where
            F: FnMut(&str) -> io::Result<()>,
        {
            let snapshot = match messages {
                Messages::Chat(messages) => serde_json::to_value(messages)?,
                Messages::Responses {
                    instructions,
                    input,
                } => serde_json::json!({
                    "instructions":instructions, "input":input,
                }),
            };
            self.snapshots.push(snapshot);
            self.tool_counts.push(tools.len());
            if let Some(count) = &self.observed_usage_count {
                self.usage_count_at_call.push(count.get());
            }
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

    #[tokio::test]
    async fn reports_each_model_steps_usage_before_the_next_step() {
        let mut first = tool_step();
        first.usage = Some(TokenUsage {
            input: 10,
            output: 2,
        });
        let mut second = text_step("done");
        second.usage = Some(TokenUsage {
            input: 20,
            output: 9,
        });
        let mut provider = fake(vec![Ok(first), Ok(second)]);
        let usage_count = Rc::new(Cell::new(0));
        provider.observed_usage_count = Some(Rc::clone(&usage_count));
        let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
        let mut prompt = prompt();
        let mut reported = Vec::new();
        let workspace = Workspace::current().unwrap();
        let metrics = run_tool_loop_with_session(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            &mut |usage| {
                reported.push(usage);
                usage_count.set(usage_count.get() + 1);
                Ok(())
            },
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
            CompactionConfig::default(),
            &mut 0,
            &workspace,
            None,
            "",
        )
        .await
        .unwrap();
        assert_eq!(reported.len(), 2);
        assert_eq!(provider.usage_count_at_call, vec![0, 1]);
        assert_eq!(reported[0].unwrap().input, 10);
        assert_eq!(reported[1].unwrap().output, 9);
        assert_eq!(metrics.input_tokens, 30);
        assert_eq!(metrics.output_tokens, 11);
    }

    #[tokio::test]
    async fn auto_compaction_runs_before_next_answer_without_consuming_agent_turn() {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".into());
        prompt.begin_turn(&"old".repeat(3_000));
        prompt.finish_turn(text_step("recorded"));
        prompt.begin_turn("latest question");
        let mut provider = fake(vec![
            Ok(text_step("## Goal\nold fact")),
            Ok(text_step("answer")),
        ]);
        let mut tools = Tools::new(false, crate::config::default_bash_bin()).unwrap();
        let config = CompactionConfig {
            context_window_tokens: 5_000,
            auto: true,
        };
        let mut printed = String::new();
        let metrics = run_tool_loop_with_context(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |delta| {
                printed.push_str(delta);
                Ok(())
            },
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
            config,
            &mut 0,
        )
        .await
        .unwrap();
        assert_eq!(metrics.summary_calls, 1);
        assert_eq!(metrics.turns, 1);
        assert_eq!(provider.tool_counts, vec![0, 0]);
        assert!(
            provider.snapshots[0][1]["content"]
                .as_str()
                .unwrap()
                .contains("oldold")
        );
        assert!(
            provider.snapshots[1][1]["content"]
                .as_str()
                .unwrap()
                .contains("old fact")
        );
        assert!(printed.contains("上下文压缩"));
    }

    #[tokio::test]
    async fn failed_summary_keeps_original_history_for_next_model_step() {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".into());
        prompt.begin_turn(&"old".repeat(3_000));
        prompt.finish_turn(text_step("recorded"));
        prompt.begin_turn("latest question");
        let mut provider = fake(vec![
            Err("summary unavailable".into()),
            Ok(text_step("answer")),
        ]);
        let mut tools = Tools::new(false, crate::config::default_bash_bin()).unwrap();
        let mut printed = String::new();
        let metrics = run_tool_loop_with_context(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |delta| {
                printed.push_str(delta);
                Ok(())
            },
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
            CompactionConfig {
                context_window_tokens: 5_000,
                auto: true,
            },
            &mut 0,
        )
        .await
        .unwrap();
        assert_eq!(metrics.summary_calls, 1);
        assert!(printed.contains("上下文压缩失败"));
        assert!(
            provider.snapshots[1][1]["content"]
                .as_str()
                .unwrap()
                .contains("oldold")
        );
    }

    #[tokio::test]
    async fn compacts_complete_tool_batch_before_followup_model_step() {
        let path =
            std::env::temp_dir().join(format!("geer-compact-tool-{}.txt", uuid::Uuid::new_v4()));
        fs::write(&path, "tool fact ".repeat(900)).unwrap();
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".into());
        prompt.begin_turn("read the file");
        let mut provider = fake(vec![
            Ok(named_step(
                "read",
                &serde_json::json!({"path": path}).to_string(),
                1,
            )),
            Ok(text_step("## Goal\nFile inspected")),
            Ok(text_step("final answer")),
        ]);
        let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
        tools.allow_all_for_test();
        let metrics = run_tool_loop_with_context(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
            CompactionConfig {
                context_window_tokens: 7_000,
                auto: true,
            },
            &mut 0,
        )
        .await
        .unwrap();
        assert_eq!(metrics.tool_calls, 1);
        assert_eq!(metrics.summary_calls, 1);
        assert_eq!(provider.tool_counts.len(), 3);
        assert_eq!(provider.tool_counts[1], 0);
        assert!(provider.snapshots[1].to_string().contains("tool fact"));
        assert!(provider.snapshots[2].to_string().contains("File inspected"));
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn oversized_new_input_rolls_back_without_model_call() {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".into());
        prompt.begin_turn(&"large".repeat(1_000));
        let mut provider = fake(vec![]);
        let mut tools = Tools::new(false, crate::config::default_bash_bin()).unwrap();
        let result = run_tool_loop_with_context(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
            CompactionConfig {
                context_window_tokens: 1_024,
                auto: true,
            },
            &mut 0,
        )
        .await;
        assert!(result.is_err());
        assert!(provider.snapshots.is_empty());
        let Messages::Chat(messages) = prompt.messages() else {
            panic!("Chat 消息")
        };
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn resuming_saved_session_clears_existing_tool_grants() {
        let path = std::env::temp_dir().join(format!(
            "geer-resume-grants-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db_config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let store = SessionStore::connect(&db_config).await.unwrap();
        let workspace_path =
            std::env::temp_dir().join(format!("geer-resume-workspace-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&workspace_path).unwrap();
        let workspace = Workspace::from_stored(workspace_path.to_str().unwrap()).unwrap();
        let mut runtime = SessionRuntime::new(
            store,
            "chat-completions".into(),
            "test-model".into(),
            "http://example.test/v1",
            vec!["test-key".into(), db_config.url.clone()],
        );
        let saved_id = uuid::Uuid::new_v4().to_string();
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "current system".into());
        prompt.begin_turn("hello");
        prompt.finish_turn(text_step("answer"));
        runtime
            .save(&saved_id, &workspace, &mut prompt, false, 0)
            .await;
        let config = Config {
            api_key: "test-key".into(),
            model: "test-model".into(),
            base_url: "http://example.test/v1".into(),
            api: OpenAiApi::ChatCompletions,
            tools_enabled: true,
            bash_bin: crate::config::default_bash_bin(),
            limits: ResourceLimits::default(),
            trace_database: None,
            session_database: None,
            compaction: CompactionConfig::default(),
        };
        let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
        tools.grant_for_test("read");
        let mut agent = Agent {
            chat: Provider::new(&config),
            tools,
            budget: DEFAULT_AGENT_BUDGET,
            compaction: config.compaction,
            model: config.model.clone(),
            sessions: SessionManager::new(
                config.api,
                PromptContext::for_test("current system"),
                workspace.clone(),
                Some(runtime),
            ),
            trace_store: None,
            api: config.api,
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
            trace_database_url: None,
            session_database_url: None,
            turn_tokens: 0,
            total_tokens: 0,
            usage_complete: true,
        };
        assert!(agent.tools.granted_for_test("read"));
        agent.record_ui_usage(&AgentMetrics {
            input_tokens: 20,
            output_tokens: 5,
            usage_complete: true,
            ..AgentMetrics::default()
        });
        assert_eq!(agent.status().turn_tokens, 25);
        assert_eq!(agent.status().total_tokens, 25);
        let initial_id = agent.session_id().to_owned();
        assert!(agent.open("missing").await.is_err());
        assert_eq!(agent.session_id(), initial_id);
        assert!(agent.tools.granted_for_test("read"));
        agent.open(&saved_id).await.unwrap();
        assert_eq!(agent.session_id(), saved_id);
        assert!(!agent.tools.granted_for_test("read"));
        agent.tools.grant_for_test("read");
        agent.open(&saved_id).await.unwrap();
        assert!(agent.tools.granted_for_test("read"));
        agent.new_session().await;
        assert!(!agent.tools.granted_for_test("read"));
        assert_eq!(agent.status().turn_tokens, 0);
        assert_eq!(agent.status().total_tokens, 25);
        agent.record_ui_usage(&AgentMetrics {
            input_tokens: 7,
            usage_complete: false,
            ..AgentMetrics::default()
        });
        assert_eq!(agent.status().total_tokens, 32);
        assert!(!agent.status().usage_complete);
        agent.tools.grant_for_test("read");
        agent.open(&saved_id).await.unwrap();
        assert!(!agent.tools.granted_for_test("read"));
        agent.tools.grant_for_test("read");
        let same_id = agent.session_id().to_owned();
        let same_message = agent.set_workspace(&workspace.as_str()).await.unwrap();
        assert!(same_message.contains("未改变"));
        assert_eq!(agent.session_id(), same_id);
        assert!(agent.tools.granted_for_test("read"));
        assert!(agent.set_workspace("missing-workspace").await.is_err());
        assert_eq!(agent.session_id(), same_id);
        assert!(agent.tools.granted_for_test("read"));

        let other_workspace_path = std::env::temp_dir().join(format!(
            "geer-resume-workspace-other-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&other_workspace_path).unwrap();
        let switched = agent
            .set_workspace(other_workspace_path.to_str().unwrap())
            .await
            .unwrap();
        assert!(switched.contains("已切换 workspace"));
        assert_ne!(agent.session_id(), same_id);
        assert_eq!(
            agent.status().workspace,
            crate::config::plain_path(fs::canonicalize(&other_workspace_path).unwrap())
                .display()
                .to_string()
        );
        assert_eq!(agent.status().turn_tokens, 0);
        assert_eq!(agent.status().total_tokens, 32);
        assert!(!agent.tools.granted_for_test("read"));
        let switched_workspace = agent.status().workspace;
        agent.new_session().await;
        assert_eq!(agent.status().workspace, switched_workspace);
        agent.tools.grant_for_test("read");
        agent.turn_tokens = 9;
        let active_id = agent.session_id().to_owned();
        agent.delete_sessions(&[saved_id]).await.unwrap();
        assert!(agent.tools.granted_for_test("read"));
        assert_eq!(agent.status().turn_tokens, 9);
        let report = agent
            .delete_sessions(std::slice::from_ref(&active_id))
            .await
            .unwrap();
        assert_ne!(agent.session_id(), active_id);
        assert_eq!(report.new_session_id.as_deref(), Some(agent.session_id()));
        assert!(!agent.tools.granted_for_test("read"));
        assert_eq!(agent.status().turn_tokens, 0);
        assert_eq!(agent.status().total_tokens, 32);
        assert_eq!(agent.status().session_title, "新会话");
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(workspace_path);
        let _ = fs::remove_dir_all(other_workspace_path);
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
        let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
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
            None,
        )
        .await?;
        Ok((printed, metrics))
    }

    fn fake(steps: Vec<Result<ModelStep, String>>) -> FakeProvider {
        FakeProvider {
            steps: steps.into(),
            snapshots: Vec::new(),
            tool_counts: Vec::new(),
            observed_usage_count: None,
            usage_count_at_call: Vec::new(),
        }
    }

    #[tokio::test]
    async fn both_apis_pair_successful_search_and_web_access_in_the_tool_loop() {
        use serde_json::json;
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
            time::{Duration, Instant},
        };
        use uuid::Uuid;
        for api in [OpenAiApi::Responses, OpenAiApi::ChatCompletions] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = format!("http://{}/", listener.local_addr().unwrap());
            let page_url = format!("{endpoint}page?query=full");
            let source_url = page_url.clone();
            let server = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(8);
                for index in 0..3 {
                    let mut stream = loop {
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                assert!(Instant::now() < deadline, "等待网页请求超时");
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(error) => panic!("mock: {error}"),
                        }
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    loop {
                        let mut block = [0; 4096];
                        let count = stream.read(&mut block).unwrap();
                        assert!(count > 0);
                        bytes.extend_from_slice(&block[..count]);
                        assert!(bytes.len() < 65536);
                        if let Some(end) = bytes.windows(4).position(|chunk| chunk == b"\r\n\r\n") {
                            let length = String::from_utf8_lossy(&bytes[..end])
                                .lines()
                                .find_map(|line| {
                                    let (key, value) = line.split_once(':')?;
                                    key.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let body = if index == 0 {
                        json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text",
                            "text":format!("Title: Mock source\nURL: {source_url}\nHighlights:\nuseful summary")}]}}).to_string()
                    } else {
                        "<p>Web fact</p>".into()
                    };
                    let mime = if index == 0 {
                        "application/json"
                    } else {
                        "text/html"
                    };
                    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                }
            });
            let workspace_path =
                std::env::temp_dir().join(format!("geer-search-web-{}", Uuid::new_v4()));
            fs::create_dir(&workspace_path).unwrap();
            fs::write(
                workspace_path.join("fact.txt"),
                "before\n    local fact\nafter\n",
            )
            .unwrap();
            let mut calls = vec![
                ToolCall {
                    id: "search_call".into(),
                    name: "search".into(),
                    args: json!({"pattern":"local fact"}).to_string(),
                },
                ToolCall {
                    id: "web_search_call".into(),
                    name: "web_search".into(),
                    args: json!({"query":"facts"}).to_string(),
                },
            ];
            calls.extend((1..=2).map(|index| ToolCall {
                id: format!("fetch_{index}"),
                name: "web_fetch".into(),
                args: json!({"url":page_url}).to_string(),
            }));
            let output = if api == OpenAiApi::Responses {
                calls.iter().map(|call|serde_json::from_value(json!({"type":"function_call","id":format!("fc_{}",call.id),"call_id":call.id,"name":call.name,"arguments":call.args,"status":"completed"})).unwrap()).collect()
            } else {
                vec![]
            };
            let mut provider = fake(vec![
                Ok(ModelStep {
                    text: String::new(),
                    calls,
                    output,
                    usage: None,
                }),
                Ok(text_step("done")),
            ]);
            let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
            tools.set_web_endpoint_for_test(&endpoint);
            let confirmations = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let seen = std::rc::Rc::clone(&confirmations);
            tools.set_confirm(move |prompt| {
                seen.borrow_mut().push(prompt.to_owned());
                Ok(true)
            });
            let mut prompt = Prompt::new(api, "system".into());
            prompt.begin_turn("inspect local and web facts");
            let workspace = Workspace::from_stored(workspace_path.to_str().unwrap()).unwrap();
            let metrics = run_tool_loop_with_session(
                &mut provider,
                &mut tools,
                &mut prompt,
                &mut |_| Ok(()),
                &mut |_| Ok(()),
                DEFAULT_AGENT_BUDGET,
                "test-model",
                None,
                CompactionConfig::default(),
                &mut 0,
                &workspace,
                None,
                "",
            )
            .await
            .unwrap();
            assert_eq!(metrics.tool_calls, 4);
            assert_eq!(provider.tool_counts[0], 11);
            let messages = if api == OpenAiApi::Responses {
                provider.snapshots[1]["input"].as_array().unwrap()
            } else {
                provider.snapshots[1].as_array().unwrap()
            };
            for (id, expected) in [
                ("search_call", "fact.txt:2:     local fact"),
                ("web_search_call", "useful summary"),
                ("fetch_1", "Web fact"),
                ("fetch_2", "Web fact"),
            ] {
                let message = messages
                    .iter()
                    .find(|item| {
                        if api == OpenAiApi::Responses {
                            item["call_id"] == id && item["type"] == "function_call_output"
                        } else {
                            item["tool_call_id"] == id && item["role"] == "tool"
                        }
                    })
                    .unwrap();
                let text = if api == OpenAiApi::Responses {
                    message["output"].as_str().unwrap()
                } else {
                    message["content"].as_str().unwrap()
                };
                assert!(text.contains(expected), "{text}");
                let meta: serde_json::Value =
                    serde_json::from_str(text.split_once("\n\n").unwrap().0).unwrap();
                assert_eq!(meta["status"], "ok");
            }
            assert_eq!(confirmations.borrow().len(), 4);
            assert_eq!(
                confirmations
                    .borrow()
                    .iter()
                    .filter(|prompt| prompt.contains("访问 URL："))
                    .count(),
                2
            );
            server.join().unwrap();
            fs::remove_dir_all(workspace_path).unwrap();
        }
    }

    #[tokio::test]
    async fn budget_timeout_persists_partial_output_and_finalization() {
        struct SlowThenFinal {
            calls: usize,
        }
        impl ChatProvider for SlowThenFinal {
            async fn complete_step<F>(
                &mut self,
                _messages: Messages,
                _tools: &[ToolSpec],
                capture: &mut TraceCapture,
                mut on_delta: F,
            ) -> Result<ModelStep, Box<dyn Error>>
            where
                F: FnMut(&str) -> io::Result<()>,
            {
                self.calls += 1;
                capture.set_request(&serde_json::json!({"step": self.calls}))?;
                if self.calls == 1 {
                    capture.append_text("partial");
                    on_delta("partial")?;
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    Ok(text_step("too late"))
                } else {
                    capture.set_response(&serde_json::json!({"text": "final"}))?;
                    on_delta("final")?;
                    Ok(text_step("final"))
                }
            }
        }

        let path = std::env::temp_dir().join(format!(
            "geer-agent-timeout-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let store = TraceStore::connect(&TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        })
        .await
        .unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        let trace = TraceContext {
            session_id: &session,
            api: OpenAiApi::ChatCompletions,
            model: "test-model",
            api_key: "test-key",
            base_url: "http://localhost",
            database_url: None,
            session_database_url: None,
            store: Some(&store),
        };
        let mut budget = DEFAULT_AGENT_BUDGET;
        budget.limits.max_duration = std::time::Duration::from_millis(10);
        let mut provider = SlowThenFinal { calls: 0 };
        let mut tools = Tools::new(false, crate::config::default_bash_bin()).unwrap();
        let mut prompt = prompt();
        let metrics = run_tool_loop_with_budget(
            &mut provider,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            budget,
            "test-model",
            Some(&trace),
        )
        .await
        .unwrap();
        assert_eq!(
            metrics.termination_reason,
            Some(TerminationReason::MaxDuration)
        );
        let page = store.list_session_page(&session, None, 10).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_ne!(page.items[0].request_id, page.items[1].request_id);
        assert_eq!(page.items[0].agent_run_id, page.items[1].agent_run_id);
        let timed_out = page
            .items
            .iter()
            .find(|item| item.request["step"] == 1)
            .unwrap();
        let final_call = page
            .items
            .iter()
            .find(|item| item.request["step"] == 2)
            .unwrap();
        assert_eq!(timed_out.status, TraceStatus::TimedOut);
        assert_eq!(timed_out.response.as_ref().unwrap()["text"], "partial");
        assert_eq!(final_call.status, TraceStatus::Completed);
        drop(store);
        std::fs::remove_file(path).unwrap();
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
    async fn file_results_reach_both_protocols_with_workspace_paths_and_explicit_read_states() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let root =
                std::env::temp_dir().join(format!("geer-read-context-{}", uuid::Uuid::new_v4()));
            let a = root.join("workspace A");
            let b = root.join("workspace B");
            fs::create_dir_all(a.join("src")).unwrap();
            fs::create_dir_all(b.join("src")).unwrap();
            fs::write(a.join("src/main.rs"), "workspace A source").unwrap();
            fs::write(b.join("src/main.rs"), "workspace B source").unwrap();
            fs::write(a.join("src/only-a.rs"), "must not read from A").unwrap();
            fs::write(b.join("empty.txt"), "").unwrap();
            fs::write(b.join("binary.bin"), [0xff, 0xfe]).unwrap();
            let paths = [
                "src/main.rs".to_owned(),
                a.join("src/main.rs").to_string_lossy().into_owned(),
                "src/only-a.rs".to_owned(),
                "empty.txt".to_owned(),
                "src".to_owned(),
                "binary.bin".to_owned(),
            ];
            let calls: Vec<ToolCall> = paths
                .iter()
                .enumerate()
                .map(|(index, path)| ToolCall {
                    id: format!("read_{index}"),
                    name: "read".to_owned(),
                    args: serde_json::json!({"path":path}).to_string(),
                })
                .collect();
            let output = if api == OpenAiApi::Responses {
                calls
                    .iter()
                    .map(|call| {
                        serde_json::from_value(serde_json::json!({
                            "type":"function_call", "id":format!("fc_{}", call.id),
                            "call_id":call.id, "name":call.name, "arguments":call.args,
                            "status":"completed",
                        }))
                        .unwrap()
                    })
                    .collect()
            } else {
                vec![]
            };
            let mut provider = fake(vec![
                Ok(ModelStep {
                    text: String::new(),
                    calls,
                    output,
                    usage: None,
                }),
                Ok(text_step("done")),
            ]);
            let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
            tools.allow_all_for_test();
            let mut prompt = Prompt::new(api, "system".to_owned());
            prompt.begin_turn("读取当前 workspace 的文件");
            let workspace = Workspace::from_stored(b.to_str().unwrap()).unwrap();
            run_tool_loop_with_session(
                &mut provider,
                &mut tools,
                &mut prompt,
                &mut |_| Ok(()),
                &mut |_| Ok(()),
                DEFAULT_AGENT_BUDGET,
                "test-model",
                None,
                CompactionConfig::default(),
                &mut 0,
                &workspace,
                None,
                "",
            )
            .await
            .unwrap();
            let outputs: Vec<_> = match api {
                OpenAiApi::ChatCompletions => provider.snapshots[1]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|message| message["role"] == "tool")
                    .map(|message| message["content"].as_str().unwrap())
                    .collect(),
                OpenAiApi::Responses => provider.snapshots[1]["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|item| item["type"] == "function_call_output")
                    .map(|item| item["output"].as_str().unwrap())
                    .collect(),
            };
            assert_eq!(outputs.len(), paths.len());
            let parts: Vec<_> = outputs
                .iter()
                .map(|text| {
                    let (header, body) = text.split_once("\n\n").unwrap();
                    (
                        serde_json::from_str::<serde_json::Value>(header).unwrap(),
                        body,
                    )
                })
                .collect();
            assert_eq!(parts[0].1, "workspace B source");
            assert_eq!(parts[0].0["path"], serde_json::json!(b.join("src/main.rs")));
            assert_eq!(parts[0].0["empty"], false);
            assert_eq!(parts[1].1, "workspace A source");
            assert_eq!(parts[2].0["code"], "not_found");
            assert_eq!(
                parts[2].0["path"],
                serde_json::json!(b.join("src/only-a.rs"))
            );
            assert_eq!(parts[3].0["status"], "ok");
            assert_eq!(parts[3].0["empty"], true);
            assert!(parts[3].0["message"].as_str().unwrap().contains("文件为空"));
            assert_eq!(parts[3].1, "");
            assert_eq!(parts[4].0["code"], "not_a_file");
            assert_eq!(parts[5].0["code"], "invalid_utf8");
            for index in [2, 4, 5] {
                assert_eq!(parts[index].0["status"], "error");
                assert!(parts[index].0["empty"].is_null());
                assert!(
                    parts[index].0["message"]
                        .as_str()
                        .unwrap()
                        .contains("未获得文件正文")
                );
                assert!(!parts[index].0["hint"].as_str().unwrap().is_empty());
                assert_eq!(parts[index].1, "");
            }
            fs::remove_dir_all(root).unwrap();
        }
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
        let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
        tools.allow_all_for_test();
        let mut prompt = prompt();
        let metrics = run_tool_loop_with_budget(
            &mut chat,
            &mut tools,
            &mut prompt,
            &mut |_| Ok(()),
            DEFAULT_AGENT_BUDGET,
            "test-model",
            None,
        )
        .await
        .expect("批次成功");
        assert_eq!((metrics.turns, metrics.tool_calls), (2, 4));
        let messages = chat.snapshots[1].as_array().expect("第二轮消息");
        let read_body = |index: usize| {
            messages[index]["content"]
                .as_str()
                .expect("读取结果")
                .split_once("\n\n")
                .expect("元信息与正文分隔")
                .1
        };
        assert_eq!(read_body(3), "before");
        assert_eq!(read_body(4), "other");
        let write: serde_json::Value =
            serde_json::from_str(messages[5]["content"].as_str().expect("写入结果"))
                .expect("写入元信息");
        assert_eq!(write["changed"], true);
        assert_eq!(read_body(6), "after");
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
        assert_eq!(chat.tool_counts, vec![11, 11, 0]);
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
        assert_eq!(chat.tool_counts, vec![11, 11, 0]);
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
        assert!(chat.tool_counts[..29].iter().all(|count| *count == 11));
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
        assert_eq!(chat.tool_counts, vec![11, 0]);
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
        assert_eq!(chat.tool_counts, vec![11, 0]);
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
        assert_eq!(chat.tool_counts, vec![11, 11, 11, 11, 0]);
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
            assert_eq!(chat.tool_counts, vec![11, 0]);
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
