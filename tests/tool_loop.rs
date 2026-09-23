use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

#[derive(Clone)]
enum Reply {
    ResponsesCalls,
    ResponsesFinal,
    ChatCalls(usize),
    ChatBash(String),
    ChatFinal,
    Error,
}

fn run_repl(api: &str, replies: Vec<Reply>, input: &str) -> (Output, Vec<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定模拟服务");
    listener.set_nonblocking(true).expect("非阻塞监听");
    let url = format!("http://{}/v1", listener.local_addr().expect("服务地址"));
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
                    Err(error) => panic!("接受连接失败：{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("请求读取限时");
            bodies.push(read_body(&mut stream));
            write_reply(&mut stream, reply);
        }
        bodies
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_geer-agent"))
        .env("OPENAI_API_KEY", "test-key")
        .env("OPENAI_MODEL", "test-model")
        .env("OPENAI_BASE_URL", url)
        .env("OPENAI_API", api)
        .env("GEER_AGENT_TOOLS", "on")
        .env_remove("GEER_AGENT_BASH_BIN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动 REPL");
    child
        .stdin
        .take()
        .expect("输入管道")
        .write_all(input.as_bytes())
        .expect("写入测试对话");
    let output = child.wait_with_output().expect("等待 REPL");
    let bodies = server.join().expect("等待模拟服务");
    (output, bodies)
}

fn read_body(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    let mut block = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut block).expect("读取请求头");
        assert!(count > 0, "请求头提前结束");
        bytes.extend_from_slice(&block[..count]);
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
                .then(|| value.trim().parse::<usize>().expect("合法长度"))
        })
        .expect("请求长度");
    while bytes.len() - header_end < length {
        let count = stream.read(&mut block).expect("读取请求体");
        assert!(count > 0, "请求体提前结束");
        bytes.extend_from_slice(&block[..count]);
    }
    serde_json::from_slice(&bytes[header_end..header_end + length]).expect("请求 JSON")
}

fn write_reply(stream: &mut TcpStream, reply: Reply) {
    let (status, kind, body) = match reply {
        Reply::Error => (
            "400 Bad Request",
            "application/json",
            r#"{"error":{"message":"boom","type":"invalid_request_error"}}"#.to_owned(),
        ),
        Reply::ResponsesCalls => {
            let response = json!({"created_at":0,"completed_at":0,"id":"resp_1","model":"test-model","object":"response","output":[
                {"type":"function_call","arguments":"{}","call_id":"call_1","name":"get_current_time"},
                {"type":"function_call","arguments":"{}","call_id":"call_2","name":"get_current_time"}
            ],"status":"completed"});
            (
                "200 OK",
                "text/event-stream",
                sse(json!({"type":"response.completed","sequence_number":1,"response":response})),
            )
        }
        Reply::ResponsesFinal => {
            let response = json!({"created_at":0,"completed_at":0,"id":"resp_2","model":"test-model","object":"response","output":[{"type":"message","content":[{"type":"output_text","annotations":[],"text":"done"}],"id":"msg_1","role":"assistant","status":"completed"}],"status":"completed"});
            let mut body = sse(
                json!({"type":"response.output_text.delta","sequence_number":1,"item_id":"msg_1","output_index":0,"content_index":0,"delta":"done"}),
            );
            body.push_str(&sse(
                json!({"type":"response.completed","sequence_number":2,"response":response}),
            ));
            ("200 OK", "text/event-stream", body)
        }
        Reply::ChatCalls(number) => {
            let mut body = String::new();
            body.push_str(&chat_chunk(json!({"tool_calls":[{"index":0,"id":format!("call_{number}"),"type":"function","function":{"name":"get_","arguments":"{"}}]}), Value::Null));
            body.push_str(&chat_chunk(json!({"tool_calls":[{"index":0,"function":{"name":"current_time","arguments":"}"}}]}), json!("tool_calls")));
            ("200 OK", "text/event-stream", body)
        }
        Reply::ChatBash(command) => {
            let args = json!({"command":command}).to_string();
            let body = chat_chunk(
                json!({"tool_calls":[{"index":0,"id":"bash_1","type":"function","function":{"name":"bash","arguments":args}}]}),
                json!("tool_calls"),
            );
            ("200 OK", "text/event-stream", body)
        }
        Reply::ChatFinal => (
            "200 OK",
            "text/event-stream",
            chat_chunk(json!({"content":"done"}), json!("stop")),
        ),
    };
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("发送响应");
}

fn chat_chunk(delta: Value, finish_reason: Value) -> String {
    sse(
        json!({"id":"chat_1","object":"chat.completion.chunk","created":0,"model":"test-model","choices":[{"index":0,"delta":delta,"finish_reason":finish_reason}]}),
    )
}

fn sse(value: Value) -> String {
    format!("data: {value}\n\n")
}

