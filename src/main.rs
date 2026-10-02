// 子系统必须在链接时选择；桌面构建从启动起就不创建控制台，普通构建保留终端能力。
#![cfg_attr(all(windows, feature = "desktop-gui"), windows_subsystem = "windows")]

mod agent;
mod config;
mod dao;
mod interaction;
mod memory;
mod prompt;
mod provider;
mod retrieval;
mod session;
mod tools;
mod trace;
mod ui;

#[cfg(all(test, unix))]
#[path = "../tests/support/mod.rs"]
mod test_support;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ui::run()
}
