use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    config::OpenAiApi,
    dao::{SESSION_REVISION_CONFLICT, SessionStore},
    interaction::emit_diagnostic,
    prompt::{Prompt, PromptContext, PromptSnapshot, RawEvent},
    session::{
        DeleteItem, DeletePreview, DeleteReport, DeleteState, DeleteTarget, SessionEvent,
        SessionRecord, StoreDeletion, Workspace, delete_ids, session_title, short_id,
    },
    trace::{now_unix_ms, redact_json},
};

pub(crate) struct SessionRuntime {
    store: SessionStore,
    api: String,
    model: String,
    endpoint: String,
    secrets: Vec<String>,
    revision: Option<i64>,
    head_event_id: Option<String>,
    queued: Vec<SessionEvent>,
    staged_count: usize,
    uncertain_tools: bool,
    status: SaveStatus,
    #[cfg(test)]
    fail_next_save: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveStatus {
    Saved,
    Pending,
    Conflict,
}

impl SessionRuntime {
    pub(crate) fn new(
        store: SessionStore,
        api: String,
        model: String,
        base_url: &str,
        mut secrets: Vec<String>,
    ) -> Self {
        secrets.push(base_url.to_owned());
        Self {
            store,
            api,
            model,
            endpoint: endpoint_identity(base_url),
            secrets,
            revision: None,
            head_event_id: None,
            queued: Vec::new(),
            staged_count: 0,
            uncertain_tools: false,
            status: SaveStatus::Pending,
            #[cfg(test)]
            fail_next_save: false,
        }
    }

    pub(crate) fn fresh(&self) -> Self {
        Self {
            store: self.store.clone(),
            api: self.api.clone(),
            model: self.model.clone(),
            endpoint: self.endpoint.clone(),
            secrets: self.secrets.clone(),
            revision: None,
            head_event_id: None,
            queued: Vec::new(),
            staged_count: 0,
            uncertain_tools: false,
            status: SaveStatus::Pending,
            #[cfg(test)]
            fail_next_save: false,
        }
    }

    pub(crate) async fn save(
        &mut self,
        id: &str,
        workspace: &Workspace,
        prompt: &mut Prompt,
        uncertain_tools: bool,
    ) -> SaveStatus {
        self.uncertain_tools = uncertain_tools;
        let secrets: Vec<&str> = self.secrets.iter().map(String::as_str).collect();
        for raw in prompt.pending_events().iter().skip(self.staged_count) {
            let mut payload = raw.payload.clone();
            redact_session_value(&mut payload, &secrets);
            let event_id = Uuid::new_v4().to_string();
            let parent_id = self
                .queued
                .last()
                .map(|event| event.id.clone())
                .or_else(|| self.head_event_id.clone());
            self.queued.push(SessionEvent {
                id: event_id,
                parent_id,
                session_id: id.to_owned(),
                kind: raw.kind.clone(),
                payload,
            });
        }
        self.staged_count = prompt.pending_events().len();
        let mut snapshot = match serde_json::to_value(prompt.snapshot()) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                emit_diagnostic("会话快照编码失败；本次对话继续在内存中运行。");
                self.status = SaveStatus::Pending;
                return self.status;
            }
        };
        redact_session_value(&mut snapshot, &secrets);
        let record = SessionRecord {
            id: id.to_owned(),
            workspace: workspace.as_str(),
            api: self.api.clone(),
            model: self.model.clone(),
            endpoint: self.endpoint.clone(),
            snapshot,
            head_event_id: self
                .queued
                .last()
                .map(|event| event.id.clone())
                .or_else(|| self.head_event_id.clone()),
            revision: self.revision.map_or(0, |revision| revision + 1),
            updated_at_ms: now_unix_ms(),
            uncertain_tools,
        };
        #[cfg(test)]
        let result = if self.fail_next_save {
            self.fail_next_save = false;
            Err(crate::trace::TraceError("模拟会话存储故障".into()))
        } else {
            self.store.save(&record, &self.queued, self.revision).await
        };
        #[cfg(not(test))]
        let result = self.store.save(&record, &self.queued, self.revision).await;
        let saved = result.is_ok()
            || self
                .store
                .load(id)
                .await
                .ok()
                .flatten()
                .is_some_and(|stored| stored == record);
        if saved {
            self.head_event_id = record.head_event_id;
            self.revision = Some(record.revision);
            self.queued.clear();
            self.staged_count = 0;
            prompt.clear_pending_events();
            self.status = SaveStatus::Saved;
        } else {
            self.status = if result
                .as_ref()
                .is_err_and(|error| error.0 == SESSION_REVISION_CONFLICT)
            {
                SaveStatus::Conflict
            } else {
                SaveStatus::Pending
            };
            emit_diagnostic(format!(
                "会话保存失败（Session ID: {id}，状态：{}）；继续运行，后续检查点将重试补写。",
                self.status.label()
            ));
        }
        self.status
    }

    pub(crate) async fn list(
        &self,
        workspace: Option<&Workspace>,
    ) -> Result<Vec<SessionRecord>, String> {
        let Some(workspace) = workspace else {
            return self
                .store
                .list_all()
                .await
                .map_err(|error| error.to_string());
        };
        let mut records = self
            .store
            .list(&workspace.as_str())
            .await
            .map_err(|error| error.to_string())?;
        if cfg!(windows) {
            // 旧版本在 Windows 上保存的是 `\\?\F:\...`；按新写法列出时一并带上。
            let legacy = self
                .store
                .list(&format!(r"\\?\{}", workspace.as_str()))
                .await
                .map_err(|error| error.to_string())?;
            if !legacy.is_empty() {
                records.extend(legacy);
                records.sort_by_key(|record| std::cmp::Reverse(record.updated_at_ms));
            }
        }
        Ok(records)
    }

    pub(crate) async fn load(
        &self,
        id: &str,
    ) -> Result<(SessionRecord, Workspace, PromptSnapshot, Vec<RawEvent>), String> {
        let record = self
            .store
            .load(id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "没有找到该 Session ID。".to_owned())?;
        if record.api != self.api || record.model != self.model || record.endpoint != self.endpoint
        {
            return Err("会话的接口、模型或端点与当前配置不兼容。".to_owned());
        }
        let workspace = Workspace::from_stored(&record.workspace)?;
        let events = self
            .store
            .history(&record)
            .await
            .map_err(|error| error.to_string())?;
        validate_events(&events)?;
        let snapshot = serde_json::from_value(record.snapshot.clone())
            .map_err(|_| "会话快照格式无效。".to_owned())?;
        let display_events = events
            .into_iter()
            .map(|event| RawEvent {
                kind: event.kind,
                payload: event.payload,
            })
            .collect();
        Ok((record, workspace, snapshot, display_events))
    }

    pub(crate) fn adopt(&mut self, record: &SessionRecord) {
        self.revision = Some(record.revision);
        self.head_event_id = record.head_event_id.clone();
        self.queued.clear();
        self.staged_count = 0;
        self.uncertain_tools = record.uncertain_tools;
        self.status = SaveStatus::Saved;
    }

    pub(crate) fn uncertain_tools(&self) -> bool {
        self.uncertain_tools
    }

    pub(crate) fn status(&self) -> SaveStatus {
        self.status
    }
}

