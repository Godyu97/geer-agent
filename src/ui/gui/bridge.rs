use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

use serde::Serialize;
use tauri::{AppHandle, State, ipc::Channel};

use crate::{
    agent::{self, Agent, SessionEntry},
    interaction::{self, DiagnosticBuffer, Input, Session, SessionStatus, Usage},
    prompt::TranscriptEntry,
    ui::{
        gui_authorization::AuthorizationGate, gui_close::close_failure, gui_commands::handle_line,
    },
};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum GuiEvent {
    Snapshot {
        request_id: Option<u64>,
        snapshot: GuiSnapshot,
        notice: Option<String>,
        error: Option<String>,
    },
    Started {
        request_id: u64,
    },
    Delta {
        request_id: u64,
        text: String,
    },
    Usage {
        request_id: u64,
        usage: Option<Usage>,
    },
    Authorization {
        id: u64,
        prompt: String,
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
}

#[derive(Serialize)]
pub(super) struct GuiSnapshot {
    status: SessionStatus,
    sessions: Vec<SessionEntry>,
    transcript: Vec<TranscriptEntry>,
    unsaved_ids: Vec<String>,
}

#[derive(Default)]
struct EventBus {
    channel: Mutex<Option<Channel<GuiEvent>>>,
}

impl EventBus {
    fn replace(&self, channel: Channel<GuiEvent>) {
        *self.channel.lock().expect("GUI 事件锁损坏") = Some(channel);
    }

    fn send(&self, event: GuiEvent) -> bool {
        let mut channel = self.channel.lock().expect("GUI 事件锁损坏");
        if channel
            .as_ref()
            .is_some_and(|sender| sender.send(event).is_ok())
        {
            true
        } else {
            *channel = None;
            false
        }
    }
}

enum Work {
    Connect,
    Submit { request_id: u64, line: String },
    Close,
}

pub(super) struct GuiBridge {
    work: Sender<Work>,
    queue_lock: Mutex<()>,
    bus: Arc<EventBus>,
    authorization: Arc<AuthorizationGate>,
    busy: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    app: AppHandle,
}

impl GuiBridge {
    pub(super) fn start(app: AppHandle) -> Self {
        let (work, incoming) = mpsc::channel();
        let bus = Arc::new(EventBus::default());
        let authorization_bus = Arc::clone(&bus);
        let authorization = Arc::new(AuthorizationGate::new(move |id, prompt| {
            authorization_bus.send(GuiEvent::Authorization {
                id,
                prompt: prompt.to_owned(),
            })
        }));
        let busy = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let closing = Arc::new(AtomicBool::new(false));
        let worker_bus = Arc::clone(&bus);
        let worker_authorization = Arc::clone(&authorization);
        let worker_busy = Arc::clone(&busy);
        let worker_ready = Arc::clone(&ready);
        let worker_closing = Arc::clone(&closing);
        let worker_app = app.clone();
        std::thread::spawn(move || {
            worker(
                incoming,
                worker_bus,
                worker_authorization,
                worker_busy,
                worker_ready,
                worker_closing,
                worker_app,
            );
        });
        Self {
            work,
            queue_lock: Mutex::new(()),
            bus,
            authorization,
            busy,
            ready,
            closing,
            app,
        }
    }

    pub(super) fn request_close(&self) {
        let _queue = self.queue_lock.lock().expect("GUI 命令锁损坏");
        if !self.closing.swap(true, Ordering::AcqRel) {
            self.bus.send(GuiEvent::Closing);
            self.authorization.cancel();
            let _ = self.work.send(Work::Close);
        }
    }
}

#[tauri::command]
pub(super) fn gui_connect(
    state: State<'_, GuiBridge>,
    on_event: Channel<GuiEvent>,
) -> Result<(), String> {
    state.authorization.cancel();
    state.bus.replace(on_event);
    state
        .work
        .send(Work::Connect)
        .map_err(|_| "GUI 工作线程已退出。".to_owned())
}

#[tauri::command]
pub(super) fn gui_submit(
    state: State<'_, GuiBridge>,
    request_id: u64,
    line: String,
) -> Result<(), String> {
    let _queue = state.queue_lock.lock().expect("GUI 命令锁损坏");
    if state.closing.load(Ordering::Acquire) {
        return Err("窗口正在关闭。".to_owned());
    }
    if !state.ready.load(Ordering::Acquire) {
        return Err("Agent 正在初始化或启动失败。".to_owned());
    }
    if line.trim().is_empty() || line.len() > 65_536 {
        return Err("输入不能为空且不能超过 65536 字节。".to_owned());
    }
    if state
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("上一项操作仍在执行。".to_owned());
    }
    if state.work.send(Work::Submit { request_id, line }).is_err() {
        state.busy.store(false, Ordering::Release);
        return Err("GUI 工作线程已退出。".to_owned());
    }
    Ok(())
}

#[tauri::command]
pub(super) fn gui_authorize(
    state: State<'_, GuiBridge>,
    id: u64,
    allowed: bool,
) -> Result<(), String> {
    state
        .authorization
        .respond(id, allowed)
        .then_some(())
        .ok_or_else(|| "授权请求已过期。".to_owned())
}

