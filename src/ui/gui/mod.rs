//! Tauri 窗口与独立 Agent 工作线程；终端入口仍由 ui::run 分派。

mod bridge;

use std::error::Error;
use tauri::{Manager, WindowEvent};

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            bridge::gui_connect,
            bridge::gui_submit,
            bridge::gui_authorize,
            bridge::gui_force_close,
        ])
        .setup(|app| {
            app.manage(bridge::GuiBridge::start(app.handle().clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.state::<bridge::GuiBridge>().request_close();
            }
        })
        .run(tauri::generate_context!("src/ui/gui/tauri.conf.json"))?;
    Ok(())
}
