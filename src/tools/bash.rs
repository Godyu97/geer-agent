use std::{io, path::Path, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    task::JoinHandle,
    time::timeout,
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CAPTURE_BYTES: usize = 8 * 1024;
const MAX_RESULT_CHARS: usize = 2000;

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
    let mut child = Command::new(bash_bin)
        .arg("-c")
        .arg(command)
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
    let stdout = join_capture(&mut stdout_task).await?;
    let stderr = join_capture(&mut stderr_task).await?;
    let success = status.is_some_and(|status| status.success());
    let mut result = if let Some(status) = status {
        format!(
            "退出状态：{}\n",
            status
                .code()
                .map_or_else(|| "由信号终止".to_owned(), |code| code.to_string())
        )
    } else {
        "命令超过时间限制，已终止。\n".to_owned()
    };
    result.push_str(&String::from_utf8_lossy(&stdout.bytes));
    if !stderr.bytes.is_empty() {
        if !stdout.bytes.is_empty() {
            result.push('\n');
        }
        result.push_str(&String::from_utf8_lossy(&stderr.bytes));
    }
    if stdout.truncated || stderr.truncated {
        result.push_str("\n[输出已截断]");
    }
    Ok((truncate_chars(result, MAX_RESULT_CHARS), success))
}

struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
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
    use super::run_with_timeout;
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

    #[cfg(unix)]
    #[tokio::test]
    async fn uses_configured_executable() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let path =
            std::env::temp_dir().join(format!("geer-agent-custom-bash-{}", std::process::id()));
        fs::write(&path, "#!/bin/sh\nprintf custom-bash-selected\n").expect("创建测试可执行文件");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("设置执行权限");
        let cwd = std::env::current_dir().expect("工作目录存在");
        let result = run_with_timeout(&path, &cwd, "printf ignored", Duration::from_secs(1)).await;
        fs::remove_file(&path).expect("清理测试文件");
        assert!(
            result
                .expect("执行自定义文件")
                .contains("custom-bash-selected")
        );
    }
}
