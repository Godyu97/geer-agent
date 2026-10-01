#![cfg(not(feature = "desktop-gui"))]

use std::{
    fs,
    process::{Command, Output},
};

use uuid::Uuid;

mod support;

fn run(command: &mut Command) -> Output {
    support::run(command, b"/exit\n").expect("限时运行临时可执行文件")
}

fn fixture_command(executable: &std::path::Path, dir: &std::path::Path) -> Command {
    let mut command = support::command(executable);
    for (name, _) in std::env::vars_os() {
        let text = name.to_string_lossy();
        if text.starts_with("OPENAI_") || text.starts_with("GEER_AGENT_") {
            command.env_remove(name);
        }
    }
    command.env("HOME", dir).env("USERPROFILE", dir);
    command
}

#[test]
fn executable_env_selects_gui_and_process_override_keeps_repl_available() {
    let dir = std::env::temp_dir().join(format!("geer-ui-selection-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let source = std::path::Path::new(env!("CARGO_BIN_EXE_geer-agent"));
    let executable = dir.join(source.file_name().unwrap());
    // 私有 tmpfs 与构建目录不在同一文件系统；复制才能隔离可执行文件同级的配置。
    fs::copy(source, &executable).unwrap();
    fs::write(
        dir.join(".env"),
        "OPENAI_API_KEY=mock-key\nOPENAI_MODEL=mock-model\nGEER_AGENT_UI=gui\nGEER_AGENT_TOOLS=off\nGEER_AGENT_TRACE=off\nGEER_AGENT_SESSION_PERSISTENCE=off\n",
    )
    .unwrap();

    #[cfg(not(feature = "gui"))]
    {
        let output = run(&mut fixture_command(&executable, &dir));
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("当前构建未包含 GUI") && error.contains("make build"));
    }

    let mut repl = fixture_command(&executable, &dir);
    repl.env("GEER_AGENT_UI", "repl");
    let output = run(&mut repl);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("bye"));
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(not(feature = "web"))]
#[test]
fn web_mode_without_feature_reports_the_build_entry() {
    let mut command = support::command(env!("CARGO_BIN_EXE_geer-agent"));
    command
        .env("GEER_AGENT_UI", "web")
        .env("GEER_AGENT_WEB_PORT", "0");
    let output = support::run(&mut command, b"").unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("当前构建未包含 Web UI") && error.contains("make build"));
}

#[test]
fn explicit_tui_without_terminal_reports_the_requirement() {
    let mut command = support::command(env!("CARGO_BIN_EXE_geer-agent"));
    command.env("GEER_AGENT_UI", "tui");
    let output = support::run(&mut command, b"").unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("TUI 需要交互终端"));
}