impl SaveStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Saved => "已保存",
            Self::Pending => "待补写",
            Self::Conflict => "revision 冲突",
        }
    }
}

pub(crate) struct SessionState {
    pub(crate) id: String,
    pub(crate) workspace: Workspace,
    pub(crate) prompt: Prompt,
    pub(crate) context_token_bias: u64,
    pub(crate) runtime: Option<SessionRuntime>,
    updated_at_ms: i64,
    dirty: bool,
}

impl SessionState {
    fn new(
        api: OpenAiApi,
        prompt_context: &PromptContext,
        workspace: Workspace,
        runtime: Option<SessionRuntime>,
    ) -> Self {
        let system = prompt_context.compose(workspace.as_path());
        Self {
            id: Uuid::new_v4().to_string(),
            workspace,
            prompt: Prompt::new(api, system),
            context_token_bias: 0,
            runtime,
            updated_at_ms: now_unix_ms(),
            dirty: true,
        }
    }

    pub(crate) fn changed(&mut self) {
        self.dirty = true;
        self.updated_at_ms = now_unix_ms();
    }

    pub(crate) async fn save(&mut self) -> Option<SaveStatus> {
        let runtime = self.runtime.as_mut()?;
        let uncertain = runtime.uncertain_tools();
        let result = runtime
            .save(&self.id, &self.workspace, &mut self.prompt, uncertain)
            .await;
        self.dirty = result != SaveStatus::Saved;
        Some(result)
    }

    fn needs_save(&self) -> bool {
        self.runtime.is_some() && self.dirty
    }

    fn status_label(&self) -> &'static str {
        match &self.runtime {
            None => "仅内存",
            Some(runtime) if self.dirty => runtime.status().label(),
            Some(_) => "已保存",
        }
    }
}

pub(crate) struct SessionManager {
    pub(crate) active: SessionState,
    parked: HashMap<String, SessionState>,
    runtime_template: Option<SessionRuntime>,
    api: OpenAiApi,
    prompt_context: PromptContext,
    titles: RefCell<HashMap<String, CachedTitle>>,
}

struct CachedTitle {
    head_event_id: Option<String>,
    title: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SessionEntry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) updated_at_ms: i64,
    pub(crate) model: String,
    pub(crate) status: String,
    pub(crate) active: bool,
    pub(crate) uncertain_tools: bool,
    pub(crate) workspace: String,
}

impl SessionManager {
    pub(crate) fn new(
        api: OpenAiApi,
        prompt_context: PromptContext,
        workspace: Workspace,
        runtime: Option<SessionRuntime>,
    ) -> Self {
        let active = SessionState::new(
            api,
            &prompt_context,
            workspace,
            runtime.as_ref().map(SessionRuntime::fresh),
        );
        Self {
            active,
            parked: HashMap::new(),
            runtime_template: runtime,
            api,
            prompt_context,
            titles: RefCell::new(HashMap::new()),
        }
    }

    fn fresh(&self, workspace: Workspace) -> SessionState {
        SessionState::new(
            self.api,
            &self.prompt_context,
            workspace,
            self.runtime_template.as_ref().map(SessionRuntime::fresh),
        )
    }

    pub(crate) async fn new_session(&mut self) -> String {
        let next = self.fresh(self.active.workspace.clone());
        self.active.save().await;
        let previous = std::mem::replace(&mut self.active, next);
        self.parked.insert(previous.id.clone(), previous);
        self.active.id.clone()
    }

    pub(crate) async fn set_workspace(&mut self, workspace: Workspace) -> Option<String> {
        if self.active.workspace == workspace {
            return None;
        }
        let next = self.fresh(workspace);
        self.active.save().await;
        let previous = std::mem::replace(&mut self.active, next);
        self.parked.insert(previous.id.clone(), previous);
        Some(self.active.id.clone())
    }

    pub(crate) async fn open(&mut self, id: &str) -> Result<Option<bool>, String> {
        if self.active.id == id {
            return Ok(None);
        }
        let target = if let Some(state) = self.parked.get(id) {
            Workspace::from_stored(&state.workspace.as_str())?;
            None
        } else {
            let runtime = self
                .runtime_template
                .as_ref()
                .ok_or_else(|| "未配置会话数据库，无法恢复。".to_owned())?;
            let (record, workspace, snapshot, display_events) = runtime.load(id).await?;
            let mut prompt =
                Prompt::new(self.api, self.prompt_context.compose(workspace.as_path()));
            prompt.restore(snapshot)?;
            prompt.restore_display_events(display_events);
            prompt.commit_turn();
            let mut runtime = runtime.fresh();
            runtime.adopt(&record);
            Some(SessionState {
                id: record.id,
                workspace,
                prompt,
                context_token_bias: 0,
                runtime: Some(runtime),
                updated_at_ms: record.updated_at_ms,
                dirty: false,
            })
        };
        self.active.save().await;
        let next = match target {
            Some(state) => state,
            None => self.parked.remove(id).expect("已检查缓存会话存在"),
        };
        let interrupted = next
            .runtime
            .as_ref()
            .is_some_and(SessionRuntime::uncertain_tools);
        let previous = std::mem::replace(&mut self.active, next);
        self.parked.insert(previous.id.clone(), previous);
        Ok(Some(interrupted))
    }

