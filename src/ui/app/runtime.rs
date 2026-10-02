use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

use super::{
    authorization::AuthorizationGate,
    close::close_failure,
    commands::{CommandResult, handle_line, tool_progress},
    events::{AppEvent, AppSnapshot, Confirmation, EventHub, EventSink},
};
use crate::{
    agent::{self, Agent},
    interaction::{self, DiagnosticBuffer, Input, Session, SessionScope},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Host {
    Desktop,
    Web,
}

enum Work {
    Submit {
        request_id: u64,
        client: String,
        line: String,
    },
    Close,
}

pub(crate) struct AppRuntime {
    work: Sender<Work>,
    queue: Mutex<bool>,
    hub: Arc<EventHub>,
    authorization: Arc<AuthorizationGate>,
    host: Host,
}

impl AppRuntime {
    pub fn start(
        host: Host,
        startup_error: Option<String>,
        finished: impl Fn(Result<(), String>) + Send + 'static,
    ) -> (Arc<Self>, Receiver<Result<(), String>>) {
        let hub = Arc::new(EventHub::default());
        let notifying = Arc::downgrade(&hub);
        let resolved = Arc::downgrade(&hub);
        let authorization = Arc::new(AuthorizationGate::with_timeout(
            move |id, prompt| {
                notifying.upgrade().is_some_and(|hub| {
                    hub.publish(AppEvent::Authorization {
                        id,
                        prompt: prompt.into(),
                    })
                })
            },
            move |id| {
                if let Some(hub) = resolved.upgrade() {
                    hub.publish(AppEvent::AuthorizationResolved { id });
                }
            },
            (host == Host::Web).then_some(Duration::from_secs(120)),
        ));
        let (work, incoming) = mpsc::channel();
        let (ready, initialized) = mpsc::channel();
        let app = Arc::new(Self {
            work,
            queue: Mutex::new(false),
            hub,
            authorization,
            host,
        });
        let worker_app = Arc::clone(&app);
        std::thread::spawn(move || worker(incoming, worker_app, ready, startup_error, finished));
        (app, initialized)
    }

    pub fn connect(&self, sink: EventSink) -> Result<String, String> {
        self.hub
            .subscribe(sink, if self.host == Host::Web { 16 } else { 1 })
    }

    pub fn disconnect(&self, client: &str) {
        if self.hub.unsubscribe(client) {
            self.authorization.cancel();
        }
    }

    pub fn submit(&self, client: &str, line: String, revision: Option<u64>) -> Result<u64, String> {
        let closing = self.queue.lock().expect("图形命令锁损坏");
        if *closing {
            return Err("服务正在关闭。".into());
        }
        if line.trim().is_empty() || line.len() > 65_536 {
            return Err("输入不能为空且不能超过 65536 字节。".into());
        }
        let confirmed = match interaction::parse_input(&line) {
            Input::Delete {
                ids,
                confirmed: true,
            } if self.host == Host::Web => Some(Confirmation::Sessions(ids)),
            Input::Memory(command) if command.confirmed() => {
                command.action().map(Confirmation::Memory)
            }
            _ => None,
        };
        let request_id = self
            .hub
            .begin(client, &line, revision, confirmed.as_ref())?;
        self.work
            .send(Work::Submit {
                request_id,
                client: client.into(),
                line,
            })
            .map_err(|_| "图形工作线程已退出。".to_owned())?;
        Ok(request_id)
    }

    pub fn authorize(&self, id: u64, allowed: bool) -> Result<(), String> {
        self.authorization
            .respond(id, allowed)
            .then_some(())
            .ok_or_else(|| "授权请求已过期。".into())
    }

    #[cfg(feature = "web")]
    pub fn is_closing(&self) -> bool {
        *self.queue.lock().expect("图形命令锁损坏")
    }

    pub fn request_close(&self) {
        let mut closing = self.queue.lock().expect("图形命令锁损坏");
        if !*closing {
            *closing = true;
            self.hub.publish(AppEvent::Closing);
            self.authorization.cancel();
            let _ = self.work.send(Work::Close);
        }
    }

    pub fn force_disconnect(&self, client: &str) {
        self.hub.private(client, AppEvent::ClientExited);
        self.disconnect(client);
    }

    #[cfg(feature = "gui")]
    pub fn cancel_authorization(&self) {
        self.authorization.cancel();
    }
}

fn worker(
    incoming: Receiver<Work>,
    app: Arc<AppRuntime>,
    ready: Sender<Result<(), String>>,
    startup_error: Option<String>,
    finished: impl Fn(Result<(), String>),
) {
    let diagnostics = DiagnosticBuffer::start();
    let startup = (|| {
        if let Some(error) = startup_error {
            return Err(error);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        // Agent 的非 Send 回调只属于本线程，网络和窗口传递拥有所有权的事件。
        let agent = runtime
            .block_on(agent::create())
            .map_err(|error| error.to_string())?;
        Ok((runtime, agent))
    })();
    let (runtime, mut agent) = match startup {
        Ok(startup) => startup,
        Err(message) => {
            app.hub.publish(AppEvent::StartupError {
                message: message.clone(),
            });
            let _ = ready.send(Err(message));
            if app.host == Host::Desktop {
                for command in incoming {
                    if matches!(command, Work::Close) {
                        finished(Err("启动失败".into()));
                        break;
                    }
                }
            }
            return;
        }
    };
    let gate = Arc::clone(&app.authorization);
    agent.set_confirm(move |prompt| Ok(gate.request(prompt)));
    snapshot(&runtime, &agent, &app, None, None, CommandResult::default());
    drain(&diagnostics, &app);
    let _ = ready.send(Ok(()));
    for command in incoming {
        match command {
            Work::Submit {
                request_id,
                client,
                line,
            } => {
                let exiting = matches!(interaction::parse_input(&line), Input::Exit);
                if exiting {
                    if app.host == Host::Desktop {
                        app.hub.publish(AppEvent::Closing);
                    } else {
                        app.hub.private(&client, AppEvent::Closing);
                    }
                }
                let result = runtime.block_on(handle_line(
                    &mut agent,
                    &line,
                    |delta| {
                        app.hub.publish(match tool_progress(delta) {
                            Some(name) => AppEvent::ToolProgress {
                                request_id,
                                name: name.into(),
                            },
                            None => AppEvent::Delta {
                                request_id,
                                text: delta.into(),
                            },
                        });
                        Ok(())
                    },
                    |usage| {
                        app.hub.publish(AppEvent::Usage { request_id, usage });
                        Ok(())
                    },
                ));
                drain(&diagnostics, &app);
                if !exiting {
                    snapshot(
                        &runtime,
                        &agent,
                        &app,
                        Some(request_id),
                        Some(&client),
                        result,
                    );
                }
                drain(&diagnostics, &app);
                if exiting && close(&runtime, &mut agent, &app, Some(&client), &finished) {
                    break;
                }
            }
            Work::Close => {
                if close(&runtime, &mut agent, &app, None, &finished) {
                    break;
                }
            }
        }
    }
}

fn drain(diagnostics: &DiagnosticBuffer, app: &AppRuntime) {
    for message in diagnostics.drain() {
        app.hub.publish(AppEvent::Diagnostic { message });
    }
}

fn snapshot(
    runtime: &tokio::runtime::Runtime,
    agent: &Agent,
    app: &AppRuntime,
    request_id: Option<u64>,
    client: Option<&str>,
    result: CommandResult,
) {
    let memories = if agent.status().memory.state == crate::memory::MemoryState::Disabled {
        Vec::new()
    } else {
        match runtime.block_on(agent.memories()) {
            Ok(entries) => entries,
            Err(error) => {
                app.hub.publish(AppEvent::Diagnostic {
                    message: format!("记忆列表读取失败：{error}"),
                });
                Vec::new()
            }
        }
    };
    let read = |scope| match runtime.block_on(agent.session_entries(scope)) {
        Ok(entries) => entries,
        Err(error) => {
            app.hub.publish(AppEvent::Diagnostic {
                message: format!("会话列表读取失败：{error}"),
            });
            Vec::new()
        }
    };
    app.hub.commit(
        AppSnapshot {
            status: agent.status(),
            sessions: read(SessionScope::Current),
            all_sessions: read(SessionScope::All),
            transcript: agent.transcript(),
            unsaved_ids: agent.unsaved_ids(),
            authorization_id: app.authorization.pending_id(),
            revision: 0,
            memories,
        },
        request_id,
        client,
        result,
    );
}

fn close(
    runtime: &tokio::runtime::Runtime,
    agent: &mut Agent,
    app: &AppRuntime,
    client: Option<&str>,
    finished: &impl Fn(Result<(), String>),
) -> bool {
    let failure = close_failure(
        runtime.block_on(interaction::save(agent)),
        agent.unsaved_ids(),
        agent.volatile_ids(),
    );
    snapshot(runtime, agent, app, None, None, CommandResult::default());
    if let Some(failure) = failure {
        let message = failure.report.clone();
        let event = AppEvent::CloseFailed {
            report: failure.report,
            unsaved_ids: failure.unsaved_ids,
            can_retry: failure.can_retry,
        };
        if app.host == Host::Web
            && let Some(client) = client
        {
            app.hub.private(client, event);
            return false;
        }
        app.hub.publish(event);
        if app.host == Host::Web {
            finished(Err(message));
            return true;
        }
        *app.queue.lock().expect("图形命令锁损坏") = false;
        false
    } else if app.host == Host::Web
        && let Some(client) = client
    {
        app.force_disconnect(client);
        false
    } else {
        finished(Ok(()));
        true
    }
}
