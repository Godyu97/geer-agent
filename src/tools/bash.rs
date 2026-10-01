use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Child,
    task::JoinHandle,
    time::timeout,
};

use crate::config::{background_command, bash_arg};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const KILL_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_GRACE: Duration = Duration::from_secs(1);
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
    if output.lingering {
        result.push_str("\n[后台进程仍占用输出，已停止等待；不要在命令里启动常驻后台进程]");
    }
    Ok((truncate_chars(result, MAX_RESULT_CHARS), success))
}

pub(super) struct CommandOutput {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    /// 命令已结束但遗留的后台进程还占着输出管道；此时只返回已捕获的部分。
    pub lingering: bool,
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

pub(super) async fn run_fixed_captured(
    bash_bin: &Path,
    cwd: &Path,
    script: &str,
    args: &[OsString],
    capture_limit: usize,
) -> io::Result<CommandOutput> {
    run_command_captured(bash_bin, cwd, script, args, COMMAND_TIMEOUT, capture_limit).await
}

async fn run_command(
    bash_bin: &Path,
    cwd: &Path,
    script: &str,
    args: &[OsString],
    limit: Duration,
) -> io::Result<CommandOutput> {
    run_command_captured(bash_bin, cwd, script, args, limit, MAX_CAPTURE_BYTES).await
}

async fn run_command_captured(
    bash_bin: &Path,
    cwd: &Path,
    script: &str,
    args: &[OsString],
    limit: Duration,
    capture_limit: usize,
) -> io::Result<CommandOutput> {
    let mut command = background_command(bash_bin);
    // Windows 命令行由 Git 启动器和 MSYS 运行时重新解析：未加引号的 `*.txt` 会被展开成文件名，
    // 反斜杠会被当作转义吞掉。命令行只放固定的引导脚本，命令与参数一律经环境变量传入。
    let mut prelude = String::new();
    let mut hidden = vec!["GEER_AGENT_SCRIPT".to_owned()];
    // Cygwin 的 fork 让后台子进程脱离 Windows 进程树，taskkill /T 找不到它们；
    // 先记下 bash 的 $$（Cygwin 中即进程组号），超时后用 Bash 自己的 kill 结束整组。
    let pgid_file = cfg!(windows)
        .then(|| std::env::temp_dir().join(format!("geer-bash-{}.pgid", uuid::Uuid::new_v4())));
    if let Some(file) = &pgid_file {
        prelude.push_str("echo $$ >\"$GEER_AGENT_PGID_FILE\" 2>/dev/null; ");
        hidden.push("GEER_AGENT_PGID_FILE".to_owned());
        command.env(
            "GEER_AGENT_PGID_FILE",
            file.to_str()
                .map_or_else(|| file.clone().into_os_string(), |raw| bash_arg(raw).into()),
        );
    }
    if !args.is_empty() {
        let mut params = Vec::with_capacity(args.len());
        for (index, value) in args.iter().enumerate() {
            let name = format!("GEER_AGENT_ARG_{}", index + 1);
            params.push(format!("\"${name}\""));
            command.env(&name, value);
            hidden.push(name);
        }
        prelude.push_str(&format!("set -- {}; ", params.join(" ")));
        // Git Bash 调原生 rg.exe 时会把 `/api/v1` 这类正则改写成 Windows 路径；固定脚本的参数必须原样传递。
        command
            .env("MSYS_NO_PATHCONV", "1")
            .env("MSYS2_ARG_CONV_EXCL", "*");
    }
    // 取消导出后命令派生的子进程看不到这些内部变量；eval 在当前 Shell 中执行，位置参数仍然可见。
    prelude.push_str(&format!(
        "export -n {}; eval \"$GEER_AGENT_SCRIPT\"",
        hidden.join(" ")
    ));
    command
        .arg("-c")
        .arg(prelude)
        .arg("geer-agent-bash")
        .env("GEER_AGENT_SCRIPT", script);
    if let Some(path) = windows_tool_path(bash_bin) {
        command.env("PATH", path);
    }
    #[cfg(unix)]
    command.process_group(0);
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
    let stdout_capture = SharedCapture::default();
    let stderr_capture = SharedCapture::default();
    let mut stdout_task = tokio::spawn(read_bounded(
        stdout,
        Arc::clone(&stdout_capture),
        capture_limit,
    ));
    let mut stderr_task = tokio::spawn(read_bounded(
        stderr,
        Arc::clone(&stderr_capture),
        MAX_CAPTURE_BYTES,
    ));
    let status = match timeout(limit, child.wait()).await {
        Ok(result) => Some(result),
        Err(_) => {
            kill_tree(bash_bin, &mut child, pgid_file.as_deref()).await;
            None
        }
    };
    if let Some(file) = &pgid_file {
        let _ = std::fs::remove_file(file);
    }
    let status = status.transpose()?;
    let (stdout, stdout_lingering) = join_capture(&mut stdout_task, &stdout_capture).await?;
    let (stderr, stderr_lingering) = join_capture(&mut stderr_task, &stderr_capture).await?;
    Ok(CommandOutput {
        exit_code: status.and_then(|status| status.code()),
        timed_out: status.is_none(),
        lingering: stdout_lingering || stderr_lingering,
        stdout,
        stderr,
    })
}

/// 超时后结束整棵进程树：Git 的 `bin\bash.exe` 只是启动器，只杀它会留下真正的 bash 和子命令，
/// 它们继续占着输出管道；Unix 上命令也可能派生后台子进程。
async fn kill_tree(bash_bin: &Path, child: &mut Child, pgid_file: Option<&Path>) {
    if let Some(pgid) = pgid_file
        .and_then(|file| std::fs::read_to_string(file).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
    {
        let _ = timeout(
            KILL_TIMEOUT,
            background_command(bash_bin)
                .args(["-c", r#"kill -KILL -- "-$1""#, "geer-kill", &pgid])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await;
    }
    if let Some(pid) = child.id() {
        #[cfg(windows)]
        let mut killer = {
            let mut killer = background_command("taskkill");
            killer.args(["/T", "/F", "/PID", &pid.to_string()]);
            killer
        };
        #[cfg(not(windows))]
        let mut killer = {
            let mut killer = background_command("kill");
            killer.args(["-KILL", "--", &format!("-{pid}")]);
            killer
        };
        let _ = timeout(
            KILL_TIMEOUT,
            killer
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status(),
        )
        .await;
    }
    // 进程树已被结束时这里只负责回收；失败也不影响向模型报告超时。
    let _ = child.start_kill();
    let _ = timeout(KILL_TIMEOUT, child.wait()).await;
}

/// Windows 从 GUI、make 或已设置 MSYSTEM 的环境启动时，Git 启动器不一定把 `usr\bin` 放进 PATH，
/// 导致 ls/head/find 都找不到；按配置的 bash.exe 位置补上 Git 自带的工具目录。
fn windows_tool_path(bash_bin: &Path) -> Option<OsString> {
    if !cfg!(windows) {
        return None;
    }
    let bin = bash_bin
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())?;
    let root = bin.parent()?;
    let mut dirs: Vec<PathBuf> = [
        root.join("usr").join("bin"),
        root.join("mingw64").join("bin"),
        bin.to_path_buf(),
    ]
    .into_iter()
    .filter(|dir| dir.is_dir())
    .collect();
    if dirs.is_empty() {
        return None;
    }
    if let Some(current) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(dirs).ok()
}

#[derive(Default)]
pub(super) struct Capture {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

// 读取任务与等待方共享缓冲：管道被遗留进程占住时，等待方仍能拿走已读到的部分。
type SharedCapture = Arc<Mutex<Capture>>;

async fn read_bounded(
    mut reader: impl AsyncRead + Unpin,
    capture: SharedCapture,
    capture_limit: usize,
) -> io::Result<()> {
    let mut block = [0_u8; 4096];
    loop {
        let count = reader.read(&mut block).await?;
        if count == 0 {
            return Ok(());
        }
        let mut capture = capture
            .lock()
            .map_err(|_| io::Error::other("输出缓冲锁损坏"))?;
        let keep = capture_limit.saturating_sub(capture.bytes.len()).min(count);
        capture.bytes.extend_from_slice(&block[..keep]);
        capture.truncated |= keep < count;
    }
}

async fn join_capture(
    task: &mut JoinHandle<io::Result<()>>,
    capture: &SharedCapture,
) -> io::Result<(Capture, bool)> {
    let lingering = match timeout(OUTPUT_GRACE, &mut *task).await {
        Ok(Ok(result)) => {
            result?;
            false
        }
        Ok(Err(error)) => return Err(io::Error::other(error)),
        Err(_) => {
            task.abort();
            true
        }
    };
    let mut capture = capture
        .lock()
        .map_err(|_| io::Error::other("输出缓冲锁损坏"))?;
    Ok((std::mem::take(&mut *capture), lingering))
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
    use crate::config::default_bash_bin;
    use std::{path::Path, time::Duration};

    #[tokio::test]
    async fn reports_output_exit_status_timeout_and_truncation() {
        let cwd = std::env::current_dir().expect("工作目录存在");
        let success = run_with_timeout(
            &default_bash_bin(),
            &cwd,
            "printf hello",
            Duration::from_secs(1),
        )
        .await
        .expect("命令可运行");
        assert!(success.contains("退出状态：0\nhello"));
        let failure = run_with_timeout(&default_bash_bin(), &cwd, "exit 7", Duration::from_secs(1))
            .await
            .expect("非零退出也返回结果");
        assert!(failure.contains("退出状态：7"));
        let timeout = run_with_timeout(
            &default_bash_bin(),
            &cwd,
            "sleep 1",
            Duration::from_millis(20),
        )
        .await
        .expect("超时返回结果");
        assert!(timeout.contains("已终止"));
        let long = run_with_timeout(
            &default_bash_bin(),
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
    async fn timeout_ends_background_children_holding_output() {
        let cwd = std::env::current_dir().expect("工作目录存在");
        let started = std::time::Instant::now();
        let result = run_with_timeout(
            &default_bash_bin(),
            &cwd,
            "sleep 30 & sleep 30",
            Duration::from_millis(300),
        )
        .await
        .expect("超时返回结果而不是输出等待错误");
        assert!(result.contains("已终止"), "{result}");
        assert!(started.elapsed() < Duration::from_secs(8));

        let started = std::time::Instant::now();
        let result = run_with_timeout(
            &default_bash_bin(),
            &cwd,
            "sleep 3 & echo done",
            Duration::from_secs(10),
        )
        .await
        .expect("遗留后台进程时仍返回已捕获输出");
        assert!(result.contains("退出状态：0\ndone"), "{result}");
        assert!(result.contains("后台进程仍占用输出"), "{result}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn fixed_arguments_keep_literal_values_and_raw_status() {
        let args = ["$(exit 88)", "a ' \" b", "--files", "*", r"\\?\C:\a\b", ""].map(Into::into);
        let result = run_command(
            &default_bash_bin(),
            Path::new("."),
            r#"printf '[%s]\n' "$@"; printf 'diagnostic' >&2; exit 7"#,
            &args,
            Duration::from_secs(5),
        )
        .await
        .expect("执行固定脚本");
        assert_eq!(result.exit_code, Some(7));
        assert!(!result.timed_out);
        assert_eq!(
            String::from_utf8(result.stdout.bytes).unwrap(),
            "[$(exit 88)]\n[a ' \" b]\n[--files]\n[*]\n[\\\\?\\C:\\a\\b]\n[]\n"
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
