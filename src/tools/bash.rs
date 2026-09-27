use std::{ffi::OsString, io, path::Path, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    task::JoinHandle,
    time::timeout,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CAPTURE_BYTES: usize = 8 * 1024;
pub(super) const MAX_RESULT_CHARS: usize = 2000;

pub(super) async fn run_with_status(bash_bin: &Path, cwd: &Path, command: &str) -> (String, bool) {
    match run_with_timeout_status(bash_bin, cwd, command, COMMAND_TIMEOUT).await {
        Ok(result) => result,
        Err(error) => (format!("Bash 执行失败：{error}"), false),
    }
}

#[cfg(test)]
async fn run_with_timeout(
    bash_bin: &Path,
    cwd: &Path,
    command: &str,
    limit: Duration,
) -> io::Result<String> {
    run_with_timeout_status(bash_bin, cwd, command, limit)
        .await
        .map(|(text, _)| text)
}

async fn run_with_timeout_status(
    bash_bin: &Path,
    cwd: &Path,
    command: &str,
    limit: Duration,
) -> io::Result<(String, bool)> {
    let output = run_command(bash_bin, cwd, command, &[], limit).await?;
    let success = !output.timed_out && output.exit_code == Some(0);
    let mut result = if output.timed_out {
        "命令超过时间限制，已终止。\n".to_owned()
    } else {
        format!(
            "退出状态：{}\n",
            output
                .exit_code
                .map_or_else(|| "由信号终止".to_owned(), |code| code.to_string())
        )
    };
    result.push_str(&String::from_utf8_lossy(&output.stdout.bytes));
    if !output.stderr.bytes.is_empty() {
        if !output.stdout.bytes.is_empty() {
            result.push('\n');
        }
        result.push_str(&String::from_utf8_lossy(&output.stderr.bytes));
    }
    if output.stdout.truncated || output.stderr.truncated {
        result.push_str("\n[输出已截断]");
    }
    Ok((truncate_chars(result, MAX_RESULT_CHARS), success))
}

pub(super) struct CommandOutput {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: Capture,
    pub stderr: Capture,
}

pub(super) async fn run_fixed(
    bash_bin: &Path,
    cwd: &Path,
    script: &str,
    args: &[OsString],
) -> io::Result<CommandOutput> {
    run_command(bash_bin, cwd, script, args, COMMAND_TIMEOUT).await
}

async fn run_command(
    bash_bin: &Path,
    cwd: &Path,
    script: &str,
    args: &[OsString],
    limit: Duration,
) -> io::Result<CommandOutput> {
    let mut command = Command::new(bash_bin);
    command.arg("-c").arg(script);
    if !args.is_empty() {
        // bash -c 后的第一个参数是 $0；用户数据只进入后续位置参数。
        command.arg("geer-file-query").args(args);
    }
    let mut child = command
        .current_dir(cwd)
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("无法捕获标准输出"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("无法捕获标准错误"))?;
    let mut stdout_task = tokio::spawn(read_bounded(stdout));
    let mut stderr_task = tokio::spawn(read_bounded(stderr));
    let status = match timeout(limit, child.wait()).await {
        Ok(result) => Some(result?),
        Err(_) => {
            child.kill().await?;
            None
        }
    };
    Ok(CommandOutput {
        exit_code: status.and_then(|status| status.code()),
        timed_out: status.is_none(),
        stdout: join_capture(&mut stdout_task).await?,
        stderr: join_capture(&mut stderr_task).await?,
    })
}

pub(super) struct Capture {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> io::Result<Capture> {
    let mut bytes = Vec::new();
    let mut block = [0_u8; 4096];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut block).await?;
        if count == 0 {
            break;
        }
        let keep = MAX_CAPTURE_BYTES.saturating_sub(bytes.len()).min(count);
        bytes.extend_from_slice(&block[..keep]);
        truncated |= keep < count;
    }
    Ok(Capture { bytes, truncated })
}

async fn join_capture(task: &mut JoinHandle<io::Result<Capture>>) -> io::Result<Capture> {
    match timeout(Duration::from_secs(1), &mut *task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(io::Error::other(error)),
        Err(_) => {
            task.abort();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "等待命令输出结束超时",
            ))
        }
    }
}

fn truncate_chars(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let marker = "\n[输出已截断]";
    let keep = max.saturating_sub(marker.chars().count());
    text.chars().take(keep).chain(marker.chars()).collect()
}

#[cfg(test)]
mod tests {
    use super::{run_command, run_with_timeout};
    use std::{path::Path, time::Duration};

    #[tokio::test]
    async fn reports_output_exit_status_timeout_and_truncation() {
        let cwd = std::env::current_dir().expect("工作目录存在");
        let success = run_with_timeout(
            Path::new("bash"),
            &cwd,
            "printf hello",
            Duration::from_secs(1),
        )
        .await
        .expect("命令可运行");
        assert!(success.contains("退出状态：0\nhello"));
        let failure = run_with_timeout(Path::new("bash"), &cwd, "exit 7", Duration::from_secs(1))
            .await
            .expect("非零退出也返回结果");
        assert!(failure.contains("退出状态：7"));
        let timeout = run_with_timeout(
            Path::new("bash"),
            &cwd,
            "sleep 1",
            Duration::from_millis(20),
        )
        .await
        .expect("超时返回结果");
        assert!(timeout.contains("已终止"));
        let long = run_with_timeout(
            Path::new("bash"),
            &cwd,
            "yes x | head -c 20000",
            Duration::from_secs(1),
        )
        .await
        .expect("大量输出可运行");
        assert!(long.chars().count() <= 2000);
        assert!(long.contains("输出已截断"));
    }

    #[tokio::test]
    async fn fixed_arguments_keep_literal_values_and_raw_status() {
        let args = ["$(exit 88)", "a ' \" b", "--files"].map(Into::into);
        let result = run_command(
            Path::new("bash"),
            Path::new("."),
            r#"printf '%s\n' "$1" "$2" "$3"; printf 'diagnostic' >&2; exit 7"#,
            &args,
            Duration::from_secs(1),
        )
        .await
        .expect("执行固定脚本");
        assert_eq!(result.exit_code, Some(7));
        assert!(!result.timed_out);
        assert_eq!(
            String::from_utf8(result.stdout.bytes).unwrap(),
            "$(exit 88)\na ' \" b\n--files\n"
        );
        assert_eq!(result.stderr.bytes, b"diagnostic");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn uses_configured_executable() {
        let path = std::env::split_paths(&std::env::var_os("PATH").expect("PATH 存在"))
            .map(|directory| directory.join("false"))
            .find(|path| path.is_file())
            .expect("PATH 中存在 false 可执行文件");
        let cwd = std::env::current_dir().expect("工作目录存在");
        let result = run_with_timeout(&path, &cwd, "printf ignored", Duration::from_secs(1))
            .await
            .expect("执行配置的可执行文件");
        assert!(result.contains("退出状态：1"), "{result}");
        assert!(!result.contains("ignored"), "不得改用默认 Bash：{result}");
    }
}
