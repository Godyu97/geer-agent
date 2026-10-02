#![cfg(all(feature = "web", not(feature = "desktop-gui"), unix))]

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
use tokio_tungstenite::{
    WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;
mod support;

struct Socket {
    inner: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    pending: VecDeque<Value>,
}
const TOKEN: &str = "web-test-password";
struct Fixture {
    dir: PathBuf,
    executable: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("geer-web-{}", Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        // 可执行文件留在构建目录；私有 tmpfs 只承载本轮数据库与 workspace。
        let executable = PathBuf::from(env!("CARGO_BIN_EXE_geer-agent"));
        Self { dir, executable }
    }
    fn configure(&self, command: &mut Command, port: u16, model: &str, persistence: bool) {
        command
            .current_dir(&self.dir)
            .env("HOME", &self.dir)
            .env("GEER_AGENT_UI", "web")
            .env("GEER_AGENT_WEB_PORT", port.to_string())
            .env("GEER_AGENT_WEB_TOKEN", TOKEN)
            .env("OPENAI_API_KEY", "web-mock-key")
            .env("OPENAI_MODEL", "web-mock-model")
            .env("OPENAI_API", "chat-completions")
            .env("OPENAI_BASE_URL", model)
            .env("GEER_AGENT_TRACE", "off")
            .env("GEER_AGENT_TOOLS", "on")
            .env(
                "GEER_AGENT_SESSION_PERSISTENCE",
                if persistence { "on" } else { "off" },
            )
            .env("GEER_AGENT_DATABASE", "sqlite")
            .env(
                "GEER_AGENT_DATABASE_URL",
                format!(
                    "sqlite://{}?mode=rwc",
                    self.dir.join("sessions.sqlite").display()
                ),
            )
            .env("GEER_AGENT_MAX_DURATION_SECONDS", "4")
            .env("GEER_AGENT_BASH_BIN", "/usr/bin/bash");
    }
    fn finite_server(
        &self,
        port: u16,
        model: &str,
        persistence: bool,
    ) -> thread::JoinHandle<Output> {
        self.server(port, model, persistence, false)
    }
    fn server(
        &self,
        port: u16,
        model: &str,
        persistence: bool,
        memory: bool,
    ) -> thread::JoinHandle<Output> {
        let mut command = support::command("/usr/bin/timeout");
        command
            .args([
                "--foreground",
                "--preserve-status",
                "--signal=TERM",
                "--kill-after=2s",
                "8s",
            ])
            .arg(&self.executable);
        self.configure(&mut command, port, model, persistence);
        command.env("GEER_AGENT_MEMORY", if memory { "on" } else { "off" });
        thread::spawn(move || support::run(&mut command, &[]).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}
async fn wait_ready(base: &str, client: &reqwest::Client) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if client.get(format!("{base}/api/auth")).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("HTTP 启动限时");
}
async fn login(base: &str, client: &reqwest::Client) -> String {
    let response = client
        .post(format!("{base}/api/login"))
        .header("origin", base)
        .json(&json!({"token": TOKEN}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly; SameSite=Strict; Path=/"));
    cookie.split(';').next().unwrap().into()
}
async fn connect(
    base: &str,
    cookie: Option<&str>,
    origin: &str,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let mut request = format!("ws{}/api/ws", base.strip_prefix("http").unwrap())
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    if let Some(cookie) = cookie {
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
    }
    tokio::time::timeout(Duration::from_secs(2), connect_async(request))
        .await
        .expect("WS 连接限时")
        .map(|(inner, _)| Socket {
            inner,
            pending: VecDeque::new(),
        })
}
async fn next(socket: &mut Socket, kind: &str) -> Value {
    next_where(socket, |event| event["type"] == kind).await
}
async fn snapshot_after(socket: &mut Socket, old_revision: u64) -> Value {
    next_where(socket, |event| {
        event["type"] == "snapshot" && revision(event) > old_revision
    })
    .await
}
async fn next_where(socket: &mut Socket, matches: impl Fn(&Value) -> bool) -> Value {
    if let Some(index) = socket.pending.iter().position(&matches) {
        return socket.pending.remove(index).unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        for _ in 0..64 {
            match socket
                .inner
                .next()
                .await
                .expect("WS 保持连接")
                .expect("WS 消息")
            {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    if matches(&value) {
                        return value;
                    }
                    socket.pending.push_back(value);
                    assert!(socket.pending.len() <= 128);
                }
                Message::Ping(data) => socket.inner.send(Message::Pong(data)).await.unwrap(),
                Message::Pong(_) => {}
                other => panic!("意外 WS 消息：{other:?}"),
            }
        }
        panic!("未收到目标事件");
    })
    .await
    .expect("WS 事件限时")
}
async fn send(socket: &mut Socket, value: Value) {
    tokio::time::timeout(
        Duration::from_secs(2),
        socket.inner.send(Message::Text(value.to_string().into())),
    )
    .await
    .unwrap()
    .unwrap();
}
async fn submit(socket: &mut Socket, id: i64, line: &str, revision: u64) -> Value {
    send(
        socket,
        json!({"type":"submit", "request_id":id, "line":line, "revision":revision}),
    )
    .await;
    let reply = next(socket, "reply").await;
    assert_eq!(reply["request_id"], id);
    reply
}
fn mock_model(tools: bool) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut bodies = vec![];
        for index in 0..if tools { 4 } else { 1 } {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("模型接受：{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            bodies.push(read_body(&mut stream));
            let chunk = |delta: Value, finish: Value| {
                format!(
                    "data: {}\n\n",
                    json!({"id":"chat_web", "object":"chat.completion.chunk", "created":0, "model":"web-mock-model", "choices":[{"index":0, "delta":delta, "finish_reason":finish}]})
                )
            };
            let body = if tools && index % 2 == 0 {
                chunk(json!({"content":"预先正文"}), Value::Null)
                    + &chunk(
                        json!({"tool_calls":[{"index":0,"id":"bash_web","type":"function","function":{"name":"bash","arguments":json!({"command":"printf web-safe"}).to_string()}}]}),
                        json!("tool_calls"),
                    )
            } else {
                chunk(json!({"content":"完成正文"}), json!("stop"))
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        bodies
    });
    (url, server)
}
fn read_body(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 64 * 1024);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
    };
    let header = String::from_utf8_lossy(&bytes);
    let length: usize = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|value| value.trim().parse().unwrap())
        })
        .unwrap();
    assert!(length <= 1024 * 1024);
    bytes.resize(header_end + length, 0);
    stream.read_exact(&mut bytes[header_end..]).unwrap();
    serde_json::from_slice(&bytes[header_end..]).unwrap()
}
fn revision(snapshot: &Value) -> u64 {
    snapshot["snapshot"]["revision"].as_u64().unwrap()
}

