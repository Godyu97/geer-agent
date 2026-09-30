use std::{
    ffi::OsStr,
    io::{self, Read, Write},
    process::{Child, Command, Output, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(20);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    // 测试不能借由继承的远程会话或启动钩子执行宿主机的 Shell 配置。
    for name in ["BASH_ENV", "ENV", "SSH_CLIENT", "SSH_CONNECTION", "SSH_TTY"] {
        command.env_remove(name);
    }
    command
}

pub fn run(command: &mut Command, input: &[u8]) -> io::Result<Output> {
    run_with_timeout(command, input, PROCESS_TIMEOUT)
}

pub fn run_with_timeout(
    command: &mut Command,
    input: &[u8],
    limit: Duration,
) -> io::Result<Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .stdin(if input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut retries = 0;
    let child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && retries < 20 => {
                // 临时硬链接在部分文件系统上会短暂拒绝执行；重试也必须有上限。
                retries += 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    };
    let mut process = TestProcess {
        child,
        cleaned: false,
    };
    let stdout = capture(
        process
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("缺少 stdout"))?,
    );
    let stderr = capture(
        process
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("缺少 stderr"))?,
    );
    let (input_sender, input_receiver) = mpsc::channel();
    let stdin = process.child.stdin.take();
    let input = input.to_vec();
    // 输入也放到工作线程，避免子进程不读取输入时阻塞主线程的超时与清理。
    thread::spawn(move || {
        let result = stdin.map_or(Ok(()), |mut pipe| pipe.write_all(&input));
        let result = match result {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            other => other,
        };
        let _ = input_sender.send(result);
        // pipe 在发送结果前已释放，REPL 会收到 EOF。
    });

    let deadline = Instant::now() + limit;
    let (mut out, mut err, mut written, mut status) = (None, None, None, None);
    loop {
        receive(&stdout, &mut out)?;
        receive(&stderr, &mut err)?;
        receive(&input_receiver, &mut written)?;
        if status.is_none() {
            status = process.child.try_wait()?;
            if status.is_some() {
                // 主进程正常退出也要清理后代，否则后台进程可能一直占着输出管道。
                process.cleanup();
            }
        }
        if status.is_some() && out.is_some() && err.is_some() && written.is_some() {
            return Ok(Output {
                status: status.ok_or_else(|| io::Error::other("缺少退出状态"))?,
                stdout: out
                    .take()
                    .ok_or_else(|| io::Error::other("缺少 stdout 结果"))?,
                stderr: err
                    .take()
                    .ok_or_else(|| io::Error::other("缺少 stderr 结果"))?,
            });
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "测试子进程或管道超过时间限制",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn capture(pipe: impl Read + Send + 'static) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .and_then(|_| {
                if bytes.len() as u64 > MAX_OUTPUT_BYTES {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "测试输出超过 4 MiB 上限",
                    ))
                } else {
                    Ok(bytes)
                }
            });
        let _ = sender.send(result);
    });
    receiver
}

fn receive<T>(receiver: &Receiver<io::Result<T>>, value: &mut Option<T>) -> io::Result<()> {
    if value.is_none() {
        match receiver.try_recv() {
            Ok(result) => *value = Some(result?),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("测试 I/O 线程意外退出"));
            }
        }
    }
    Ok(())
}

struct TestProcess {
    child: Child,
    cleaned: bool,
}

impl TestProcess {
    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        #[cfg(unix)]
        let mut killer = {
            // 使用绝对路径，缺失命令测试不能让清理程序也受空 PATH 影响。
            let mut killer = Command::new("/bin/kill");
            killer.args(["-KILL", "--", &format!("-{}", self.child.id())]);
            killer
        };
        #[cfg(windows)]
        let mut killer = {
            let mut killer = Command::new("taskkill");
            killer.args(["/T", "/F", "/PID", &self.child.id().to_string()]);
            killer
        };
        #[cfg(any(unix, windows))]
        if let Ok(mut killer) = killer
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            reap(&mut killer);
        }
        let _ = self.child.kill();
        reap(&mut self.child);
    }
}

impl Drop for TestProcess {
    fn drop(&mut self) {
        // 错误返回和 panic 同样经过这里；只依赖超时 future 的 drop 无法清理后代。
        self.cleanup();
    }
}

fn reap(child: &mut Child) {
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                return;
            }
        }
    }
}
