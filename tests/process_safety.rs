#![cfg(unix)]

use std::{
    fs, io, thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

mod support;

#[test]
fn empty_input_is_eof_without_loading_startup_files() {
    let dir = std::env::temp_dir().join(format!("geer-process-startup-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    fs::write(
        dir.join(".bashrc"),
        "printf 'unexpected-startup'; exit 88\n",
    )
    .unwrap();
    let mut command = support::command("bash");
    command.env("HOME", &dir).args([
        "--noprofile",
        "--norc",
        "-c",
        "if read -r value; then exit 99; fi; printf eof",
    ]);
    let output = support::run(&mut command, &[]).unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"eof");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn blocked_input_write_is_included_in_timeout() {
    let mut command = support::command("bash");
    command.args(["--noprofile", "--norc", "-c", "sleep 2"]);
    let input = vec![b'x'; 1024 * 1024];
    let start = Instant::now();
    let error =
        support::run_with_timeout(&mut command, &input, Duration::from_millis(100)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn timeout_and_normal_exit_clean_up_background_children() {
    let dir = std::env::temp_dir().join(format!("geer-process-cleanup-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    for wait in [true, false] {
        let marker = dir.join(if wait { "timeout" } else { "normal-exit" });
        let mut command = support::command("bash");
        command.args(["--noprofile", "--norc", "-c"]);
        let script = if wait {
            r#"(sleep 1; printf leaked > "$1") & printf started; wait"#
        } else {
            r#"(sleep 1; printf leaked > "$1") & printf started"#
        };
        command.arg(script).arg("test-cleanup").arg(&marker);
        let start = Instant::now();
        let result = support::run_with_timeout(&mut command, &[], Duration::from_millis(300));
        if wait {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        } else {
            let output = result.unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"started");
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(1100));
        assert!(!marker.exists(), "后代进程不应在测试结束后继续写文件");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn excessive_output_fails_without_unbounded_capture() {
    let mut command = support::command("bash");
    // 有限的 5 MiB 输出验证捕获上限，不使用持续输出或递归进程。
    command.args(["--noprofile", "--norc", "-c", "head -c 5242880 /dev/zero"]);
    let error = support::run(&mut command, &[]).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
