use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use uuid::Uuid;

use super::commands::CommandResult;
use crate::{
    interaction::{SessionStatus, Usage},
    memory::{MemoryAction, MemoryPreview},
    prompt::TranscriptEntry,
    session::{DeletePreview, DeleteReport, SessionEntry},
};

pub(crate) type EventSink = Arc<dyn Fn(AppEvent) -> bool + Send + Sync>;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AppSnapshot {
    pub status: SessionStatus,
    pub sessions: Vec<SessionEntry>,
    pub all_sessions: Vec<SessionEntry>,
    pub transcript: Vec<TranscriptEntry>,
    pub unsaved_ids: Vec<String>,
    pub authorization_id: Option<u64>,
    pub revision: u64,
    pub memories: Vec<crate::memory::MemoryEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    fn snapshot() -> AppSnapshot {
        AppSnapshot {
            status: SessionStatus {
                model: "test".into(),
                session_id: "one".into(),
                session_title: "one".into(),
                workspace: "/tmp".into(),
                context_tokens: 0,
                context_window_tokens: 100,
                turn_tokens: 0,
                total_tokens: 0,
                usage_complete: true,
                instructions_loaded: false,
                memory: crate::memory::MemoryService::disabled().status(),
            },
            sessions: vec![],
            all_sessions: vec![],
            transcript: vec![],
            unsaved_ids: vec![],
            authorization_id: None,
            revision: 0,
            memories: vec![],
        }
    }
    fn connect(hub: &EventHub) -> (String, mpsc::Receiver<AppEvent>) {
        let (sent, received) = mpsc::channel();
        let id = hub
            .subscribe(Arc::new(move |event| sent.send(event).is_ok()), 16)
            .unwrap();
        (id, received)
    }
    fn next(received: &mpsc::Receiver<AppEvent>) -> AppEvent {
        received.recv_timeout(Duration::from_secs(2)).unwrap()
    }

    #[test]
    fn join_during_stream_recovers_full_live_state_without_worker() {
        let hub = EventHub::default();
        hub.commit(snapshot(), None, None, CommandResult::default());
        let (first, _) = connect(&hub);
        let id = hub.begin(&first, "question", Some(1), None).unwrap();
        hub.publish(AppEvent::Delta {
            request_id: id,
            text: "partial".into(),
        });
        hub.publish(AppEvent::ToolProgress {
            request_id: id,
            name: "bash".into(),
        });
        hub.publish(AppEvent::Usage {
            request_id: id,
            usage: Some(Usage {
                input: 2,
                output: 3,
            }),
        });
        hub.publish(AppEvent::Authorization {
            id: 7,
            prompt: "confirm".into(),
        });
        let (_, received) = connect(&hub);
        let AppEvent::Sync { state } = next(&received) else {
            panic!("完整状态")
        };
        assert_eq!(state.snapshot.unwrap().revision, 1);
        let running = state.running.unwrap();
        assert_eq!(running.text, "partial\n\n");
        assert_eq!(running.tools, ["bash"]);
        assert_eq!(running.usage, 5);
        assert_eq!(state.authorization.unwrap().id, 7);
    }

    #[test]
    fn commands_are_serial_and_stale_revisions_cannot_change_another_session() {
        let hub = EventHub::default();
        hub.commit(snapshot(), None, None, CommandResult::default());
        let (first, _first_events) = connect(&hub);
        let (second, _second_events) = connect(&hub);
        let id = hub.begin(&first, "/new", Some(1), None).unwrap();
        assert!(
            hub.begin(&second, "wrong", Some(1), None)
                .unwrap_err()
                .contains("仍在执行")
        );
        hub.commit(snapshot(), Some(id), Some(&first), CommandResult::default());
        assert!(
            hub.begin(&second, "wrong", Some(1), None)
                .unwrap_err()
                .contains("状态已改变")
        );
        assert!(hub.begin(&second, "correct", Some(2), None).is_ok());
    }

    #[test]
    fn delete_preview_is_private_and_expires_after_another_command() {
        let hub = EventHub::default();
        hub.commit(snapshot(), None, None, CommandResult::default());
        let (first, first_events) = connect(&hub);
        let (second, second_events) = connect(&hub);
        let id = hub.begin(&first, "/delete", Some(1), None).unwrap();
        hub.commit(
            snapshot(),
            Some(id),
            Some(&first),
            CommandResult {
                delete_confirmation: Some(DeletePreview { targets: vec![] }),
                ..Default::default()
            },
        );
        assert!(
            first_events
                .try_iter()
                .any(|event| matches!(event, AppEvent::DeleteConfirmation { revision: 2, .. }))
        );
        assert!(
            !second_events
                .try_iter()
                .any(|event| matches!(event, AppEvent::DeleteConfirmation { .. }))
        );
        assert!(
            hub.begin(
                &second,
                "/delete --yes",
                Some(2),
                Some(&Confirmation::Sessions(vec![]))
            )
            .is_err()
        );
        let id = hub.begin(&second, "/save", Some(2), None).unwrap();
        hub.commit(
            snapshot(),
            Some(id),
            Some(&second),
            CommandResult::default(),
        );
        assert!(
            hub.begin(
                &first,
                "/delete --yes",
                Some(3),
                Some(&Confirmation::Sessions(vec![]))
            )
            .unwrap_err()
            .contains("过期")
        );
    }

    #[test]
    fn connection_limit_and_slow_subscriber_do_not_block_other_clients() {
        let hub = EventHub::default();
        let id = hub.subscribe(Arc::new(|_| true), 1).unwrap();
        assert!(hub.subscribe(Arc::new(|_| true), 1).is_err());
        assert!(hub.unsubscribe(&id));
        let (sent, received) = mpsc::sync_channel(1);
        hub.subscribe(Arc::new(move |event| sent.try_send(event).is_ok()), 16)
            .unwrap();
        let (_, fast) = connect(&hub);
        hub.publish(AppEvent::Diagnostic {
            message: "overflow".into(),
        });
        assert!(
            fast.try_iter()
                .any(|event| matches!(event, AppEvent::Diagnostic { .. }))
        );
        assert!(matches!(next(&received), AppEvent::Sync { .. }));
        assert!(received.recv_timeout(Duration::from_millis(10)).is_err());
    }

    #[test]
    fn memory_previews_are_private_typed_and_expire_on_state_change() {
        let hub = EventHub::default();
        hub.commit(snapshot(), None, None, CommandResult::default());
        let (first, first_events) = connect(&hub);
        let (second, second_events) = connect(&hub);
        let request = hub.begin(&first, "/memory clear", Some(1), None).unwrap();
        hub.commit(
            snapshot(),
            Some(request),
            Some(&first),
            CommandResult {
                memory_confirmation: Some(MemoryPreview {
                    action: MemoryAction::Clear,
                    entries: vec![],
                    count: 2,
                }),
                ..Default::default()
            },
        );
        assert!(
            first_events
                .try_iter()
                .any(|event| matches!(event, AppEvent::MemoryConfirmation { revision: 2, .. }))
        );
        assert!(
            !second_events
                .try_iter()
                .any(|event| matches!(event, AppEvent::MemoryConfirmation { .. }))
        );
        assert!(
            hub.begin(
                &second,
                "/memory clear --yes",
                Some(2),
                Some(&Confirmation::Memory(MemoryAction::Clear))
            )
            .is_err()
        );
        assert!(
            hub.begin(
                &first,
                "/memory delete --yes other",
                Some(2),
                Some(&Confirmation::Memory(MemoryAction::Delete {
                    id: "other".into()
                }))
            )
            .is_err()
        );
        let request = hub
            .begin(&second, "/memory add new", Some(2), None)
            .unwrap();
        hub.commit(
            snapshot(),
            Some(request),
            Some(&second),
            CommandResult::default(),
        );
        assert!(
            hub.begin(
                &first,
                "/memory clear --yes",
                Some(3),
                Some(&Confirmation::Memory(MemoryAction::Clear))
            )
            .unwrap_err()
            .contains("过期")
        );
        let request = hub.begin(&first, "/memory clear", Some(3), None).unwrap();
        hub.commit(
            snapshot(),
            Some(request),
            Some(&first),
            CommandResult {
                memory_confirmation: Some(MemoryPreview {
                    action: MemoryAction::Clear,
                    entries: vec![],
                    count: 3,
                }),
                ..Default::default()
            },
        );
        assert!(
            hub.begin(
                &first,
                "/memory clear --yes",
                Some(4),
                Some(&Confirmation::Memory(MemoryAction::Clear))
            )
            .is_ok()
        );
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Running {
    pub request_id: u64,
    pub operation_id: String,
    pub client_id: String,
    pub line: String,
    pub text: String,
    pub tools: Vec<String>,
    pub usage: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Authorization {
    pub id: u64,
    pub prompt: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct View {
    pub snapshot: Option<AppSnapshot>,
    pub running: Option<Running>,
    pub authorization: Option<Authorization>,
    pub diagnostics: Vec<String>,
    pub startup_error: Option<String>,
    pub closing: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub delete_report: Option<DeleteReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AppEvent {
    Sync {
        state: Box<View>,
    },
    Snapshot {
        request_id: Option<u64>,
        snapshot: Box<AppSnapshot>,
        notice: Option<String>,
        error: Option<String>,
        delete_report: Option<DeleteReport>,
    },
    Started {
        request_id: u64,
        operation_id: String,
        client_id: String,
        line: String,
    },
    Delta {
        request_id: u64,
        text: String,
    },
    ToolProgress {
        request_id: u64,
        name: String,
    },
    Usage {
        request_id: u64,
        usage: Option<Usage>,
    },
    Authorization {
        id: u64,
        prompt: String,
    },
    AuthorizationResolved {
        id: u64,
    },
    DeleteConfirmation {
        preview: DeletePreview,
        revision: u64,
    },
    MemoryConfirmation {
        preview: MemoryPreview,
        revision: u64,
    },
    Diagnostic {
        message: String,
    },
    StartupError {
        message: String,
    },
    Closing,
    CloseFailed {
        report: String,
        unsaved_ids: Vec<String>,
        can_retry: bool,
    },
    ClientExited,
}

#[derive(Default)]
struct State {
    view: View,
    clients: HashMap<String, EventSink>,
    previews: HashMap<String, (u64, Confirmation)>,
    next_request: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Confirmation {
    Sessions(Vec<String>),
    Memory(MemoryAction),
}

#[derive(Default)]
pub(crate) struct EventHub {
    state: Mutex<State>,
}

impl State {
    fn broadcast(&mut self, event: AppEvent) -> bool {
        self.clients.retain(|_, sink| sink(event.clone()));
        self.previews.retain(|id, _| self.clients.contains_key(id));
        !self.clients.is_empty()
    }

    fn private(&mut self, client: &str, event: AppEvent) {
        if self.clients.get(client).is_some_and(|sink| !sink(event)) {
            self.clients.remove(client);
            self.previews.remove(client);
        }
    }
}

impl EventHub {
    pub fn subscribe(&self, sink: EventSink, limit: usize) -> Result<String, String> {
        let mut state = self.state.lock().expect("图形事件锁损坏");
        if state.view.closing {
            return Err("服务正在关闭。".into());
        }
        if state.clients.len() >= limit {
            return Err("连接已达上限，请关闭其他页面后重试。".into());
        }
        let id = Uuid::new_v4().to_string();
        // 注册和完整状态在同一锁内，回答增量不会落在快照与订阅之间。
        if !sink(AppEvent::Sync {
            state: Box::new(state.view.clone()),
        }) {
            return Err("事件连接不可用。".into());
        }
        state.clients.insert(id.clone(), sink);
        Ok(id)
    }

    pub fn unsubscribe(&self, client: &str) -> bool {
        let mut state = self.state.lock().expect("图形事件锁损坏");
        state.clients.remove(client);
        state.previews.remove(client);
        state.clients.is_empty()
    }

    pub fn begin(
        &self,
        client: &str,
        line: &str,
        revision: Option<u64>,
        confirmed: Option<&Confirmation>,
    ) -> Result<u64, String> {
        let mut state = self.state.lock().expect("图形事件锁损坏");
        if !state.clients.contains_key(client) {
            return Err("页面已经断开。".into());
        }
        if state.view.closing {
            return Err("服务正在关闭。".into());
        }
        let snapshot = state
            .view
            .snapshot
            .as_ref()
            .ok_or("Geer 正在初始化或启动失败。")?;
        if state.view.running.is_some() {
            return Err("上一项操作仍在执行。".into());
        }
        if revision.is_some_and(|revision| revision != snapshot.revision) {
            return Err("会话状态已改变，请等待同步后重试；删除操作需要重新预览。".into());
        }
        if let Some(confirmation) = confirmed {
            let valid = state
                .previews
                .get(client)
                .is_some_and(|(rev, targets)| *rev == snapshot.revision && targets == confirmation);
            if !valid {
                return Err("删除确认已过期，请重新预览。".into());
            }
        }
        state.previews.clear();
        state.next_request += 1;
        let request_id = state.next_request;
        let operation_id = Uuid::new_v4().to_string();
        state.view.running = Some(Running {
            request_id,
            operation_id: operation_id.clone(),
            client_id: client.into(),
            line: line.into(),
            text: String::new(),
            tools: Vec::new(),
            usage: 0,
        });
        state.view.notice = None;
        state.view.error = None;
        state.broadcast(AppEvent::Started {
            request_id,
            operation_id,
            client_id: client.into(),
            line: line.into(),
        });
        Ok(request_id)
    }

    pub fn publish(&self, event: AppEvent) -> bool {
        let mut state = self.state.lock().expect("图形事件锁损坏");
        if matches!(&event, AppEvent::Authorization { .. }) && state.view.closing {
            return false;
        }
        match &event {
            AppEvent::Delta { request_id, text } => {
                if let Some(run) = state
                    .view
                    .running
                    .as_mut()
                    .filter(|run| run.request_id == *request_id)
                {
                    run.text.push_str(text);
                }
            }
            AppEvent::ToolProgress { request_id, name } => {
                if let Some(run) = state
                    .view
                    .running
                    .as_mut()
                    .filter(|run| run.request_id == *request_id)
                {
                    if !run.text.is_empty() && !run.text.ends_with("\n\n") {
                        run.text.push_str("\n\n");
                    }
                    run.tools.push(name.clone());
                }
            }
            AppEvent::Usage {
                request_id,
                usage: Some(usage),
            } => {
                if let Some(run) = state
                    .view
                    .running
                    .as_mut()
                    .filter(|run| run.request_id == *request_id)
                {
                    run.usage += usage.input + usage.output;
                }
            }
            AppEvent::Authorization { id, prompt } => {
                state.view.authorization = Some(Authorization {
                    id: *id,
                    prompt: prompt.clone(),
                })
            }
            AppEvent::AuthorizationResolved { id } => {
                if state
                    .view
                    .authorization
                    .as_ref()
                    .is_some_and(|auth| auth.id == *id)
                {
                    state.view.authorization = None;
                }
            }
            AppEvent::Diagnostic { message } => {
                state.view.diagnostics.push(message.clone());
                if state.view.diagnostics.len() > 20 {
                    state.view.diagnostics.remove(0);
                }
            }
            AppEvent::StartupError { message } => state.view.startup_error = Some(message.clone()),
            AppEvent::Closing => state.view.closing = true,
            AppEvent::CloseFailed { .. } => state.view.closing = false,
            _ => {}
        }
        state.broadcast(event)
    }

    pub fn commit(
        &self,
        mut snapshot: AppSnapshot,
        request_id: Option<u64>,
        client: Option<&str>,
        result: CommandResult,
    ) {
        let mut state = self.state.lock().expect("图形事件锁损坏");
        snapshot.revision = state
            .view
            .snapshot
            .as_ref()
            .map_or(1, |old| old.revision + 1);
        state.view.running = None;
        state.view.notice = result.notice.clone();
        state.view.error = result.error.clone();
        state.view.delete_report = result.delete_report.clone();
        state.view.snapshot = Some(snapshot.clone());
        state.previews.clear();
        state.broadcast(AppEvent::Snapshot {
            request_id,
            snapshot: Box::new(snapshot.clone()),
            notice: result.notice,
            error: result.error,
            delete_report: result.delete_report,
        });
        if let (Some(client), Some(preview)) = (client, result.delete_confirmation) {
            state.previews.insert(
                client.into(),
                (snapshot.revision, Confirmation::Sessions(preview.ids())),
            );
            state.private(
                client,
                AppEvent::DeleteConfirmation {
                    preview,
                    revision: snapshot.revision,
                },
            );
        }
        if let (Some(client), Some(preview)) = (client, result.memory_confirmation) {
            state.previews.insert(
                client.into(),
                (
                    snapshot.revision,
                    Confirmation::Memory(preview.action.clone()),
                ),
            );
            state.private(
                client,
                AppEvent::MemoryConfirmation {
                    preview,
                    revision: snapshot.revision,
                },
            );
        }
    }

    pub fn private(&self, client: &str, event: AppEvent) {
        self.state
            .lock()
            .expect("图形事件锁损坏")
            .private(client, event);
    }
}
