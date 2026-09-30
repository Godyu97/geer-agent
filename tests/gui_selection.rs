#![cfg(not(feature = "gui"))]

use std::{
    fs,
    io::{self, Write},
    process::{Command, Output, Stdio},
    thread,
    time::Duration,
};

use uuid::Uuid;

fn run(command: &mut Command) -> Output {
    let mut retries = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && retries < 20 => {
                retries += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("启动临时可执行文件失败：{error}"),
        }
    };
    let _ = child.stdin.take().unwrap().write_all(b"/exit\n");
    child.wait_with_output().unwrap()
}

#[test]
fn executable_env_selects_gui_and_process_override_keeps_repl_available() {
    let dir = std::env::temp_dir().join(format!("geer-gui-selection-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let source = std::path::Path::new(env!("CARGO_BIN_EXE_geer-agent"));
    let executable = dir.join(source.file_name().unwrap());
    // /tmp 在本机是 tmpfs：复制调试二进制会直接占用 RAM。
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, &executable).unwrap();
    #[cfg(not(unix))]
    fs::copy(source, &executable).unwrap();
    fs::write(
        dir.join(".env"),
        "OPENAI_API_KEY=mock-key\nOPENAI_MODEL=mock-model\nGEER_AGENT_UI=gui\nGEER_AGENT_TRACE=off\nGEER_AGENT_SESSION_PERSISTENCE=off\n",
    )
    .unwrap();

    let mut gui = Command::new(&executable);
    gui.env("HOME", &dir)
        .env_remove("GEER_AGENT_UI")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run(&mut gui);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("当前构建未包含 GUI"));

    let mut repl = Command::new(&executable);
    repl.env("HOME", &dir)
        .env("GEER_AGENT_UI", "repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run(&mut repl);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("bye"));
    fs::remove_dir_all(dir).unwrap();
}
