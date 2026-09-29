use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use uuid::Uuid;

fn body(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut block = [0_u8; 4096];
    let end = loop {
        let count = stream.read(&mut block).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&block[..count]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while bytes.len() - end < length {
        let count = stream.read(&mut block).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&block[..count]);
    }
    serde_json::from_slice(&bytes[end..end + length]).unwrap()
}

fn reply(stream: &mut TcpStream, api: &str, text: &str, index: usize) {
    let body = if api == "chat-completions" {
        let chunk = json!({
            "id": format!("chat_{index}"), "object": "chat.completion.chunk", "created": 0,
            "model": "test-model", "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": "stop"}]
        });
        format!("data: {chunk}\n\n")
    } else {
        let response = json!({
            "created_at": 0, "completed_at": 0, "id": format!("resp_{index}"),
            "model": "test-model", "object": "response", "status": "completed",
            "output": [{"type": "message", "id": format!("msg_{index}"), "role": "assistant",
                "status": "completed", "content": [{"type": "output_text", "annotations": [], "text": text}]}]
        });
        format!(
            "data: {}\n\ndata: {}\n\n",
            json!({"type":"response.output_text.delta", "sequence_number":1, "item_id":format!("msg_{index}"), "output_index":0,"content_index":0,"delta":text}),
            json!({"type": "response.completed", "sequence_number": 2, "response": response})
        )
    };
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
}

fn run(api: &str, base_url: &str, db_url: &str, input: &str) -> Output {
    run_with_model(api, base_url, db_url, "test-model", input)
}

