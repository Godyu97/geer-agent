use std::collections::{HashMap, HashSet};

use serde_json::Value;
use uuid::Uuid;

use crate::{
    config::OpenAiApi,
    dao::{SESSION_REVISION_CONFLICT, SessionStore},
    interaction::emit_diagnostic,
    prompt::{Prompt, PromptSnapshot},
    session::{SessionEvent, SessionRecord},
    trace::{now_unix_ms, redact_json},
};

pub(super) struct SessionRuntime {
    store: SessionStore,
    workspace: String,
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
pub(super) enum SaveStatus {
    Saved,
    Pending,
    Conflict,
}

impl SessionRuntime {
    pub(super) fn new(
        store: SessionStore,
        workspace: String,
        api: String,
        model: String,
        base_url: &str,
        mut secrets: Vec<String>,
    ) -> Self {
        secrets.push(base_url.to_owned());
        Self {
            store,
            workspace,
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

    pub(super) fn fresh(&self) -> Self {
        Self {
            store: self.store.clone(),
            workspace: self.workspace.clone(),
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

    pub(super) async fn save(
        &mut self,
        id: &str,
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
            workspace: self.workspace.clone(),
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

    pub(super) async fn list(&self) -> Result<Vec<SessionRecord>, String> {
        self.store
            .list(&self.workspace)
            .await
            .map_err(|error| error.to_string())
    }

    pub(super) async fn load(&self, id: &str) -> Result<(SessionRecord, PromptSnapshot), String> {
        let record = self
            .store
            .load(id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "没有找到该 Session ID。".to_owned())?;
        if record.workspace != self.workspace
            || record.api != self.api
            || record.model != self.model
            || record.endpoint != self.endpoint
        {
            return Err("会话的工作目录、接口、模型或端点与当前配置不兼容。".to_owned());
        }
        let events = self
            .store
            .history(&record)
            .await
            .map_err(|error| error.to_string())?;
        validate_events(&events)?;
        let snapshot = serde_json::from_value(record.snapshot.clone())
            .map_err(|_| "会话快照格式无效。".to_owned())?;
        Ok((record, snapshot))
    }

    pub(super) fn adopt(&mut self, record: &SessionRecord) {
        self.revision = Some(record.revision);
        self.head_event_id = record.head_event_id.clone();
        self.queued.clear();
        self.staged_count = 0;
        self.uncertain_tools = record.uncertain_tools;
        self.status = SaveStatus::Saved;
    }

    pub(super) fn uncertain_tools(&self) -> bool {
        self.uncertain_tools
    }

    pub(super) fn status(&self) -> SaveStatus {
        self.status
    }
}

impl SaveStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Saved => "已保存",
            Self::Pending => "待补写",
            Self::Conflict => "revision 冲突",
        }
    }
}

pub(super) struct SessionState {
    pub(super) id: String,
    pub(super) prompt: Prompt,
    pub(super) context_token_bias: u64,
    pub(super) runtime: Option<SessionRuntime>,
    updated_at_ms: i64,
    dirty: bool,
}

impl SessionState {
    fn new(api: OpenAiApi, system: &str, runtime: Option<SessionRuntime>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            prompt: Prompt::new(api, system.to_owned()),
            context_token_bias: 0,
            runtime,
            updated_at_ms: now_unix_ms(),
            dirty: true,
        }
    }

    pub(super) fn changed(&mut self) {
        self.dirty = true;
        self.updated_at_ms = now_unix_ms();
    }

    pub(super) async fn save(&mut self) -> Option<SaveStatus> {
        let runtime = self.runtime.as_mut()?;
        let uncertain = runtime.uncertain_tools();
        let result = runtime.save(&self.id, &mut self.prompt, uncertain).await;
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

pub(super) struct SessionManager {
    pub(super) active: SessionState,
    parked: HashMap<String, SessionState>,
    runtime_template: Option<SessionRuntime>,
    api: OpenAiApi,
    system: String,
}

impl SessionManager {
    pub(super) fn new(api: OpenAiApi, system: String, runtime: Option<SessionRuntime>) -> Self {
        let active = SessionState::new(api, &system, runtime.as_ref().map(SessionRuntime::fresh));
        Self {
            active,
            parked: HashMap::new(),
            runtime_template: runtime,
            api,
            system,
        }
    }

    fn fresh(&self) -> SessionState {
        SessionState::new(
            self.api,
            &self.system,
            self.runtime_template.as_ref().map(SessionRuntime::fresh),
        )
    }

    pub(super) async fn new_session(&mut self) -> String {
        let next = self.fresh();
        self.active.save().await;
        let previous = std::mem::replace(&mut self.active, next);
        self.parked.insert(previous.id.clone(), previous);
        self.active.id.clone()
    }

    pub(super) async fn open(&mut self, id: &str) -> Result<Option<bool>, String> {
        if self.active.id == id {
            return Ok(None);
        }
        let target = if self.parked.contains_key(id) {
            None
        } else {
            let runtime = self
                .runtime_template
                .as_ref()
                .ok_or_else(|| "未配置会话数据库，无法恢复。".to_owned())?;
            let (record, snapshot) = runtime.load(id).await?;
            let mut prompt = Prompt::new(self.api, self.system.clone());
            prompt.restore(snapshot)?;
            prompt.commit_turn();
            let mut runtime = runtime.fresh();
            runtime.adopt(&record);
            Some(SessionState {
                id: record.id,
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

    pub(super) async fn save_all(&mut self) -> Vec<(String, SaveStatus)> {
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

    pub(super) fn unsaved_ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = std::iter::once(&self.active)
            .chain(self.parked.values())
            .filter(|state| state.needs_save())
            .map(|state| state.id.clone())
            .collect();
        ids.sort();
        ids
    }

    pub(super) async fn list(&self) -> Result<Vec<String>, String> {
        let mut entries: HashMap<String, (i64, String)> = HashMap::new();
        if let Some(runtime) = &self.runtime_template {
            match runtime.list().await {
                Ok(records) => {
                    for record in records {
                        entries.insert(
                            record.id.clone(),
                            (
                                record.updated_at_ms,
                                format!(
                                    "已保存{}",
                                    if record.uncertain_tools {
                                        "，工具状态未确认"
                                    } else {
                                        ""
                                    }
                                ),
                            ),
                        );
                    }
                }
                Err(error) => {
                    emit_diagnostic(format!("会话存档列表读取失败：{error}；仅显示本进程会话。"))
                }
            }
        }
        for state in std::iter::once(&self.active).chain(self.parked.values()) {
            let extra = if state
                .runtime
                .as_ref()
                .is_some_and(SessionRuntime::uncertain_tools)
            {
                "，工具状态未确认"
            } else {
                ""
            };
            entries.insert(
                state.id.clone(),
                (
                    state.updated_at_ms,
                    format!("{}{extra}", state.status_label()),
                ),
            );
        }
        let mut sorted: Vec<_> = entries.into_iter().collect();
        sorted.sort_by(|left, right| right.1.0.cmp(&left.1.0).then_with(|| left.0.cmp(&right.0)));
        Ok(sorted
            .into_iter()
            .map(|(id, (updated, status))| {
                let time = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated)
                    .map_or_else(|| updated.to_string(), |time| time.to_rfc3339());
                format!(
                    "{}{}  {}  {}  [{}]",
                    if id == self.active.id { "* " } else { "  " },
                    id,
                    time,
                    self.runtime_template
                        .as_ref()
                        .map_or("", |runtime| runtime.model.as_str()),
                    status
                )
            })
            .collect())
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
        provider::ModelStep,
    };

    fn reply(text: &str) -> ModelStep {
        ModelStep {
            text: text.into(),
            calls: vec![],
            output: vec![],
            usage: None,
        }
    }

    #[tokio::test]
    async fn memory_switch_preserves_each_prompt_summary_bias_and_pending_events() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let mut manager = SessionManager::new(api, "current system".into(), None);
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
            let before = manager.active.id.clone();
            assert!(manager.open("missing").await.is_err());
            assert_eq!(manager.active.id, before);
            let listed = manager.list().await.unwrap();
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
            "/tmp/test-workspace".into(),
            "chat-completions".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut manager =
            SessionManager::new(OpenAiApi::ChatCompletions, "system".into(), Some(runtime));
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
                .list()
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
                .list()
                .await
                .unwrap()
                .iter()
                .any(|line| line.contains(&a) && line.contains("revision 冲突"))
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn invalid_or_foreign_snapshot_does_not_change_active_state() {
        let path =
            std::env::temp_dir().join(format!("geer-session-invalid-{}.sqlite", Uuid::new_v4()));
        let config = TraceDatabaseConfig {
            kind: TraceDatabase::Sqlite,
            url: format!("sqlite://{}?mode=rwc", path.display()),
        };
        let store = SessionStore::connect(&config).await.unwrap();
        let runtime = SessionRuntime::new(
            store.clone(),
            "/tmp/workspace-a".into(),
            "responses".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut source =
            SessionManager::new(OpenAiApi::Responses, "system".into(), Some(runtime.fresh()));
        let saved_id = source.active.id.clone();
        source.active.prompt.begin_turn("saved input");
        source.active.prompt.finish_turn(reply("saved answer"));
        source.active.changed();
        assert_eq!(source.active.save().await, Some(SaveStatus::Saved));

        let foreign = SessionRuntime::new(
            store.clone(),
            "/tmp/workspace-b".into(),
            "responses".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec![],
        );
        let mut manager =
            SessionManager::new(OpenAiApi::Responses, "current system".into(), Some(foreign));
        let current_id = manager.active.id.clone();
        manager.active.prompt.begin_turn("local input");
        let original = serde_json::to_value(manager.active.prompt.snapshot()).unwrap();
        assert!(
            manager
                .open(&saved_id)
                .await
                .unwrap_err()
                .contains("不兼容")
        );
        assert_eq!(manager.active.id, current_id);
        assert_eq!(
            serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
            original
        );
        assert!(store.load(&current_id).await.unwrap().is_none());

        let mut corrupted = store.load(&saved_id).await.unwrap().unwrap();
        corrupted.revision += 1;
        corrupted.snapshot = Value::Null;
        store.save(&corrupted, &[], Some(0)).await.unwrap();
        manager.runtime_template = Some(runtime);
        assert!(manager.open(&saved_id).await.unwrap_err().contains("快照"));
        assert_eq!(manager.active.id, current_id);
        assert_eq!(
            serde_json::to_value(manager.active.prompt.snapshot()).unwrap(),
            original
        );
        assert!(store.load(&current_id).await.unwrap().is_none());
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
            "/tmp/test-workspace".into(),
            "chat-completions".into(),
            "test-model".into(),
            "https://user:pass@example.test/v1?key=hidden",
            vec!["known-key".into(), config.url.clone()],
        );
        let id = Uuid::new_v4().to_string();
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "fresh system".into());
        prompt.begin_turn("Authorization: Bearer my-secret\nX-API-Key: extra-secret\nknown-key");
        prompt.finish_turn(reply("first answer"));
        runtime.fail_next_save = true;
        runtime.save(&id, &mut prompt, false).await;
        assert!(runtime.store.load(&id).await.unwrap().is_none());
        assert_eq!(prompt.pending_events().len(), 2);

        prompt.begin_turn("second");
        prompt.finish_turn(reply("second answer"));
        runtime.save(&id, &mut prompt, false).await;
        let record = runtime.store.load(&id).await.unwrap().unwrap();
        assert_eq!(record.revision, 0);
        assert_eq!(runtime.store.history(&record).await.unwrap().len(), 4);
        assert!(!record.snapshot.to_string().contains("my-secret"));
        assert!(!record.snapshot.to_string().contains("extra-secret"));
        assert!(!record.snapshot.to_string().contains("known-key"));
        assert_eq!(record.endpoint, "https://example.test/v1");
        let (_, snapshot) = runtime.load(&id).await.unwrap();
        let mut recovered = Prompt::new(OpenAiApi::ChatCompletions, "new system".into());
        recovered.restore(snapshot).unwrap();
        let crate::provider::Messages::Chat(messages) = recovered.messages() else {
            panic!("Chat 会话")
        };
        let visible = serde_json::to_string(&messages).unwrap();
        assert!(visible.contains("new system"));
        assert!(visible.contains("second answer"));
        let store = SessionStore::connect(&config).await.unwrap();
        let mut restarted = SessionRuntime::new(
            store,
            "/tmp/test-workspace".into(),
            "chat-completions".into(),
            "test-model".into(),
            "https://example.test/v1",
            vec!["known-key".into(), config.url.clone()],
        );
        let (record, _) = restarted.load(&id).await.unwrap();
        restarted.adopt(&record);
        recovered.begin_turn("third");
        restarted.save(&id, &mut recovered, true).await;
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