#[tauri::command]
pub(super) fn gui_force_close(state: State<'_, GuiBridge>) {
    state.authorization.cancel();
    state.app.exit(0);
}

fn worker(
    incoming: Receiver<Work>,
    bus: Arc<EventBus>,
    authorization: Arc<AuthorizationGate>,
    busy: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    app: AppHandle,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            wait_for_startup_error(incoming, &bus, &app, error.to_string());
            return;
        }
    };
    let diagnostics = DiagnosticBuffer::start();
    let mut agent = match runtime.block_on(agent::create()) {
        Ok(agent) => agent,
        Err(error) => {
            wait_for_startup_error(incoming, &bus, &app, error.to_string());
            return;
        }
    };
    let gate = Arc::clone(&authorization);
    agent.set_confirm(move |prompt| Ok(gate.request(prompt)));
    ready.store(true, Ordering::Release);
    for command in incoming {
        match command {
            Work::Connect => {
                send_snapshot(&runtime, &agent, &bus, None, None, None);
                for message in diagnostics.drain() {
                    bus.send(GuiEvent::Diagnostic { message });
                }
            }
            Work::Submit { request_id, line } => {
                bus.send(GuiEvent::Started { request_id });
                let (notice, error) = runtime.block_on(handle_line(
                    &mut agent,
                    &line,
                    |delta| {
                        bus.send(GuiEvent::Delta {
                            request_id,
                            text: delta.to_owned(),
                        });
                        Ok(())
                    },
                    |usage| {
                        bus.send(GuiEvent::Usage { request_id, usage });
                        Ok(())
                    },
                ));
                for message in diagnostics.drain() {
                    bus.send(GuiEvent::Diagnostic { message });
                }
                if matches!(interaction::parse_input(&line), Input::Exit) {
                    closing.store(true, Ordering::Release);
                    bus.send(GuiEvent::Closing);
                }
                busy.store(false, Ordering::Release);
                send_snapshot(&runtime, &agent, &bus, Some(request_id), notice, error);
                for message in diagnostics.drain() {
                    bus.send(GuiEvent::Diagnostic { message });
                }
                if matches!(interaction::parse_input(&line), Input::Exit) {
                    close(&runtime, &mut agent, &bus, &closing, &app);
                }
            }
            Work::Close => close(&runtime, &mut agent, &bus, &closing, &app),
        }
    }
}

fn wait_for_startup_error(
    incoming: Receiver<Work>,
    bus: &EventBus,
    app: &AppHandle,
    message: String,
) {
    for command in incoming {
        match command {
            Work::Connect => {
                bus.send(GuiEvent::StartupError {
                    message: message.clone(),
                });
            }
            Work::Submit { .. } => {
                bus.send(GuiEvent::StartupError {
                    message: message.clone(),
                });
            }
            Work::Close => {
                app.exit(1);
                return;
            }
        }
    }
}

fn send_snapshot(
    runtime: &tokio::runtime::Runtime,
    agent: &Agent,
    bus: &EventBus,
    request_id: Option<u64>,
    notice: Option<String>,
    error: Option<String>,
) {
    let sessions = match runtime.block_on(agent.session_entries()) {
        Ok(sessions) => sessions,
        Err(error) => {
            bus.send(GuiEvent::Diagnostic {
                message: format!("会话列表读取失败：{error}"),
            });
            Vec::new()
        }
    };
    bus.send(GuiEvent::Snapshot {
        request_id,
        snapshot: GuiSnapshot {
            status: agent.status(),
            sessions,
            transcript: agent.transcript(),
            unsaved_ids: agent.unsaved_ids(),
        },
        notice,
        error,
    });
}

fn close(
    runtime: &tokio::runtime::Runtime,
    agent: &mut Agent,
    bus: &EventBus,
    closing: &AtomicBool,
    app: &AppHandle,
) {
    let failure = close_failure(
        runtime.block_on(agent.flush()),
        agent.unsaved_ids(),
        agent.volatile_ids(),
    );
    if let Some(failure) = failure {
        bus.send(GuiEvent::CloseFailed {
            report: failure.report,
            unsaved_ids: failure.unsaved_ids,
            can_retry: failure.can_retry,
        });
        closing.store(false, Ordering::Release);
    } else {
        app.exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    use tauri::ipc::InvokeResponseBody;

    #[test]
    fn channel_preserves_stream_order_and_request_id() {
        let bus = EventBus::default();
        let (events, received) = mpsc::channel();
        bus.replace(Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                events.send(json).expect("事件接收中");
            }
            Ok(())
        }));
        assert!(bus.send(GuiEvent::Delta {
            request_id: 2,
            text: "a".into()
        }));
        assert!(bus.send(GuiEvent::Delta {
            request_id: 2,
            text: "b".into()
        }));
        let first: serde_json::Value =
            serde_json::from_str(&received.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
        let second: serde_json::Value =
            serde_json::from_str(&received.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
        assert_eq!(first["type"], "delta");
        assert_eq!(first["request_id"], 2);
        assert_eq!(first["text"], "a");
        assert_eq!(second["text"], "b");
    }

    #[test]
    fn missing_frontend_drops_stream() {
        assert!(!EventBus::default().send(GuiEvent::Delta {
            request_id: 1,
            text: "x".into()
        }));
    }
}