    pub(crate) async fn save_all(&mut self) -> Vec<(String, SaveStatus)> {
        let mut results = Vec::new();
        if self.active.needs_save() {
            let id = self.active.id.clone();
            if let Some(status) = self.active.save().await {
                results.push((id, status));
            }
        }
        for state in self.parked.values_mut() {
            if state.needs_save() {
                let id = state.id.clone();
                if let Some(status) = state.save().await {
                    results.push((id, status));
                }
            }
        }
        results.sort_by(|left, right| left.0.cmp(&right.0));
        results
    }

    pub(crate) async fn preview_delete(&self, ids: &[String]) -> Result<DeletePreview, String> {
        let mut targets = Vec::new();
        for id in delete_ids(ids)? {
            let state = if id == self.active.id {
                Some(&self.active)
            } else {
                self.parked.get(&id)
            };
            let title = if let Some(state) = state {
                session_title(state.prompt.first_user_input())
            } else if let Some(runtime) = &self.runtime_template {
                match runtime
                    .store
                    .load(&id)
                    .await
                    .map_err(|error| error.to_string())?
                {
                    Some(record) => self.record_title(&record).await,
                    None => short_id(&id),
                }
            } else {
                short_id(&id)
            };
            targets.push(DeleteTarget {
                active: id == self.active.id,
                id,
                title,
            });
        }
        Ok(DeletePreview { targets })
    }

    pub(crate) async fn delete(&mut self, ids: &[String]) -> Result<DeleteReport, String> {
        let mut ids = delete_ids(ids)?;
        ids.sort_by_key(|id| id == &self.active.id);
        let mut report = DeleteReport::default();
        for id in ids {
            let in_memory = id == self.active.id || self.parked.contains_key(&id);
            let result = match &self.runtime_template {
                Some(runtime) => runtime.store.delete(&id).await,
                None => Ok(StoreDeletion {
                    existed: false,
                    cleanup_error: None,
                }),
            };
            let (state, error) = match result {
                Err(error) => (DeleteState::Failed, Some(error.to_string())),
                Ok(deleted) => {
                    self.titles.borrow_mut().remove(&id);
                    if id == self.active.id {
                        // 不走 new_session：用户要丢弃旧状态，不能先保存或停放它。
                        self.active = self.fresh(self.active.workspace.clone());
                        report.new_session_id = Some(self.active.id.clone());
                    } else {
                        self.parked.remove(&id);
                    }
                    let state = if deleted.cleanup_error.is_some() {
                        DeleteState::CleanupPending
                    } else if deleted.existed || in_memory {
                        DeleteState::Deleted
                    } else {
                        DeleteState::Absent
                    };
                    (state, deleted.cleanup_error)
                }
            };
            report.items.push(DeleteItem { id, state, error });
        }
        Ok(report)
    }

    pub(crate) fn unsaved_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = std::iter::once(&self.active)
            .chain(self.parked.values())
            .filter(|state| state.needs_save())
            .map(|state| state.id.clone())
            .collect();
        ids.sort();
        ids
    }

    #[cfg(any(feature = "gui", test))]
    pub(crate) fn volatile_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = std::iter::once(&self.active)
            .chain(self.parked.values())
            .filter(|state| state.runtime.is_none() && !state.prompt.pending_events().is_empty())
            .map(|state| state.id.clone())
            .collect();
        ids.sort();
        ids
    }

    pub(crate) async fn list(&self, all: bool) -> Result<Vec<String>, String> {
        Ok(self
            .list_entries(all)
            .await?
            .into_iter()
            .map(|entry| {
                let time =
                    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(entry.updated_at_ms)
                        .map_or_else(|| entry.updated_at_ms.to_string(), |time| time.to_rfc3339());
                format!(
                    "{}{}  {}  {}  {}  [{}{}]{}",
                    if entry.active { "* " } else { "  " },
                    entry.id,
                    entry.title,
                    time,
                    entry.model,
                    entry.status,
                    if entry.uncertain_tools {
                        "，工具状态未确认"
                    } else {
                        ""
                    },
                    if all {
                        format!("  {}", entry.workspace)
                    } else {
                        String::new()
                    }
                )
            })
            .collect())
    }

    pub(crate) async fn list_entries(&self, all: bool) -> Result<Vec<SessionEntry>, String> {
        let mut entries: HashMap<String, SessionEntry> = HashMap::new();
        if let Some(runtime) = &self.runtime_template {
            let workspace = (!all).then_some(&self.active.workspace);
            match runtime.list(workspace).await {
                Ok(records) => {
                    for record in records {
                        if record.id == self.active.id || self.parked.contains_key(&record.id) {
                            continue;
                        }
                        let title = self.record_title(&record).await;
                        entries.insert(
                            record.id.clone(),
                            SessionEntry {
                                id: record.id,
                                title,
                                updated_at_ms: record.updated_at_ms,
                                model: record.model,
                                status: "已保存".to_owned(),
                                active: false,
                                uncertain_tools: record.uncertain_tools,
                                workspace: record.workspace,
                            },
                        );
                    }
                }
                Err(error) => {
                    emit_diagnostic(format!("会话存档列表读取失败：{error}；仅显示本进程会话。"))
                }
            }
        }
        for state in std::iter::once(&self.active)
            .chain(self.parked.values())
            .filter(|state| all || state.workspace == self.active.workspace)
        {
            entries.insert(
                state.id.clone(),
                SessionEntry {
                    id: state.id.clone(),
                    title: session_title(state.prompt.first_user_input()),
                    updated_at_ms: state.updated_at_ms,
                    model: self
                        .runtime_template
                        .as_ref()
                        .map_or(String::new(), |runtime| runtime.model.clone()),
                    status: state.status_label().to_owned(),
                    active: state.id == self.active.id,
                    uncertain_tools: state
                        .runtime
                        .as_ref()
                        .is_some_and(SessionRuntime::uncertain_tools),
                    workspace: state.workspace.as_str(),
                },
            );
        }
        let mut sorted: Vec<_> = entries.into_values().collect();
        sorted.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(sorted)
    }

    async fn record_title(&self, record: &SessionRecord) -> String {
        if let Some(cached) = self.titles.borrow().get(&record.id)
            && cached.head_event_id == record.head_event_id
        {
            return cached.title.clone();
        }
        let Some(runtime) = &self.runtime_template else {
            return short_id(&record.id);
        };
        match runtime.store.history(record).await {
            Ok(events) => {
                let input = events
                    .iter()
                    .find(|event| event.kind == "user")
                    .and_then(|event| event.payload.get("text").and_then(Value::as_str));
                let title = session_title(input);
                self.titles.borrow_mut().insert(
                    record.id.clone(),
                    CachedTitle {
                        head_event_id: record.head_event_id.clone(),
                        title: title.clone(),
                    },
                );
                title
            }
            Err(error) => {
                emit_diagnostic(format!("会话 {} 标题读取失败：{error}", record.id));
                short_id(&record.id)
            }
        }
    }
}

