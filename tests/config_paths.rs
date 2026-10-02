#![cfg(not(feature = "desktop-gui"))]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

use uuid::Uuid;

mod support;

fn layout() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!(".test-config-paths-{}", Uuid::new_v4()));
    let bin = root.join("bin");
    let home = root.join("home");
    let cwd = root.join("cwd");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(home.join(".geer-agent")).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let source = Path::new(env!("CARGO_BIN_EXE_geer-agent"));
    let executable = bin.join(source.file_name().unwrap());
    // current_exe 会跟随符号链接回到 target；同文件系统硬链接隔离源码配置，也不复制大二进制。
    fs::hard_link(source, &executable).unwrap();
    (root, executable, home, cwd)
}

fn run(executable: &Path, cwd: &Path, home: &Path, overrides: &[(&str, &str)]) -> Output {
    let mut command = support::command(executable);
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
        .env("GEER_AGENT_MEMORY", "off")
        .env("GEER_AGENT_DATABASE", "sqlite")
        .env("GEER_AGENT_DATABASE_URL", "");
    for &(name, value) in overrides {
        command.env(name, value);
    }
    support::run(&mut command, b"/exit\n").expect("限时运行临时可执行文件")
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
