use crate::ui::app::{AppEvent, AppRuntime, Host};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State, ipc::Channel};

pub(super) struct GuiBridge {
    runtime: Arc<AppRuntime>,
    client: Mutex<Option<String>>,
    app: AppHandle,
}

impl GuiBridge {
    pub(super) fn start(app: AppHandle, startup_error: Option<String>) -> Self {
        let window = app.clone();
        let (runtime, _) = AppRuntime::start(Host::Desktop, startup_error, move |result| {
            window.exit(if result.is_ok() { 0 } else { 1 })
        });
        Self {
            runtime,
            client: Mutex::new(None),
            app,
        }
    }
    pub(super) fn request_close(&self) {
        self.runtime.request_close();
    }
}

#[tauri::command]
pub(super) fn gui_connect(
    state: State<'_, GuiBridge>,
    on_event: Channel<AppEvent>,
) -> Result<(), String> {
    let mut client = state.client.lock().expect("窗口连接锁损坏");
    if let Some(old) = client.take() {
        state.runtime.disconnect(&old);
    }
    *client = Some(
        state
            .runtime
            .connect(Arc::new(move |event| on_event.send(event).is_ok()))?,
    );
    Ok(())
}

#[tauri::command]
pub(super) fn gui_submit(
    state: State<'_, GuiBridge>,
    request_id: u64,
    line: String,
    revision: Option<u64>,
) -> Result<(), String> {
    // 前端编号仅用于本端失败关联；全局编号通过 Started 事件返回。
    let _ = request_id;
    let client = state.client.lock().expect("窗口连接锁损坏");
    state
        .runtime
        .submit(client.as_deref().ok_or("窗口尚未连接。")?, line, revision)
        .map(|_| ())
}

#[tauri::command]
pub(super) fn gui_authorize(
    state: State<'_, GuiBridge>,
    id: u64,
    allowed: bool,
) -> Result<(), String> {
    state.runtime.authorize(id, allowed)
}

#[tauri::command]
pub(super) fn gui_force_close(state: State<'_, GuiBridge>) {
    state.runtime.cancel_authorization();
    state.app.exit(0);
}