fn endpoint_identity(base_url: &str) -> String {
    let clean = base_url.split(['?', '#']).next().unwrap_or(base_url);
    if let Some((scheme, rest)) = clean.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        format!("{scheme}://{host}/{path}")
    } else {
        clean.to_owned()
    }
}

fn redact_session_value(value: &mut Value, secrets: &[&str]) {
    redact_json(value, secrets);
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let lower = key.to_ascii_lowercase();
                if [
                    "authorization",
                    "proxy-authorization",
                    "api_key",
                    "api-key",
                    "apikey",
                    "x-api-key",
                    "access_token",
                    "cookie",
                    "set-cookie",
                    "password",
                    "secret",
                ]
                .iter()
                .any(|name| lower == *name)
                {
                    *value = Value::String("[REDACTED]".to_owned());
                } else {
                    redact_session_value(value, &[]);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| redact_session_value(item, &[])),
        Value::String(text) => {
            let mut safe = String::with_capacity(text.len());
            for line in text.split_inclusive('\n') {
                let lower = line.trim_start().to_ascii_lowercase();
                if [
                    "authorization:",
                    "x-api-key:",
                    "cookie:",
                    "set-cookie:",
                    "\"authorization\"",
                    "\"api_key\"",
                    "\"api-key\"",
                    "\"x-api-key\"",
                ]
                .iter()
                .any(|marker| lower.contains(marker))
                {
                    safe.push_str("[REDACTED]");
                    if line.ends_with('\n') {
                        safe.push('\n');
                    }
                } else {
                    safe.push_str(line);
                }
            }
            *text = safe;
        }
        _ => {}
    }
}