fn run_with_model(api: &str, base_url: &str, db_url: &str, model: &str, input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_geer-agent"))
        .env("OPENAI_API_KEY", "mock-key")
        .env("OPENAI_MODEL", model)
        .env("OPENAI_BASE_URL", base_url)
        .env("OPENAI_API", api)
        .env("GEER_AGENT_TOOLS", "off")
        .env("GEER_AGENT_AUTO_COMPACT", "off")
        .env("GEER_AGENT_TRACE", "off")
        .env("GEER_AGENT_DATABASE", "sqlite")
        .env("GEER_AGENT_DATABASE_URL", db_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn run_default(api: &str, base_url: &str, workspace: &std::path::Path, input: &str) -> Output {
    let source = std::path::Path::new(env!("CARGO_BIN_EXE_geer-agent"));
    let executable = workspace.join("bin").join(source.file_name().unwrap());
    if !executable.exists() {
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::copy(source, &executable).unwrap();
    }
    let mut command = Command::new(executable);
    command
        .current_dir(workspace)
        .env("HOME", workspace)
        .env("USERPROFILE", workspace)
        .env("OPENAI_API_KEY", "mock-key")
        .env("OPENAI_MODEL", "test-model")
        .env("OPENAI_BASE_URL", base_url)
        .env("OPENAI_API", api)
        .env("GEER_AGENT_TOOLS", "off")
        .env("GEER_AGENT_AUTO_COMPACT", "off")
        .env_remove("GEER_AGENT_DATABASE")
        .env_remove("GEER_AGENT_DATABASE_URL")
        .env_remove("GEER_AGENT_TRACE")
        .env_remove("GEER_AGENT_SESSION_PERSISTENCE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn persisted_session_restores_its_workspace_across_processes() {
    for api in ["chat-completions", "responses"] {
        let root = std::env::temp_dir().join(format!("geer-restore-workspace-{}", Uuid::new_v4()));
        let target = root.join("中文 workspace");
        std::fs::create_dir_all(&target).unwrap();
        let database = root.join("sessions.sqlite");
        let database_url = format!("sqlite://{}?mode=rwc", database.display());
        let first = run(
            api,
            "http://127.0.0.1:9/v1",
            &database_url,
            &format!("/workspace \"{}\"\n/exit\n", target.display()),
        );
        assert!(
            first.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        let first_stdout = String::from_utf8(first.stdout).unwrap();
        let saved_id = first_stdout
            .lines()
            .filter_map(|line| line.strip_prefix("Session ID: "))
            .nth(1)
            .expect("切换后的 Session ID");
        let second = run(
            api,
            "http://127.0.0.1:9/v1",
            &database_url,
            &format!("/open {saved_id}\n/workspace\n/exit\n"),
        );
        assert!(
            second.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&second.stderr)
        );
        let second_stdout = String::from_utf8(second.stdout).unwrap();
        assert!(second_stdout.contains("已恢复会话"), "{second_stdout}");
        assert!(
            second_stdout.contains(&format!("Workspace: {}", target.display())),
            "{second_stdout}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

async fn default_database_is_shared_across_sessions_and_restarts(api: &str) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let api_owned = api.to_owned();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        for (index, text) in [
            "A-answer-1",
            "B-answer-1",
            "A-answer-2",
            "A-summary",
            "B-answer-2",
            "A-answer-3",
        ]
        .iter()
        .enumerate()
        {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(value) => break value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "等待第 {} 次请求超时", index + 1);
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("模拟服务连接失败：{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            requests.push(body(&mut stream));
            reply(&mut stream, &api_owned, text, index);
        }
        requests
    });
    let workspace = std::env::temp_dir().join(format!("geer-default-sessions-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();
    let first = run_default(
        api,
        &base_url,
        &workspace,
        "A-first\n/new\nB-first\n/exit\n",
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_stdout = String::from_utf8(first.stdout).unwrap();
    let ids: Vec<_> = first_stdout
        .lines()
        .filter_map(|line| line.strip_prefix("Session ID: "))
        .map(str::to_owned)
        .collect();
    assert_eq!(ids.len(), 2, "{first_stdout}");
    let (a, b) = (&ids[0], &ids[1]);
    assert_ne!(a, b);
    let db_path = workspace.join(".geer-agent/.db/geer.sqlite");
    assert!(
        db_path.is_file(),
        "默认 SQLite 未创建：{}",
        db_path.display()
    );

    let second = run_default(
        api,
        &base_url,
        &workspace,
        &format!(
            "/open {a}\nA-second\n/compact\n/open {b}\nB-second\n/open {a}\nA-third\n/sessions\n/exit\n"
        ),
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_stdout = String::from_utf8(second.stdout).unwrap();
    assert!(second_stdout.contains("已压缩"), "{second_stdout}");
    assert!(
        second_stdout.contains(&format!("* {a}  ")),
        "{second_stdout}"
    );
    assert!(
        second_stdout.contains(&format!("  {b}  ")),
        "{second_stdout}"
    );

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 6);
    let as_text: Vec<_> = requests.iter().map(Value::to_string).collect();
    assert!(
        as_text[2].contains("A-first") && !as_text[2].contains("B-first"),
        "{}",
        as_text[2]
    );
    assert!(
        as_text[3].contains("A-first") && !as_text[3].contains("B-first"),
        "{}",
        as_text[3]
    );
    assert!(
        as_text[4].contains("B-first") && !as_text[4].contains("A-first"),
        "{}",
        as_text[4]
    );
    assert!(
        as_text[5].contains("A-summary") && !as_text[5].contains("B-first"),
        "{}",
        as_text[5]
    );

    let db = Database::connect(format!("sqlite://{}?mode=rw", db_path.display()))
        .await
        .unwrap();
    let sessions = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id FROM agent_sessions".to_owned(),
        ))
        .await
        .unwrap();
    let stored_ids: Vec<String> = sessions
        .iter()
        .map(|row| row.try_get("", "id").unwrap())
        .collect();
    assert!(stored_ids.contains(a) && stored_ids.contains(b));
    let traces = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT session_id, request FROM llm_traces".to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(traces.len(), 6);
    for (index, request) in requests.iter().enumerate() {
        let row = traces
            .iter()
            .find(|row| row.try_get::<Value>("", "request").unwrap() == *request)
            .unwrap();
        let actual_id: String = row.try_get("", "session_id").unwrap();
        assert_eq!(
            &actual_id,
            if [0, 2, 3, 5].contains(&index) { a } else { b }
        );
    }
    drop(db);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[tokio::test]
async fn chat_default_database_shares_session_and_trace() {
    default_database_is_shared_across_sessions_and_restarts("chat-completions").await;
}

#[tokio::test]
async fn responses_default_database_shares_session_and_trace() {
    default_database_is_shared_across_sessions_and_restarts("responses").await;
}

#[tokio::test]
async fn eof_saves_all_default_sessions_without_model_requests() {
    let workspace = std::env::temp_dir().join(format!("geer-eof-sessions-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();
    let output = run_default("responses", "http://127.0.0.1:1/v1", &workspace, "/new\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("bye"));
    let ids: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("Session ID: "))
        .collect();
    assert_eq!(ids.len(), 2);
    let db_path = workspace.join(".geer-agent/.db/geer.sqlite");
    let db = Database::connect(format!("sqlite://{}?mode=rw", db_path.display()))
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id FROM agent_sessions".to_owned(),
        ))
        .await
        .unwrap();
    let stored: Vec<String> = rows
        .iter()
        .map(|row| row.try_get("", "id").unwrap())
        .collect();
    assert!(ids.iter().all(|id| stored.iter().any(|saved| saved == id)));
    drop(db);
    std::fs::remove_dir_all(workspace).unwrap();
}

async fn cross_process(api: &str) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let api_owned = api.to_owned();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        for (index, text) in [
            "first-response",
            "second-response",
            "summary-of-first",
            "continued-response",
        ]
        .iter()
        .enumerate()
        {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(value) => break value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "模拟服务等待第 {} 次请求超时",
                            index + 1
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("模拟服务连接失败：{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            requests.push(body(&mut stream));
            reply(&mut stream, &api_owned, text, index);
        }
        requests
    });
    let path = std::env::temp_dir().join(format!("geer-cross-process-{}.sqlite", Uuid::new_v4()));
    let db_url = format!("sqlite://{}?mode=rwc", path.display());
    let first = run(
        api,
        &base_url,
        &db_url,
        "first fact 111\nsecond fact 222\n/compact\n/sessions\n/exit\n",
    );
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        first_stdout.contains("已压缩"),
        "{first_stdout}\nstderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let id = first_stdout
        .lines()
        .find_map(|line| line.strip_prefix("Session ID: "))
        .unwrap();
    assert!(first_stdout.contains(&format!("{id}  ")), "{first_stdout}");

    let incompatible = run_with_model(
        api,
        &base_url,
        &db_url,
        "different-model",
        &format!("/resume {id}\n/exit\n"),
    );
    let incompatible_stderr = String::from_utf8_lossy(&incompatible.stderr);
    assert!(incompatible.status.success());
    assert!(
        incompatible_stderr.contains("不兼容"),
        "{incompatible_stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&incompatible.stdout)
            .contains(&format!("已恢复会话。\nSession ID: {id}"))
    );

    // 模拟进程在工具批次检查点之后退出；恢复只提示不确定性并等待下一条输入。
    let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
        .await
        .unwrap();
    db.execute_unprepared(&format!(
        "UPDATE agent_sessions SET uncertain_tools = 1 WHERE id = '{id}'"
    ))
    .await
    .unwrap();
    drop(db);

    let second = run(
        api,
        &base_url,
        &db_url,
        &format!("/resume {id}\n/reset\n/resume {id}\nthird question\n/exit\n"),
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(second_stdout.contains("已恢复会话"), "{second_stdout}");
    assert!(second_stdout.contains("已清空对话记忆"), "{second_stdout}");
    assert_eq!(second_stdout.matches("已恢复会话").count(), 2);
    assert!(second_stdout.contains("状态未确认"), "{second_stdout}");
    assert!(
        second_stdout.contains("continued-response"),
        "{second_stdout}"
    );
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 4);
    let continued = requests[3].to_string();
    assert!(continued.contains("summary-of-first"), "{continued}");
    assert!(continued.contains("second fact 222"), "{continued}");
    assert!(continued.contains("third question"), "{continued}");
    assert!(!continued.contains("first fact 111"), "{continued}");
    assert!(!continued.contains("/compact"), "{continued}");
    assert!(!continued.contains("/resume"), "{continued}");
    assert!(!continued.contains("/reset"), "{continued}");

    let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            format!("SELECT kind, payload FROM agent_session_events WHERE session_id = '{id}'"),
        ))
        .await
        .unwrap();
    assert!(
        rows.iter()
            .any(|row| row.try_get::<String>("", "kind").unwrap() == "compaction")
    );
    assert!(rows.iter().any(|row| {
        row.try_get::<Value>("", "payload")
            .unwrap()
            .to_string()
            .contains("first fact 111")
    }));
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn chat_session_compacts_exits_resumes_and_continues() {
    cross_process("chat-completions").await;
}

#[tokio::test]
async fn responses_session_compacts_exits_resumes_and_continues() {
    cross_process("responses").await;
}