#[test]
fn project_memory_web_syncs_full_crud_and_rejects_private_stale_or_wrong_confirmations() {
    let fixture = Fixture::new();
    fs::write(fixture.dir.join("AGENTS.md"), "WEB_ROOT_RULE").unwrap();
    let port = port();
    let server = fixture.server(port, "http://127.0.0.1:9/v1", true, true);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let base = format!("http://127.0.0.1:{port}");
            let http = http_client();
            wait_ready(&base, &http).await;
            let cookie = login(&base, &http).await;
            let mut desktop = connect(&base, Some(&cookie), &base).await.unwrap();
            let mut phone = connect(&base, Some(&cookie), &base).await.unwrap();
            let initial = next(&mut desktop, "sync").await;
            next(&mut phone, "sync").await;
            let mut rev = initial["state"]["snapshot"]["revision"].as_u64().unwrap();
            let session_id = initial["state"]["snapshot"]["status"]["session_id"].clone();
            assert_eq!(
                initial["state"]["snapshot"]["status"]["memory"]["state"],
                "ready"
            );
            assert_eq!(
                initial["state"]["snapshot"]["status"]["instructions_loaded"],
                true
            );
            assert!(
                submit(&mut desktop, 1, "/memory clear --yes", rev).await["error"]
                    .as_str()
                    .unwrap()
                    .contains("确认已过期")
            );
            assert!(
                submit(&mut desktop, 2, "/memory add 用户偏好 Rust\n完整多行", rev).await["error"]
                    .is_null()
            );
            let saved = snapshot_after(&mut desktop, rev).await;
            rev = revision(&saved);
            let target = saved["snapshot"]["memories"][0]["id"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(saved["snapshot"]["status"]["memory"]["count"], 1);
            let mirrored = snapshot_after(&mut phone, rev - 1).await;
            assert_eq!(
                mirrored["snapshot"]["memories"],
                saved["snapshot"]["memories"]
            );
            assert_eq!(
                mirrored["snapshot"]["memories"][0]["content"],
                "用户偏好 Rust\n完整多行"
            );
            assert!(submit(&mut desktop, 3, "/memory clear", rev).await["error"].is_null());
            rev = revision(&snapshot_after(&mut desktop, rev).await);
            let preview = next(&mut desktop, "memory_confirmation").await;
            assert_eq!(preview["preview"]["count"], 1);
            assert_eq!(preview["preview"]["action"]["kind"], "clear");
            assert_eq!(preview["revision"], rev);
            let phone_preview = snapshot_after(&mut phone, rev - 1).await;
            assert!(phone_preview.get("memory_confirmation").is_none());
            assert!(
                submit(&mut phone, 4, "/memory clear --yes", rev).await["error"]
                    .as_str()
                    .unwrap()
                    .contains("确认已过期")
            );
            assert!(
                submit(&mut phone, 5, "/memory add 新增的另一条", rev).await["error"].is_null()
            );
            rev = revision(&snapshot_after(&mut phone, rev).await);
            assert!(
                submit(&mut desktop, 6, "/memory clear --yes", rev - 1).await["error"]
                    .as_str()
                    .unwrap()
                    .contains("状态已改变")
            );
            assert!(
                submit(&mut desktop, 7, "/memory clear --yes", rev).await["error"]
                    .as_str()
                    .unwrap()
                    .contains("确认已过期")
            );
            assert!(
                submit(&mut desktop, 8, &format!("/memory delete {target}"), rev).await["error"]
                    .is_null()
            );
            rev = revision(&snapshot_after(&mut desktop, rev).await);
            let preview = next(&mut desktop, "memory_confirmation").await;
            assert_eq!(preview["preview"]["entries"][0]["id"], target);
            assert!(
                submit(&mut desktop, 9, "/memory clear --yes", rev).await["error"]
                    .as_str()
                    .unwrap()
                    .contains("确认已过期")
            );
            assert!(
                submit(
                    &mut desktop,
                    10,
                    &format!("/memory delete --yes {target}"),
                    rev
                )
                .await["error"]
                    .is_null()
            );
            let deleted = snapshot_after(&mut desktop, rev).await;
            rev = revision(&deleted);
            assert_eq!(deleted["snapshot"]["memories"].as_array().unwrap().len(), 1);
            let remaining = deleted["snapshot"]["memories"][0]["id"].as_str().unwrap();
            assert!(
                submit(
                    &mut desktop,
                    11,
                    &format!("/memory edit {remaining} 中文 RUST 新事实"),
                    rev
                )
                .await["error"]
                    .is_null()
            );
            let edited = snapshot_after(&mut desktop, rev).await;
            rev = revision(&edited);
            assert_eq!(edited["snapshot"]["memories"][0]["id"], remaining);
            assert_eq!(
                edited["snapshot"]["memories"][0]["content"],
                "中文 RUST 新事实"
            );
            assert!(
                submit(&mut desktop, 12, "/memory search rust 中文", rev).await["error"].is_null()
            );
            let searched = snapshot_after(&mut desktop, rev).await;
            rev = revision(&searched);
            assert!(
                searched["notice"]
                    .as_str()
                    .unwrap()
                    .contains("中文 RUST 新事实")
            );
            assert!(submit(&mut desktop, 13, "/memory clear", rev).await["error"].is_null());
            rev = revision(&snapshot_after(&mut desktop, rev).await);
            next(&mut desktop, "memory_confirmation").await;
            assert!(submit(&mut desktop, 14, "/memory clear --yes", rev).await["error"].is_null());
            let cleared = snapshot_after(&mut desktop, rev).await;
            rev = revision(&cleared);
            assert_eq!(cleared["snapshot"]["memories"], json!([]));
            assert_eq!(cleared["snapshot"]["status"]["memory"]["count"], 0);
            assert_eq!(cleared["snapshot"]["status"]["session_id"], session_id);
            assert_eq!(cleared["snapshot"]["status"]["instructions_loaded"], true);
            assert_eq!(cleared["snapshot"]["transcript"], json!([]));
            let mirrored = snapshot_after(&mut phone, rev - 1).await;
            assert_eq!(mirrored["snapshot"]["memories"], json!([]));
            assert!(
                !phone
                    .pending
                    .iter()
                    .any(|event| event["type"] == "memory_confirmation")
            );
            let mut reconnected = connect(&base, Some(&cookie), &base).await.unwrap();
            let restored = next(&mut reconnected, "sync").await;
            assert_eq!(
                restored["state"]["snapshot"]["status"]["memory"]["count"],
                0
            );
            desktop.inner.close(None).await.unwrap();
            phone.inner.close(None).await.unwrap();
            reconnected.inner.close(None).await.unwrap();
        });
    let output = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn authenticated_browsers_share_stream_authorization_sessions_and_page_exit() {
    let fixture = Fixture::new();
    let port = port();
    let (model, model_server) = mock_model(true);
    let server = fixture.finite_server(port, &model, true);
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let base = format!("http://127.0.0.1:{port}");
        let client = http_client();
        wait_ready(&base, &client).await;
        assert_eq!(client.get(format!("{base}/api/auth")).send().await.unwrap().status(), 401);
        let page = client.get(&base).send().await.unwrap();
        assert!(page.headers()["content-security-policy"].to_str().unwrap().contains("frame-ancestors 'none'"));
        let html = page.text().await.unwrap();
        assert!(html.contains("/assets/"));
        assert!(!html.contains(TOKEN));
        assert_eq!(client.post(format!("{base}/api/login")).header("origin", "http://evil.example").json(&json!({"token": TOKEN})).send().await.unwrap().status(), 403);
        assert_eq!(client.post(format!("{base}/api/login")).header("origin", &base).json(&json!({"token": "wrong"})).send().await.unwrap().status(), 401);
        let cookie = login(&base, &client).await;
        assert_eq!(client.get(format!("{base}/api/auth")).header("cookie", &cookie).send().await.unwrap().status(), 204);
        assert!(matches!(connect(&base, None, &base).await, Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status() == 401));
        assert!(matches!(connect(&base, Some(&cookie), "http://evil.example").await, Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status() == 403));
        let mut desktop = connect(&base, Some(&cookie), &base).await.unwrap();
        let mut phone = connect(&base, Some(&cookie), &base).await.unwrap();
        let initial = next(&mut desktop, "sync").await;
        let initial_revision = initial["state"]["snapshot"]["revision"].as_u64().unwrap();
        next(&mut phone, "sync").await;
        assert!(submit(&mut desktop, 1, "execute safe command", initial_revision).await["error"].is_null());
        let auth = next(&mut desktop, "authorization").await;
        let auth_id = auth["id"].as_u64().unwrap();
        let mut joined = connect(&base, Some(&cookie), &base).await.unwrap();
        let recovery = next(&mut joined, "sync").await;
        assert!(recovery["state"]["running"]["text"].as_str().unwrap().contains("预先正文"));
        assert_eq!(recovery["state"]["authorization"]["id"], auth_id);
        assert!(!recovery.to_string().contains("web-mock-key"));
        assert!(submit(&mut phone, 2, "competing", initial_revision).await["error"].as_str().unwrap().contains("仍在执行"));
        send(&mut joined, json!({"type":"authorize","request_id":3,"id":auth_id,"allowed":true})).await;
        assert!(next(&mut joined, "reply").await["error"].is_null());
        send(&mut desktop, json!({"type":"authorize","request_id":4,"id":auth_id,"allowed":false})).await;
        assert!(next(&mut desktop, "reply").await["error"].as_str().unwrap().contains("过期"));
        let completed = snapshot_after(&mut desktop, initial_revision).await;
        let mut rev = revision(&completed);
        assert!(completed["snapshot"]["transcript"].to_string().contains("完成正文"));
        assert!(submit(&mut phone, 5, "/new", initial_revision).await["error"].as_str().unwrap().contains("状态已改变"));
        let current = completed["snapshot"]["status"]["session_id"].as_str().unwrap();
        assert!(submit(&mut desktop, 6, &format!("/delete {current}"), rev).await["error"].is_null());
        let preview_snapshot = snapshot_after(&mut desktop, rev).await;
        rev = revision(&preview_snapshot);
        assert_eq!(next(&mut desktop, "delete_confirmation").await["revision"], rev);
        assert!(submit(&mut phone, 7, "/save", rev).await["error"].is_null());
        rev = revision(&snapshot_after(&mut phone, rev).await);
        assert!(submit(&mut desktop, 8, &format!("/delete --yes {current}"), rev - 1).await["error"].as_str().unwrap().contains("状态已改变"));
        assert!(submit(&mut desktop, 9, &format!("/delete --yes {current}"), rev).await["error"].as_str().unwrap().contains("确认已过期"));
        assert!(submit(&mut desktop, 10, &"x".repeat(65_537), rev).await["error"].as_str().unwrap().contains("65536"));
        assert!(submit(&mut desktop, 11, &format!("/delete {current}"), rev).await["error"].is_null());
        rev = revision(&snapshot_after(&mut desktop, rev).await);
        next(&mut desktop, "delete_confirmation").await;
        assert!(submit(&mut desktop, 12, &format!("/delete --yes {current}"), rev).await["error"].is_null());
        let deleted = snapshot_after(&mut desktop, rev).await;
        rev = revision(&deleted);
        assert_eq!(deleted["delete_report"]["items"][0]["state"], "deleted");
        assert_ne!(deleted["snapshot"]["status"]["session_id"], current);
        let workspace = fixture.dir.join("other workspace"); fs::create_dir(&workspace).unwrap();
        assert!(submit(&mut desktop, 13, &format!("/workspace {}", workspace.display()), rev).await["error"].is_null());
        let switched = snapshot_after(&mut desktop, rev).await; rev = revision(&switched);
        assert_eq!(switched["snapshot"]["status"]["workspace"], workspace.to_str().unwrap());
        assert!(submit(&mut desktop, 14, "/exit", rev).await["error"].is_null());
        let saved = snapshot_after(&mut desktop, rev).await; rev = revision(&saved);
        next(&mut desktop, "client_exited").await;
        assert!(submit(&mut phone, 15, "/help", rev).await["error"].is_null());
        assert!(snapshot_after(&mut phone, rev).await["notice"].as_str().unwrap().contains("可用命令"));
        let mut extra = vec![];
        for _ in 0..14 {
            let mut socket = connect(&base, Some(&cookie), &base).await.unwrap(); next(&mut socket, "sync").await; extra.push(socket);
        }
        assert!(matches!(connect(&base, Some(&cookie), &base).await, Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status() == 429));
        for mut socket in extra { socket.inner.close(None).await.unwrap(); }
        send(&mut joined, json!({"type":"submit", "request_id":20, "line":"x".repeat(512*1024+1), "revision":rev})).await;
        tokio::time::timeout(Duration::from_secs(2), async { while let Some(Ok(message)) = joined.inner.next().await { if matches!(message, Message::Close(_)) { break; } } }).await.unwrap();
        let logout = client.post(format!("{base}/api/logout")).header("origin", &base).header("cookie", &cookie).send().await.unwrap();
        assert_eq!(logout.status(), 204);
        assert_eq!(client.get(format!("{base}/api/auth")).header("cookie", &cookie).send().await.unwrap().status(), 401);
        phone.inner.close(None).await.unwrap();
        let cookie = login(&base, &client).await;
        let mut alone = connect(&base, Some(&cookie), &base).await.unwrap();
        let recovered = next(&mut alone, "sync").await;
        let rev = recovered["state"]["snapshot"]["revision"].as_u64().unwrap();
        assert!(submit(&mut alone, 21, "deny when disconnected", rev).await["error"].is_null());
        next(&mut alone, "authorization").await;
        alone.inner.close(None).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut reconnected = connect(&base, Some(&cookie), &base).await.unwrap();
        let state = next(&mut reconnected, "sync").await;
        assert!(state["state"]["authorization"].is_null());
        let transcript = if state["state"]["running"].is_null() { state["state"]["snapshot"]["transcript"].clone() } else { next(&mut reconnected, "snapshot").await["snapshot"]["transcript"].clone() };
        assert!(transcript.to_string().contains("用户拒绝授权"));
        reconnected.inner.close(None).await.unwrap();
    });
    let output = server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(TOKEN));
    let bodies = model_server.join().unwrap();
    assert_eq!(bodies.len(), 4);
    assert!(
        bodies[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["role"] == "tool"
                && message["content"].as_str().unwrap().contains("web-safe"))
    );
}