fn validate_events(events: &[SessionEvent]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for event in events {
        if !seen.insert(&event.id) {
            return Err("会话事件重复。".to_owned());
        }
        if event.kind == "tool_step" {
            let calls = event
                .payload
                .get("calls")
                .and_then(Value::as_array)
                .ok_or_else(|| "会话工具调用记录已损坏。".to_owned())?;
            let results = event
                .payload
                .get("results")
                .and_then(Value::as_array)
                .ok_or_else(|| "会话工具结果记录已损坏。".to_owned())?;
            if calls.len() != results.len()
                || calls
                    .iter()
                    .zip(results)
                    .any(|(call, result)| call.get("id") != result.get("id"))
            {
                return Err("会话工具调用与结果不匹配。".to_owned());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{OpenAiApi, TraceDatabase, TraceDatabaseConfig},
        provider::{Messages, ModelStep},
    };

    fn reply(text: &str) -> ModelStep {
        ModelStep {
            text: text.into(),
            calls: vec![],
            output: vec![],
            usage: None,
        }
    }

    async fn history_manager(
        api: OpenAiApi,
    ) -> (
        std::path::PathBuf,
        TraceDatabaseConfig,
        SessionStore,
        SessionManager,
    ) {
        let root = std::env::temp_dir().join(format!("geer-history-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}/history.sqlite?mode=rwc", root.display()),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let runtime = SessionRuntime::new(
            store.clone(),
            api.as_str().into(),
            "test".into(),
            "https://example.test/v1",
            vec![],
        );
        let manager = SessionManager::new(
            api,
            context("system"),
            Workspace::from_stored(root.to_str().unwrap()).unwrap(),
            Some(runtime),
        );
        (root, config, store, manager)
    }

    #[tokio::test]
    async fn title_survives_save_compaction_and_restore_without_snapshot_fields() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let (root, _, store, mut manager) = history_manager(api).await;
            let id = manager.active.id.clone();
            let first = "第一条中文🙂输入 \n  从这里开始的历史".repeat(100);
            let expected = session_title(Some(&first));
            manager.active.prompt.begin_turn(&first);
            manager
                .active
                .prompt
                .finish_turn(reply(&"长回答".repeat(100)));
            manager.active.save().await;
            manager.active.prompt.begin_turn("第二条消息");
            manager.active.prompt.finish_turn(reply("继续回答"));
            let plan = manager.active.prompt.prepare_compaction(10, true).unwrap();
            manager
                .active
                .prompt
                .apply_compaction(plan, "早期事实".into())
                .unwrap();
            manager.active.changed();
            manager.active.save().await;
            assert_eq!(
                manager.list_entries(false).await.unwrap()[0].title,
                expected
            );
            let record = store.load(&id).await.unwrap().unwrap();
            assert!(record.snapshot.get("title").is_none());
            assert!(record.snapshot.get("first_user_input").is_none());
            let mut restored = SessionManager::new(
                api,
                context("system"),
                manager.active.workspace.clone(),
                manager.runtime_template.as_ref().map(SessionRuntime::fresh),
            );
            let listed = restored.list_entries(false).await.unwrap();
            assert_eq!(
                listed.iter().find(|entry| entry.id == id).unwrap().title,
                expected
            );
            restored.open(&id).await.unwrap();
            assert_eq!(
                session_title(restored.active.prompt.first_user_input()),
                expected
            );
            restored.active.prompt.begin_turn("后续消息不能重命名会话");
            assert_eq!(
                session_title(restored.active.prompt.first_user_input()),
                expected
            );
            drop((restored, manager, store));
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn deleting_memory_sessions_deduplicates_and_replaces_current_only_when_selected() {
        let mut manager = SessionManager::new(
            OpenAiApi::Responses,
            context("system"),
            Workspace::current().unwrap(),
            None,
        );
        manager.active.prompt.begin_turn("同名消息");
        let first = manager.active.id.clone();
        let second = manager.new_session().await;
        manager.active.prompt.begin_turn("同名消息");
        let preview = manager
            .preview_delete(&[first.clone(), second.clone(), first.clone()])
            .await
            .unwrap();
        assert_eq!(preview.targets.len(), 2);
        assert_eq!(preview.targets[0].title, preview.targets[1].title);
        let report = manager
            .delete(&[first.clone(), first.clone()])
            .await
            .unwrap();
        assert_eq!(report.items.len(), 1);
        assert!(report.new_session_id.is_none());
        assert_eq!(manager.active.id, second);
        assert_eq!(manager.active.prompt.first_user_input(), Some("同名消息"));
        let workspace = manager.active.workspace.clone();
        let report = manager.delete(std::slice::from_ref(&second)).await.unwrap();
        assert_eq!(
            report.new_session_id.as_deref(),
            Some(manager.active.id.as_str())
        );
        assert_eq!(manager.active.workspace, workspace);
        assert!(manager.active.prompt.pending_events().is_empty());
        assert!(manager.parked.is_empty());
        assert_eq!(
            manager.delete(&[second]).await.unwrap().items[0].state,
            DeleteState::Absent
        );
    }

    #[tokio::test]
    async fn deleting_dirty_current_and_parked_states_prevents_flush_resurrection() {
        let (root, _, store, mut manager) = history_manager(OpenAiApi::ChatCompletions).await;
        let failed_id = manager.active.id.clone();
        manager.active.prompt.begin_turn("尚未保存的首条消息");
        manager.active.runtime.as_mut().unwrap().fail_next_save = true;
        let current = manager.new_session().await;
        assert!(manager.unsaved_ids().contains(&failed_id));
        manager.active.prompt.begin_turn("已保存的首句");
        manager.active.prompt.finish_turn(reply("答复"));
        manager.active.save().await;
        manager.active.prompt.begin_turn("待保存的第二条消息");
        manager.active.changed();
        let report = manager
            .delete(&[current.clone(), failed_id.clone()])
            .await
            .unwrap();
        assert!(
            report
                .items
                .iter()
                .all(|item| item.state == DeleteState::Deleted)
        );
        manager.save_all().await;
        assert!(store.load(&current).await.unwrap().is_none());
        assert!(store.load(&failed_id).await.unwrap().is_none());
        assert!(manager.open(&current).await.is_err());
        assert!(!manager.unsaved_ids().contains(&failed_id));
        assert_eq!(manager.list_entries(true).await.unwrap().len(), 1);
        drop((store, manager));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn deletion_failure_rolls_back_database_and_keeps_current_while_other_items_succeed() {
        use sea_orm::{ConnectionTrait, Database};
        let (root, config, store, mut manager) = history_manager(OpenAiApi::Responses).await;
        let previous = manager.active.id.clone();
        let active = manager.new_session().await;
        manager.active.prompt.begin_turn("保留我");
        manager.active.prompt.finish_turn(reply("答复"));
        manager.active.save().await;
        let before = store.load(&active).await.unwrap().unwrap();
        let database = Database::connect(&config.url).await.unwrap();
        database.execute_unprepared(&format!("CREATE TRIGGER fail_delete BEFORE DELETE ON agent_session_events WHEN OLD.session_id = '{active}' BEGIN SELECT RAISE(FAIL, 'test failure'); END")).await.unwrap();
        let report = manager
            .delete(&[previous.clone(), active.clone()])
            .await
            .unwrap();
        assert_eq!(report.items[0].state, DeleteState::Deleted);
        assert_eq!(report.items[1].state, DeleteState::Failed);
        assert_eq!(report.retry_ids(), vec![active.clone()]);
        assert_eq!(manager.active.id, active);
        assert_eq!(manager.active.prompt.first_user_input(), Some("保留我"));
        assert_eq!(store.load(&active).await.unwrap(), Some(before.clone()));
        assert_eq!(store.history(&before).await.unwrap().len(), 2);
        assert!(store.load(&previous).await.unwrap().is_none());
        database
            .execute_unprepared("DROP TRIGGER fail_delete")
            .await
            .unwrap();
        let report = manager.delete(&report.retry_ids()).await.unwrap();
        assert!(report.new_session_id.is_some());
        assert!(store.history(&before).await.is_err());
        drop((database, store, manager));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn deleting_incompatible_archive_does_not_require_existing_workspace_or_valid_history() {
        let (root, _, store, manager) = history_manager(OpenAiApi::Responses).await;
        let id = Uuid::new_v4().to_string();
        let record = SessionRecord {
            id: id.clone(),
            workspace: root.join("missing").display().to_string(),
            api: "different-api".into(),
            model: "old-model".into(),
            endpoint: "different".into(),
            snapshot: Value::Null,
            head_event_id: Some(Uuid::new_v4().to_string()),
            revision: 0,
            updated_at_ms: 0,
            uncertain_tools: false,
        };
        store.save(&record, &[], None).await.unwrap();
        assert_eq!(
            manager
                .list_entries(true)
                .await
                .unwrap()
                .iter()
                .find(|entry| entry.id == id)
                .unwrap()
                .title,
            short_id(&id)
        );
        let mut manager = manager;
        let preview = manager
            .preview_delete(std::slice::from_ref(&id))
            .await
            .unwrap();
        assert_eq!(preview.targets[0].id, id);
        assert_eq!(
            manager
                .delete(std::slice::from_ref(&id))
                .await
                .unwrap()
                .items[0]
                .state,
            DeleteState::Deleted
        );
        assert!(store.load(&id).await.unwrap().is_none());
        drop((store, manager));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn workspace(path: &str) -> Workspace {
        std::fs::create_dir_all(path).unwrap();
        Workspace::from_stored(path).unwrap()
    }

    fn context(label: &str) -> PromptContext {
        PromptContext::for_test(label)
    }

    fn system_prompt(prompt: &Prompt) -> String {
        match prompt.messages() {
            Messages::Chat(messages) => serde_json::to_value(messages).unwrap()[0]["content"]
                .as_str()
                .unwrap()
                .to_owned(),
            Messages::Responses { instructions, .. } => instructions,
        }
    }

    #[tokio::test]
    async fn workspace_switch_rebuilds_both_protocol_prompts_and_deleted_target_is_atomic() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let root =
                std::env::temp_dir().join(format!("geer-session-workspaces-{}", Uuid::new_v4()));
            let path_a = root.join("workspace A");
            let path_b = root.join("工作区 B");
            std::fs::create_dir_all(&path_a).unwrap();
            std::fs::create_dir_all(&path_b).unwrap();
            let workspace_a = Workspace::from_stored(path_a.to_str().unwrap()).unwrap();
            let workspace_b = Workspace::from_stored(path_b.to_str().unwrap()).unwrap();
            let mut manager =
                SessionManager::new(api, context("system"), workspace_a.clone(), None);
            let id_a = manager.active.id.clone();
            manager.active.prompt.begin_turn("A history");
            manager.active.prompt.finish_turn(reply("A answer"));
            assert!(system_prompt(&manager.active.prompt).contains(&workspace_a.as_str()));

            let id_b = manager.set_workspace(workspace_b.clone()).await.unwrap();
            assert_ne!(id_a, id_b);
            assert!(manager.active.prompt.transcript().is_empty());
            let prompt_b = system_prompt(&manager.active.prompt);
            assert!(prompt_b.contains(&workspace_b.as_str()));
            assert!(!prompt_b.contains(&workspace_a.as_str()));
            manager.active.prompt.begin_turn("B history");
            manager.active.prompt.finish_turn(reply("B answer"));

            assert_eq!(manager.list_entries(false).await.unwrap().len(), 1);
            assert_eq!(manager.list_entries(true).await.unwrap().len(), 2);
            assert_eq!(manager.open(&id_a).await.unwrap(), Some(false));
            assert_eq!(manager.active.workspace, workspace_a);
            assert!(system_prompt(&manager.active.prompt).contains(&path_a.display().to_string()));
            assert_eq!(manager.active.prompt.transcript()[0].text, "A history");

            std::fs::remove_dir_all(&path_b).unwrap();
            let before_id = manager.active.id.clone();
            let before_prompt = serde_json::to_value(manager.active.prompt.snapshot()).unwrap();
            assert!(manager.open(&id_b).await.unwrap_err().contains("无法访问"));
            assert_eq!(manager.active.id, before_id);
            assert_eq!(
                serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
                before_prompt
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn memory_switch_preserves_each_prompt_summary_bias_and_pending_events() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let mut manager = SessionManager::new(
                api,
                context("current system"),
                workspace("/tmp/geer-memory-workspace"),
                None,
            );
            let a = manager.active.id.clone();
            manager.active.prompt.begin_turn("A-first");
            manager.active.prompt.finish_turn(reply("A-answer"));
            let plan = manager.active.prompt.prepare_compaction(0, true).unwrap();
            manager
                .active
                .prompt
                .apply_compaction(plan, "A-summary".into())
                .unwrap();
            manager.active.context_token_bias = 41;
            let a_snapshot = serde_json::to_value(manager.active.prompt.snapshot()).unwrap();
            let a_events = manager.active.prompt.pending_events().len();

            let b = manager.new_session().await;
            assert_ne!(a, b);
            assert_eq!(manager.active.context_token_bias, 0);
            manager.active.prompt.begin_turn("B-first");
            manager.active.prompt.finish_turn(reply("B-answer"));
            manager.active.context_token_bias = 7;
            let mut expected_volatile = vec![a.clone(), b.clone()];
            expected_volatile.sort();
            assert_eq!(manager.volatile_ids(), expected_volatile);
            assert!(
                !serde_json::to_string(&manager.active.prompt.snapshot())
                    .unwrap()
                    .contains("A-summary")
            );
            assert_eq!(manager.open(&a).await.unwrap(), Some(false));
            assert_eq!(manager.active.context_token_bias, 41);
            assert_eq!(
                serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
                a_snapshot
            );
            assert_eq!(manager.active.prompt.pending_events().len(), a_events);
            assert_eq!(manager.open(&b).await.unwrap(), Some(false));
            assert_eq!(manager.active.context_token_bias, 7);
            assert!(
                serde_json::to_string(&manager.active.prompt.snapshot())
                    .unwrap()
                    .contains("B-first")
            );
            assert_eq!(manager.open(&b).await.unwrap(), None);
            assert_eq!(manager.active.context_token_bias, 7);
            assert_eq!(manager.active.prompt.transcript()[0].text, "B-first");
            let before = manager.active.id.clone();
            assert!(manager.open("missing").await.is_err());
            assert_eq!(manager.active.id, before);
            let listed = manager.list(false).await.unwrap();
            let structured = manager.list_entries(false).await.unwrap();
            assert_eq!(structured.len(), 2);
            assert!(structured.iter().any(|entry| entry.id == b && entry.active));
            assert!(
                structured
                    .iter()
                    .any(|entry| entry.id == a && !entry.active)
            );
            assert_eq!(listed.len(), 2);
            assert!(
                listed
                    .iter()
                    .any(|line| line.contains("[仅内存]") && line.contains(&a))
            );
            assert!(
                listed
                    .iter()
                    .any(|line| line.starts_with(&format!("* {b}")))
            );
        }
    }

    #[tokio::test]
    async fn failed_parked_save_is_retried_and_revision_conflict_keeps_local_events() {
        let path =
            std::env::temp_dir().join(format!("geer-session-manager-{}.sqlite", Uuid::new_v4()));
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let runtime = SessionRuntime::new(
            store.clone(),
            "chat-completions".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut manager = SessionManager::new(
            OpenAiApi::ChatCompletions,
            context("system"),
            workspace("/tmp/test-workspace"),
            Some(runtime),
        );
        let a = manager.active.id.clone();
        manager.active.prompt.begin_turn("A-private");
        manager.active.prompt.finish_turn(reply("A-answer"));
        manager.active.changed();
        manager.active.runtime.as_mut().unwrap().fail_next_save = true;
        let b = manager.new_session().await;
        assert!(store.load(&a).await.unwrap().is_none());
        assert_eq!(manager.parked[&a].prompt.pending_events().len(), 2);
        assert!(
            manager
                .list(false)
                .await
                .unwrap()
                .iter()
                .any(|line| line.contains(&a) && line.contains("待补写"))
        );
        let results = manager.save_all().await;
        assert!(results.contains(&(a.clone(), SaveStatus::Saved)));
        assert!(results.contains(&(b, SaveStatus::Saved)));
        assert!(manager.unsaved_ids().is_empty());
        assert_eq!(
            store
                .history(&store.load(&a).await.unwrap().unwrap())
                .await
                .unwrap()
                .len(),
            2
        );

        assert_eq!(manager.open(&a).await.unwrap(), Some(false));
        let mut external = store.load(&a).await.unwrap().unwrap();
        external.revision += 1;
        external.updated_at_ms += 1;
        store.save(&external, &[], Some(0)).await.unwrap();
        manager.active.prompt.begin_turn("local-only");
        manager.active.prompt.finish_turn(reply("local-answer"));
        manager.active.changed();
        assert_eq!(manager.active.save().await, Some(SaveStatus::Conflict));
        assert_eq!(store.load(&a).await.unwrap().unwrap(), external);
        assert_eq!(manager.active.prompt.pending_events().len(), 2);
        let results = manager.save_all().await;
        assert!(results.contains(&(a.clone(), SaveStatus::Conflict)));
        assert_eq!(manager.unsaved_ids(), vec![a.clone()]);
        assert!(
            manager
                .list(false)
                .await
                .unwrap()
                .iter()
                .any(|line| line.contains(&a) && line.contains("revision 冲突"))
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn failed_save_before_workspace_switch_retries_with_original_workspace() {
        let root = std::env::temp_dir().join(format!("geer-session-retry-{}", Uuid::new_v4()));
        let path_a = root.join("workspace A");
        let path_b = root.join("workspace B");
        let workspace_a = workspace(path_a.to_str().unwrap());
        let workspace_b = workspace(path_b.to_str().unwrap());
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!(
                "sqlite://{}?mode=rwc",
                root.join("sessions.sqlite").display()
            ),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let runtime = SessionRuntime::new(
            store.clone(),
            "chat-completions".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut manager = SessionManager::new(
            OpenAiApi::ChatCompletions,
            context("system"),
            workspace_a.clone(),
            Some(runtime.fresh()),
        );
        let a = manager.active.id.clone();
        manager.active.prompt.begin_turn("A-private");
        manager.active.prompt.finish_turn(reply("A-answer"));
        manager.active.changed();
        manager.active.runtime.as_mut().unwrap().fail_next_save = true;

        let b = manager.set_workspace(workspace_b.clone()).await.unwrap();
        assert!(store.load(&a).await.unwrap().is_none());
        assert!(manager.unsaved_ids().contains(&a));
        assert_eq!(manager.parked[&a].workspace, workspace_a);
        manager.active.prompt.begin_turn("B-private");
        manager.active.prompt.finish_turn(reply("B-answer"));
        manager.active.changed();

        let results = manager.save_all().await;
        assert!(results.contains(&(a.clone(), SaveStatus::Saved)));
        assert!(results.contains(&(b.clone(), SaveStatus::Saved)));
        let record_a = store.load(&a).await.unwrap().unwrap();
        assert_eq!(record_a.workspace, workspace_a.as_str());
        assert_eq!(store.history(&record_a).await.unwrap().len(), 2);
        assert_eq!(
            store.load(&b).await.unwrap().unwrap().workspace,
            workspace_b.as_str()
        );
        let all = manager.list_entries(true).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(
            all.iter()
                .any(|entry| entry.id == a && entry.workspace == workspace_a.as_str())
        );
        assert_eq!(manager.list_entries(false).await.unwrap().len(), 1);

        let mut restarted = SessionManager::new(
            OpenAiApi::ChatCompletions,
            context("system"),
            workspace_b.clone(),
            Some(runtime),
        );
        let current_id = restarted.active.id.clone();
        std::fs::remove_dir_all(&path_a).unwrap();
        assert!(restarted.open(&a).await.unwrap_err().contains("无法访问"));
        assert_eq!(restarted.active.id, current_id);
        assert_eq!(restarted.active.workspace, workspace_b);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn invalid_snapshot_does_not_change_active_state_and_foreign_workspace_opens() {
        let path =
            std::env::temp_dir().join(format!("geer-session-invalid-{}.sqlite", Uuid::new_v4()));
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let runtime = SessionRuntime::new(
            store.clone(),
            "responses".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut source = SessionManager::new(
            OpenAiApi::Responses,
            context("system"),
            workspace("/tmp/workspace-a"),
            Some(runtime.fresh()),
        );
        let saved_id = source.active.id.clone();
        source.active.prompt.begin_turn("saved input");
        source.active.prompt.finish_turn(reply("saved answer"));
        source.active.changed();
        assert_eq!(source.active.save().await, Some(SaveStatus::Saved));

        let mut compatible = SessionManager::new(
            OpenAiApi::Responses,
            context("restored system"),
            workspace("/tmp/workspace-b"),
            Some(runtime.fresh()),
        );
        assert!(compatible.open(&saved_id).await.is_ok());
        assert_eq!(
            compatible
                .active
                .prompt
                .transcript()
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>(),
            ["saved input", "saved answer"]
        );

        assert_eq!(compatible.active.workspace, workspace("/tmp/workspace-a"));

        let mut manager = SessionManager::new(
            OpenAiApi::Responses,
            context("current system"),
            workspace("/tmp/workspace-b"),
            Some(runtime.fresh()),
        );
        let current_id = manager.active.id.clone();
        manager.active.prompt.begin_turn("local input");
        let original = serde_json::to_value(manager.active.prompt.snapshot()).unwrap();
        assert_eq!(manager.open(&saved_id).await.unwrap(), Some(false));
        assert_eq!(manager.active.workspace, workspace("/tmp/workspace-a"));
        assert_eq!(manager.open(&current_id).await.unwrap(), Some(false));
        assert_eq!(
            serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
            original
        );
        let current_revision = store.load(&current_id).await.unwrap().unwrap().revision;

        let mut corrupted = store.load(&saved_id).await.unwrap().unwrap();
        let expected_revision = corrupted.revision;
        corrupted.revision += 1;
        corrupted.snapshot = Value::Null;
        store
            .save(&corrupted, &[], Some(expected_revision))
            .await
            .unwrap();
        manager.runtime_template = Some(runtime);
        manager.parked.remove(&saved_id);
        assert!(manager.open(&saved_id).await.unwrap_err().contains("快照"));
        assert_eq!(manager.active.id, current_id);
        assert_eq!(
            serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
            original
        );
        assert_eq!(
            store.load(&current_id).await.unwrap().unwrap().revision,
            current_revision
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn failed_checkpoint_retries_raw_events_and_restores_redacted_history() {
        let path =
            std::env::temp_dir().join(format!("geer-session-runtime-{}.sqlite", Uuid::new_v4()));
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let mut runtime = SessionRuntime::new(
            store,
            "chat-completions".into(),
            "test-model".into(),
            "https://user:pass@example.test/v1?key=hidden",
            vec!["known-key".into(), config.url.clone()],
        );
        let id = Uuid::new_v4().to_string();
        let workspace = workspace("/tmp/test-workspace");
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "fresh system".into());
        prompt.begin_turn("Authorization: Bearer my-secret\nX-API-Key: extra-secret\nknown-key");
        prompt.finish_turn(reply("first answer"));
        runtime.fail_next_save = true;
        runtime.save(&id, &workspace, &mut prompt, false).await;
        assert!(runtime.store.load(&id).await.unwrap().is_none());
        assert_eq!(prompt.pending_events().len(), 2);

        prompt.begin_turn("second");
        prompt.finish_turn(reply("second answer"));
        runtime.save(&id, &workspace, &mut prompt, false).await;
        let record = runtime.store.load(&id).await.unwrap().unwrap();
        assert_eq!(record.revision, 0);
        assert_eq!(runtime.store.history(&record).await.unwrap().len(), 4);
        assert!(!record.snapshot.to_string().contains("my-secret"));
        assert!(!record.snapshot.to_string().contains("extra-secret"));
        assert!(!record.snapshot.to_string().contains("known-key"));
        assert_eq!(record.endpoint, "https://example.test/v1");
        let (_, _, snapshot, display_events) = runtime.load(&id).await.unwrap();
        let mut recovered = Prompt::new(OpenAiApi::ChatCompletions, "new system".into());
        recovered.restore(snapshot).unwrap();
        recovered.restore_display_events(display_events);
        assert_eq!(recovered.transcript().len(), 4);
        assert_eq!(recovered.transcript()[3].text, "second answer");
        assert!(!format!("{:?}", recovered.transcript()).contains("my-secret"));
        let crate::provider::Messages::Chat(messages) = recovered.messages() else {
            panic!("Chat 会话")
        };
        let visible = serde_json::to_string(&messages).unwrap();
        assert!(visible.contains("new system"));
        assert!(visible.contains("second answer"));
        let store = SessionStore::connect(&config).await.unwrap();
        let mut restarted = SessionRuntime::new(
            store,
            "chat-completions".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec!["known-key".into(), config.url.clone()],
        );
        let (record, _, _, _) = restarted.load(&id).await.unwrap();
        restarted.adopt(&record);
        recovered.begin_turn("third");
        restarted.save(&id, &workspace, &mut recovered, true).await;
        assert_eq!(
            restarted.store.load(&id).await.unwrap().unwrap().revision,
            1
        );
        assert!(
            restarted
                .store
                .load(&id)
                .await
                .unwrap()
                .unwrap()
                .uncertain_tools
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_broken_tool_event_pairs() {
        let event = SessionEvent {
            id: "one".into(),
            parent_id: None,
            session_id: "s".into(),
            kind: "tool_step".into(),
            payload: serde_json::json!({"calls":[{"id":"call-1"}],"results":[{"id":"call-2"}]}),
        };
        assert!(validate_events(&[event]).is_err());
    }
}
