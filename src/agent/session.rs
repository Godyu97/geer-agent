use std::collections::HashSet;

use serde_json::Value;
use uuid::Uuid;

use crate::{
    dao::SessionStore,
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
    #[cfg(test)]
    fail_next_save: bool,
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
            #[cfg(test)]
            fail_next_save: false,
        }
    }

    pub(super) async fn save(&mut self, id: &str, prompt: &mut Prompt, uncertain_tools: bool) {
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
                eprintln!("会话快照编码失败；本次对话继续在内存中运行。");
                return;
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
        } else {
            eprintln!("会话保存失败（Session ID: {id}）；继续运行，后续检查点将重试补写。");
        }
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
    }

    pub(super) fn reset(&mut self) {
        self.revision = None;
        self.head_event_id = None;
        self.queued.clear();
        self.staged_count = 0;
        self.uncertain_tools = false;
    }

    pub(super) fn uncertain_tools(&self) -> bool {
        self.uncertain_tools
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