#[test]
fn failed_page_save_keeps_service_available_and_signal_reports_failure() {
    let fixture = Fixture::new();
    let port = port();
    let (model, model_server) = mock_model(false);
    let server = fixture.finite_server(port, &model, false);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let base = format!("http://127.0.0.1:{port}");
            let client = http_client();
            wait_ready(&base, &client).await;
            let cookie = login(&base, &client).await;
            let mut socket = connect(&base, Some(&cookie), &base).await.unwrap();
            let initial = next(&mut socket, "sync").await;
            let mut rev = initial["state"]["snapshot"]["revision"].as_u64().unwrap();
            assert!(submit(&mut socket, 1, "unsaved conversation", rev).await["error"].is_null());
            rev = revision(&snapshot_after(&mut socket, rev).await);
            assert!(submit(&mut socket, 2, "/exit", rev).await["error"].is_null());
            rev = revision(&snapshot_after(&mut socket, rev).await);
            let failed = next(&mut socket, "close_failed").await;
            assert_eq!(failed["can_retry"], false);
            assert!(failed["report"].as_str().unwrap().contains("仅内存"));
            assert!(submit(&mut socket, 3, "/help", rev).await["error"].is_null());
            assert!(
                snapshot_after(&mut socket, rev).await["notice"]
                    .as_str()
                    .unwrap()
                    .contains("可用命令")
            );
            socket.inner.close(None).await.unwrap();
        });
    let output = server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("退出时保存失败"));
    assert_eq!(model_server.join().unwrap().len(), 1);
}

#[test]
fn invalid_port_missing_model_and_occupied_port_fail_clearly() {
    let fixture = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let occupied = listener.local_addr().unwrap().port();
    for (port, key, expected) in [
        ("0".into(), "web-mock-key", "GEER_AGENT_WEB_PORT"),
        (occupied.to_string(), "web-mock-key", "无法监听"),
        (port().to_string(), "", "OPENAI_API_KEY"),
    ] {
        let mut command = support::command(&fixture.executable);
        fixture.configure(&mut command, occupied, "http://127.0.0.1:1/v1", false);
        command
            .env("GEER_AGENT_WEB_PORT", port)
            .env("OPENAI_API_KEY", key);
        let output = support::run(&mut command, &[]).unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
