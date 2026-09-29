use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::Duration,
};

use uuid::Uuid;

fn layout() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("geer-config-paths-{}", Uuid::new_v4()));
    let bin = root.join("bin");
    let home = root.join("home");
    let cwd = root.join("cwd");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(home.join(".geer-agent")).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let source = Path::new(env!("CARGO_BIN_EXE_geer-agent"));
    let executable = bin.join(source.file_name().unwrap());
    fs::copy(source, &executable).unwrap();
    (root, executable, home, cwd)
}

fn run(executable: &Path, cwd: &Path, home: &Path, overrides: &[(&str, &str)]) -> Output {
    let mut command = Command::new(executable);
    for (name, _) in std::env::vars_os() {
        let name_text = name.to_string_lossy();
        if name_text.starts_with("OPENAI_") || name_text.starts_with("GEER_AGENT_") {
            command.env_remove(name);
        }
    }
    command
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("GEER_AGENT_TOOLS", "off")
        .env("GEER_AGENT_TRACE", "on")
        .env("GEER_AGENT_SESSION_PERSISTENCE", "on")
        .env("GEER_AGENT_DATABASE", "sqlite")
        .env("GEER_AGENT_DATABASE_URL", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for &(name, value) in overrides {
        command.env(name, value);
    }
    let mut retries = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && retries < 20 => {
                // 刚复制好的可执行文件在部分文件系统上会短暂拒绝执行。
                retries += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("启动临时可执行文件失败：{error}"),
        }
    };
    child.stdin.take().unwrap().write_all(b"/exit\n").unwrap();
    child.wait_with_output().unwrap()
}

fn assert_started(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Session ID:"));
}

#[test]
fn executable_env_wins_and_invalid_file_does_not_fall_back() {
    let (root, executable, home, cwd) = layout();
    let bin = executable.parent().unwrap();
    fs::write(
        bin.join(".env"),
        "OPENAI_API_KEY=mock-key\nOPENAI_MODEL=exe-model\n",
    )
    .unwrap();
    fs::write(
        home.join(".geer-agent/.env"),
        "GEER_AGENT_CONTEXT_WINDOW_TOKENS=invalid\n",
    )
    .unwrap();
    fs::write(
        cwd.join(".env"),
        "GEER_AGENT_CONTEXT_WINDOW_TOKENS=invalid\n",
    )
    .unwrap();

    assert_started(&run(&executable, &cwd, &home, &[]));
    assert!(bin.join(".db/geer.sqlite").is_file());
    assert!(!home.join(".geer-agent/.db").exists());
    assert!(!cwd.join(".db").exists());

    fs::write(bin.join(".env"), "OPENAI_API_KEY='unterminated\n").unwrap();
    assert!(!run(&executable, &cwd, &home, &[]).status.success());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn home_env_is_used_when_executable_has_no_env() {
    let (root, executable, home, cwd) = layout();
    fs::write(
        home.join(".geer-agent/.env"),
        "OPENAI_API_KEY=mock-key\nOPENAI_MODEL=home-model\n",
    )
    .unwrap();
    fs::write(
        cwd.join(".env"),
        "GEER_AGENT_CONTEXT_WINDOW_TOKENS=invalid\n",
    )
    .unwrap();

    assert_started(&run(&executable, &cwd, &home, &[]));
    assert!(home.join(".geer-agent/.db/geer.sqlite").is_file());
    assert!(!executable.parent().unwrap().join(".db").exists());
    assert!(!cwd.join(".db").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn no_external_env_uses_home_as_program_dir() {
    let (root, executable, home, cwd) = layout();
    #[cfg(not(feature = "embed-env"))]
    let overrides = &[
        ("OPENAI_API_KEY", "mock-key"),
        ("OPENAI_MODEL", "process-model"),
    ][..];
    #[cfg(feature = "embed-env")]
    let overrides = &[][..];

    assert_started(&run(&executable, &cwd, &home, overrides));
    assert!(home.join(".geer-agent/.db/geer.sqlite").is_file());
    assert!(!executable.parent().unwrap().join(".db").exists());
    fs::remove_dir_all(root).unwrap();
}
