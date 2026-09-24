use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
enum Reply {
    Error,
    Completed,
    Partial,
}

fn run_repl(replies: Vec<Reply>, input: &str) -> (Output, Vec<String>) {
    run_repl_with_trace(replies, input, None)
}

fn run_repl_with_trace(
    replies: Vec<Reply>,
    input: &str,
    trace_url: Option<&str>,
) -> (Output, Vec<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定测试端口");
    listener.set_nonblocking(true).expect("设置非阻塞监听");
    let url = format!("http://{}/v1", listener.local_addr().expect("读取测试端口"));
    let server = thread::spawn(move || {
        let mut bodies = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(8);
        for reply in replies {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "等待模型请求超时");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("接收模型请求失败：{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("设置读取超时");
            bodies.push(read_body(&mut stream));
            write_reply(&mut stream, reply);
        }
        bodies
    });

    let mut command = Command::new(env!("CARGO_BIN_EXE_geer-agent"));
    command
        .env("OPENAI_API_KEY", "test-key")
        .env("OPENAI_MODEL", "test-model")
        .env("OPENAI_BASE_URL", url)
        .env("OPENAI_API", "responses")
        .env("GEER_AGENT_TOOLS", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(trace_url) = trace_url {
        command
            .env("GEER_AGENT_TRACE_DATABASE", "sqlite")
            .env("GEER_AGENT_TRACE_DATABASE_URL", trace_url);
    }
    let mut child = command.spawn().expect("启动 REPL");
    child
        .stdin
        .take()
        .expect("获取标准输入")
        .write_all(input.as_bytes())
        .expect("写入对话");
    let output = child.wait_with_output().expect("等待 REPL 退出");
    let bodies = server.join().expect("等待模拟服务结束");
    (output, bodies)
}

fn read_body(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    let header_end = loop {
        let count = stream.read(&mut chunk).expect("读取请求头");
        assert!(count > 0, "请求头提前结束");
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(pos) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().expect("Content-Length 合法"))
        })
        .expect("请求包含 Content-Length");
    while bytes.len() - header_end < length {
        let count = stream.read(&mut chunk).expect("读取请求体");
        assert!(count > 0, "请求体提前结束");
        bytes.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8(bytes[header_end..header_end + length].to_vec()).expect("请求体为 UTF-8")
}

fn write_reply(stream: &mut TcpStream, reply: Reply) {
    let (status, content_type, body) = match reply {
        Reply::Error => (
            "500 Internal Server Error",
            "application/json",
            r#"{"error":{"message":"temporary failure","type":"server_error"}}"#.to_owned(),
        ),
        Reply::Completed => (
            "200 OK",
            "text/event-stream",
            format!("{}{}data: [DONE]\n\n", delta_event(), completed_event()),
        ),
        Reply::Partial => ("200 OK", "text/event-stream", delta_event()),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("发送模拟响应");
}

fn delta_event() -> String {
    "data: {\"type\":\"response.output_text.delta\",\"sequence_number\":1,\"item_id\":\"msg_1\",\"output_index\":0,\"content_index\":0,\"delta\":\"hello\"}\n\n".to_owned()
}

fn completed_event() -> String {
    "data: {\"type\":\"response.completed\",\"sequence_number\":2,\"response\":{\"created_at\":0,\"completed_at\":0,\"id\":\"resp_1\",\"model\":\"test-model\",\"object\":\"response\",\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"annotations\":[],\"text\":\"hello\"}],\"id\":\"msg_1\",\"role\":\"assistant\",\"status\":\"completed\"}],\"status\":\"completed\"}}\n\n".to_owned()
}

#[test]
fn retries_then_succeeds_and_keeps_completed_history() {
    let (output, bodies) = run_repl(
        vec![
            Reply::Error,
            Reply::Error,
            Reply::Completed,
            Reply::Completed,
        ],
        "first\nsecond\n/exit\n",
    );
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("标准输出为 UTF-8");
    assert!(
        stdout.contains("Ai › retry 1/5...\nretry 2/5...\nhello"),
        "stdout: {stdout}\nbodies: {bodies:?}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout.contains("retry 3/5..."));
    assert_eq!(bodies.len(), 4);
    assert_eq!(bodies[0], bodies[1]);
    assert_eq!(bodies[1], bodies[2]);
    assert!(bodies[3].contains("\"text\":\"hello\""), "{}", bodies[3]);
}

#[test]
fn stops_after_five_retries_and_reports_final_error() {
    let mut replies = vec![Reply::Error; 6];
    replies.push(Reply::Completed);
    let (output, bodies) = run_repl(replies, "first\nsecond\n/exit\n");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("标准输出为 UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("标准错误为 UTF-8");
    for number in 1..=5 {
        assert!(stdout.contains(&format!("retry {number}/5...")), "{stdout}");
    }
    assert!(!stdout.contains("retry 6/5..."));
    assert!(stderr.contains("模型请求失败"), "{stderr}");
    assert!(stdout.contains("bye"));
    assert_eq!(bodies.len(), 7);
    assert!(
        bodies[6].contains("\"content\":\"second\""),
        "{}",
        bodies[6]
    );
    assert!(!bodies[6].contains("first"), "{}", bodies[6]);
}

#[test]
fn partial_text_failure_is_not_retried() {
    let (output, bodies) = run_repl(vec![Reply::Partial], "first\n/exit\n");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("标准输出为 UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("标准错误为 UTF-8");
    assert!(stdout.contains("Ai › hello"), "{stdout}");
    assert!(!stdout.contains("retry"), "{stdout}");
    assert!(stderr.contains("模型请求失败"), "{stderr}");
    assert_eq!(bodies.len(), 1);
}

#[tokio::test]
async fn trace_retries_once_logically_and_retains_partial_failure() {
    let path = std::env::temp_dir().join(format!("geer-trace-retry-{}.sqlite", Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let (output, bodies) = run_repl_with_trace(
        vec![Reply::Error, Reply::Error, Reply::Completed, Reply::Partial],
        "first\nsecond\n/exit\n",
        Some(&url),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(bodies.len(), 4);
    let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT request_id, request, response, status, attempts, error FROM llm_traces",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let first_request: Value = serde_json::from_str(&bodies[0]).unwrap();
    let second_request: Value = serde_json::from_str(&bodies[3]).unwrap();
    let first = rows
        .iter()
        .find(|row| row.try_get::<Value>("", "request").unwrap() == first_request)
        .unwrap();
    let second = rows
        .iter()
        .find(|row| row.try_get::<Value>("", "request").unwrap() == second_request)
        .unwrap();
    assert_ne!(
        first.try_get::<String>("", "request_id").unwrap(),
        second.try_get::<String>("", "request_id").unwrap()
    );
    assert_eq!(first.try_get::<i32>("", "attempts").unwrap(), 3);
    assert_eq!(first.try_get::<String>("", "status").unwrap(), "completed");
    assert_eq!(second.try_get::<i32>("", "attempts").unwrap(), 1);
    assert_eq!(second.try_get::<String>("", "status").unwrap(), "failed");
    assert_eq!(
        second.try_get::<Value>("", "response").unwrap()["text"],
        "hello"
    );
    assert!(
        !second
            .try_get::<String>("", "error")
            .unwrap()
            .contains("test-key")
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}