#[test]
fn responses_api_returns_two_tool_results_before_final_answer() {
    let (output, bodies) = run_repl(
        "responses",
        vec![Reply::ResponsesCalls, Reply::ResponsesFinal],
        "time\n/exit\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("调用工具 get_current_time"));
    assert!(stdout.contains("done"));
    assert_eq!(bodies.len(), 2);
    for body in &bodies {
        let prompt = body["instructions"].as_str().expect("默认系统提示");
        assert!(prompt.contains("<context_data>"));
        assert!(prompt.contains("system_version:"));
        assert!(prompt.contains("bash_version: GNU bash, version "));
    }
    assert_eq!(bodies[0]["tools"].as_array().expect("工具清单").len(), 5);
    let input = bodies[1]["input"].as_array().expect("下一请求历史");
    assert_eq!(
        input
            .iter()
            .filter(|item| item["type"] == "function_call_output")
            .count(),
        2
    );
    assert!(
        input
            .iter()
            .any(|item| item["call_id"] == "call_1" && item["type"] == "function_call_output")
    );
}

#[test]
fn chat_api_reassembles_fragments_and_pairs_tool_result() {
    let (output, bodies) = run_repl(
        "chat-completions",
        vec![Reply::ChatCalls(1), Reply::ChatFinal],
        "time\n/exit\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(bodies[0]["tools"].as_array().expect("工具清单").len(), 5);
    for body in &bodies {
        let messages = body["messages"].as_array().expect("消息");
        assert_eq!(messages[0]["role"], "system");
        assert!(
            messages[0]["content"]
                .as_str()
                .expect("系统提示")
                .contains("<context_data>")
        );
    }
    let messages = bodies[1]["messages"].as_array().expect("历史消息");
    assert!(
        messages
            .iter()
            .any(|item| item["role"] == "assistant" && item["tool_calls"][0]["id"] == "call_1")
    );
    assert!(
        messages
            .iter()
            .any(|item| item["role"] == "tool" && item["tool_call_id"] == "call_1")
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("done"));
}

#[test]
fn reset_keeps_system_prompt_for_both_apis() {
    for api in ["responses", "chat-completions"] {
        let replies = if api == "responses" {
            vec![Reply::ResponsesFinal; 2]
        } else {
            vec![Reply::ChatFinal; 2]
        };
        let (output, bodies) = run_repl(api, replies, "first\n/reset\nsecond\n/exit\n");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 2);
        if api == "responses" {
            assert_eq!(bodies[0]["instructions"], bodies[1]["instructions"]);
            assert!(
                bodies[1]["instructions"]
                    .as_str()
                    .expect("系统提示")
                    .contains("<context_data>")
            );
            let input = bodies[1]["input"].as_array().expect("消息");
            assert_eq!(input.len(), 1);
            assert_eq!(input[0]["content"], "second");
        } else {
            let first = bodies[0]["messages"].as_array().expect("消息");
            let second = bodies[1]["messages"].as_array().expect("消息");
            assert_eq!(first[0], second[0]);
            assert_eq!(second.len(), 2);
            assert_eq!(second[1]["content"], "second");
        }
    }
}

#[test]
fn invalid_bash_path_fails_before_repl() {
    let output = Command::new(env!("CARGO_BIN_EXE_geer-agent"))
        .env("OPENAI_API_KEY", "test-key")
        .env("OPENAI_MODEL", "test-model")
        .env("GEER_AGENT_BASH_BIN", "/definitely/missing/geer-agent-bash")
        .output()
        .expect("启动程序");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("无法运行 Bash"));
}

#[test]
fn chat_failure_after_tool_keeps_complete_call_result_pair() {
    let (output, bodies) = run_repl(
        "chat-completions",
        vec![Reply::ChatCalls(1), Reply::Error, Reply::ChatFinal],
        "first\nsecond\n/exit\n",
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("模型请求失败"));
    let messages = bodies[2]["messages"].as_array().expect("后续请求历史");
    assert!(
        messages
            .iter()
            .any(|item| item["role"] == "tool" && item["tool_call_id"] == "call_1")
    );
    assert!(
        messages
            .iter()
            .any(|item| item["role"] == "user" && item["content"] == "second"),
        "{messages:?}"
    );
}

#[test]
fn responses_failure_after_tool_keeps_complete_call_result_pairs() {
    // Responses 失败会重试 5 次；6 次 Error 才能结束该轮，下一轮用户输入才会发出。
    let replies = [
        vec![Reply::ResponsesCalls],
        vec![Reply::Error; 6],
        vec![Reply::ResponsesFinal],
    ]
    .concat();
    let (output, bodies) = run_repl("responses", replies, "first\nsecond\n/exit\n");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("模型请求失败"));
    let input = bodies.last().expect("后续请求")["input"]
        .as_array()
        .expect("后续请求历史");
    assert_eq!(
        input
            .iter()
            .filter(|item| item["type"] == "function_call_output")
            .count(),
        2
    );
    assert!(
        input
            .iter()
            .any(|item| item["role"] == "user" && item["content"] == "second"),
        "{input:?}"
    );
}

#[test]
fn noninteractive_bash_request_is_denied_without_running() {
    let path = std::env::temp_dir().join(format!("geer-agent-denied-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let command = format!("printf leak > {}", path.display());
    let (output, bodies) = run_repl(
        "chat-completions",
        vec![Reply::ChatBash(command), Reply::ChatFinal],
        "run\n/exit\n",
    );
    assert!(output.status.success());
    assert!(!path.exists(), "未经授权的命令不应创建文件");
    let messages = bodies[1]["messages"].as_array().expect("工具结果");
    assert!(messages.iter().any(|item| {
        item["role"] == "tool"
            && item["content"]
                .as_str()
                .is_some_and(|text| text.contains("未执行"))
    }));
}

#[test]
fn chat_stops_after_five_tool_rounds() {
    let replies = (1..=6).map(Reply::ChatCalls).collect();
    let (output, bodies) = run_repl("chat-completions", replies, "time\n/exit\n");
    assert!(output.status.success());
    assert_eq!(bodies.len(), 6);
    assert!(String::from_utf8_lossy(&output.stdout).contains("工具调用轮次过多"));
}

#[test]
fn responses_stops_after_five_tool_rounds() {
    let replies = vec![Reply::ResponsesCalls; 6];
    let (output, bodies) = run_repl("responses", replies, "time\n/exit\n");
    assert!(output.status.success());
    assert_eq!(bodies.len(), 6);
    assert!(String::from_utf8_lossy(&output.stdout).contains("工具调用轮次过多"));
}
