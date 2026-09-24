use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone)]
enum Reply {
    ResponsesCalls,
    ResponsesFinal,
    ChatCalls(usize),
    ChatBash(String),
    ChatFinal,
    ChatFinalDropTrace(String),
    ReadBatch {
        api: &'static str,
    },
    NamedCall {
        api: &'static str,
        name: String,
        args: String,
        id: usize,
        usage: bool,
    },
    Error,
    RetryableError,
    ChatPartial,
}

fn run_repl(api: &str, replies: Vec<Reply>, input: &str) -> (Output, Vec<Value>) {
    run_repl_with_env(api, replies, input, &[])
}

fn run_repl_with_env(
    api: &str,
    replies: Vec<Reply>,
    input: &str,
    env: &[(&str, &str)],
) -> (Output, Vec<Value>) {
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

    let mut command = Command::new(env!("CARGO_BIN_EXE_geer-agent"));
    command
        .env("OPENAI_API_KEY", "test-key")
        .env("OPENAI_MODEL", "test-model")
        .env("OPENAI_BASE_URL", url)
        .env("OPENAI_API", api)
        .env("GEER_AGENT_TOOLS", "on")
        .env_remove("GEER_AGENT_BASH_BIN");
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command
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
    if let Reply::ChatFinalDropTrace(path) = &reply {
        let path = path.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let db = Database::connect(format!("sqlite://{path}?mode=rw"))
                .await
                .unwrap();
            db.execute_unprepared("DROP TABLE llm_traces")
                .await
                .unwrap();
        });
    }
    let (status, kind, body) = match reply {
        Reply::Error => (
            "400 Bad Request",
            "application/json",
            r#"{"error":{"message":"boom","type":"invalid_request_error"}}"#.to_owned(),
        ),
        Reply::RetryableError => (
            "500 Internal Server Error",
            "application/json",
            r#"{"error":{"message":"temporary failure","type":"server_error"}}"#.to_owned(),
        ),
        Reply::ChatPartial => (
            "200 OK",
            "text/event-stream",
            chat_chunk(json!({"content":"part"}), Value::Null),
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
        Reply::ChatFinal | Reply::ChatFinalDropTrace(_) => (
            "200 OK",
            "text/event-stream",
            chat_chunk(json!({"content":"done"}), json!("stop")),
        ),
        Reply::ReadBatch {
            api: "chat-completions",
        } => {
            let calls: Vec<_> = (0..3).map(|index| json!({"index":index,"id":format!("read_{index}"),"type":"function","function":{"name":"read","arguments":format!(r#"{{"path":"batch-{index}.txt"}}"#)}})).collect();
            (
                "200 OK",
                "text/event-stream",
                chat_chunk(json!({"tool_calls":calls}), json!("tool_calls")),
            )
        }
        Reply::ReadBatch { api: "responses" } => {
            let calls: Vec<_> = (0..3).map(|index| json!({"type":"function_call","arguments":format!(r#"{{"path":"batch-{index}.txt"}}"#),"call_id":format!("read_{index}"),"name":"read"})).collect();
            let response = json!({"created_at":0,"completed_at":0,"id":"resp_batch","model":"test-model","object":"response","output":calls,"status":"completed"});
            (
                "200 OK",
                "text/event-stream",
                sse(json!({"type":"response.completed","sequence_number":1,"response":response})),
            )
        }
        Reply::ReadBatch { .. } => panic!("不支持的 API"),
        Reply::NamedCall {
            api: "chat-completions",
            name,
            args,
            id,
            usage,
        } => {
            let mut body = chat_chunk(
                json!({"tool_calls":[{"index":0,"id":format!("call_{id}"),"type":"function","function":{"name":name,"arguments":args}}]}),
                json!("tool_calls"),
            );
            if usage {
                body.push_str(&sse(json!({"id":"chat_1","object":"chat.completion.chunk","created":0,"model":"test-model","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}})));
            }
            ("200 OK", "text/event-stream", body)
        }
        Reply::NamedCall {
            api: "responses",
            name,
            args,
            id,
            usage,
        } => {
            let mut response = json!({"created_at":0,"completed_at":0,"id":format!("resp_{id}"),"model":"test-model","object":"response","output":[{"type":"function_call","arguments":args,"call_id":format!("call_{id}"),"name":name}],"status":"completed"});
            if usage {
                response["usage"] = json!({"input_tokens":10,"input_tokens_details":{"cached_tokens":0},"output_tokens":2,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":12});
            }
            (
                "200 OK",
                "text/event-stream",
                sse(json!({"type":"response.completed","sequence_number":1,"response":response})),
            )
        }
        Reply::NamedCall { .. } => panic!("不支持的 API"),
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

fn named(api: &'static str, name: &str, args: &str, id: usize, usage: bool) -> Reply {
    Reply::NamedCall {
        api,
        name: name.to_owned(),
        args: args.to_owned(),
        id,
        usage,
    }
}

fn run_reason(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["event"] == "agent_run")
        .and_then(|event| event["terminationReason"].as_str().map(str::to_owned))
        .expect("运行结束原因")
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
fn ordinary_turns_keep_history_but_commands_do_not_add_messages() {
    for api in ["responses", "chat-completions"] {
        let replies = if api == "responses" {
            vec![Reply::ResponsesFinal; 2]
        } else {
            vec![Reply::ChatFinal; 2]
        };
        let (output, bodies) = run_repl(api, replies, "first\n/help\n\nsecond\n/exit\n");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 2);
        if api == "responses" {
            let input = bodies[1]["input"].as_array().expect("Responses 历史");
            assert_eq!(input.len(), 3);
            assert_eq!(input[0]["content"], "first");
            assert_eq!(input[1]["type"], "message");
            assert_eq!(input[2]["content"], "second");
        } else {
            let messages = bodies[1]["messages"].as_array().expect("Chat 历史");
            assert_eq!(messages.len(), 4);
            assert_eq!(messages[0]["role"], "system");
            assert_eq!(messages[1]["content"], "first");
            assert_eq!(messages[2]["content"], "done");
            assert_eq!(messages[3]["content"], "second");
        }
    }
}

#[test]
fn chat_failure_before_tools_discards_failed_user_message() {
    let (output, bodies) = run_repl(
        "chat-completions",
        vec![Reply::Error, Reply::ChatFinal],
        "first\nsecond\n/exit\n",
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("模型请求失败"));
    let messages = bodies[1]["messages"].as_array().expect("Chat 消息");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["content"], "second");
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
fn both_apis_continue_after_five_tool_turns() {
    for api in ["responses", "chat-completions"] {
        let replies = if api == "responses" {
            [vec![Reply::ResponsesCalls; 6], vec![Reply::ResponsesFinal]].concat()
        } else {
            (1..=6)
                .map(Reply::ChatCalls)
                .chain([Reply::ChatFinal])
                .collect()
        };
        let (output, bodies) = run_repl(api, replies, "time\n/exit\n");
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 7, "{api}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("done"), "{api}: {stdout}");
        assert!(!stdout.contains("工具调用轮次过多"), "{api}: {stdout}");
    }
}

#[test]
fn both_apis_finalize_without_tools_and_keep_final_text() {
    for api in ["responses", "chat-completions"] {
        let replies = if api == "responses" {
            [
                vec![Reply::ResponsesCalls; 29],
                vec![Reply::ResponsesFinal, Reply::ResponsesFinal],
            ]
            .concat()
        } else {
            (1..=29)
                .map(Reply::ChatCalls)
                .chain([Reply::ChatFinal, Reply::ChatFinal])
                .collect()
        };
        let (output, bodies) = run_repl(api, replies, "first\nsecond\n/exit\n");
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 31, "{api}");

        let finalization = &bodies[29];
        let next_turn = &bodies[30];
        assert!(
            finalization["tools"].is_null()
                || finalization["tools"].as_array().is_some_and(Vec::is_empty),
            "{api}: {finalization}"
        );
        assert_eq!(
            next_turn["tools"].as_array().expect("下一轮恢复工具").len(),
            5,
            "{api}"
        );

        if api == "responses" {
            assert!(
                finalization["instructions"]
                    .as_str()
                    .expect("收敛指令")
                    .contains("最后一个 Agent Turn")
            );
            assert!(
                !next_turn["instructions"]
                    .as_str()
                    .expect("下一轮系统提示")
                    .contains("最后一个 Agent Turn")
            );
            let input = next_turn["input"].as_array().expect("下一轮历史");
            assert!(input.iter().any(|item| {
                item["type"] == "message"
                    && item["role"] == "assistant"
                    && item.to_string().contains("done")
            }));
            assert!(
                input
                    .iter()
                    .any(|item| { item["role"] == "user" && item["content"] == "second" })
            );
        } else {
            let final_messages = finalization["messages"].as_array().expect("收敛消息");
            assert!(
                final_messages[0]["content"]
                    .as_str()
                    .expect("收敛指令")
                    .contains("最后一个 Agent Turn")
            );
            let messages = next_turn["messages"].as_array().expect("下一轮历史");
            assert!(
                !messages[0]["content"]
                    .as_str()
                    .expect("下一轮系统提示")
                    .contains("最后一个 Agent Turn")
            );
            assert!(
                messages
                    .iter()
                    .any(|item| { item["role"] == "assistant" && item["content"] == "done" })
            );
            assert!(
                messages
                    .iter()
                    .any(|item| { item["role"] == "user" && item["content"] == "second" })
            );
        }
    }
}

#[test]
fn both_apis_stop_repeated_calls_and_consecutive_errors() {
    for api in ["chat-completions", "responses"] {
        for distinct in [false, true] {
            let mut replies: Vec<_> = (1..=4)
                .map(|id| {
                    let args = if distinct {
                        format!(r#"{{"attempt":{id}}}"#)
                    } else {
                        "{}".to_owned()
                    };
                    named(api, "unknown", &args, id, false)
                })
                .collect();
            replies.push(if api == "responses" {
                Reply::ResponsesFinal
            } else {
                Reply::ChatFinal
            });
            let (output, bodies) = run_repl(api, replies, "work\n/exit\n");
            assert!(
                output.status.success(),
                "{api}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(bodies.len(), 5, "{api}");
            assert!(
                bodies[4]["tools"].is_null()
                    || bodies[4]["tools"].as_array().is_some_and(Vec::is_empty)
            );
            assert_eq!(
                run_reason(&output),
                if distinct {
                    "consecutive_errors"
                } else {
                    "repeated_tool_loop"
                }
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("attempt"));
            assert!(!stderr.contains("未知工具"));
        }
    }
}

#[test]
fn both_apis_enforce_reported_token_budget_without_running_tool() {
    for api in ["chat-completions", "responses"] {
        let replies = vec![
            named(api, "get_current_time", "{}", 1, true),
            if api == "responses" {
                Reply::ResponsesFinal
            } else {
                Reply::ChatFinal
            },
        ];
        let (output, bodies) = run_repl_with_env(
            api,
            replies,
            "work\n/exit\n",
            &[("GEER_AGENT_MAX_INPUT_TOKENS", "10")],
        );
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 2, "{api}");
        assert_eq!(run_reason(&output), "max_tokens");
        assert!(
            bodies[1]["tools"].is_null()
                || bodies[1]["tools"].as_array().is_some_and(Vec::is_empty)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("\"event\":\"tool_call\""),
            "工具未执行: {stderr}"
        );
    }
}

#[test]
fn both_apis_stop_when_configured_budget_lacks_usage() {
    for api in ["chat-completions", "responses"] {
        let replies = vec![
            named(api, "get_current_time", "{}", 1, false),
            if api == "responses" {
                Reply::ResponsesFinal
            } else {
                Reply::ChatFinal
            },
        ];
        let (output, bodies) = run_repl_with_env(
            api,
            replies,
            "work\n/exit\n",
            &[("GEER_AGENT_MAX_INPUT_TOKENS", "10")],
        );
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 2, "{api}");
        assert_eq!(run_reason(&output), "usage_unavailable");
        assert!(
            bodies[1]["tools"].is_null()
                || bodies[1]["tools"].as_array().is_some_and(Vec::is_empty)
        );
    }
}

#[test]
fn both_apis_pair_multi_read_batch_in_original_order() {
    for api in ["chat-completions", "responses"] {
        let replies = vec![
            Reply::ReadBatch { api },
            if api == "responses" {
                Reply::ResponsesFinal
            } else {
                Reply::ChatFinal
            },
        ];
        let (output, bodies) = run_repl(api, replies, "read\n/exit\n");
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 2);
        assert_eq!(run_reason(&output), "completed");
        if api == "responses" {
            let input = bodies[1]["input"].as_array().expect("Responses 输入");
            let ids: Vec<_> = input
                .iter()
                .filter(|item| item["type"] == "function_call_output")
                .map(|item| item["call_id"].as_str().expect("调用标识"))
                .collect();
            assert_eq!(ids, ["read_0", "read_1", "read_2"]);
        } else {
            let messages = bodies[1]["messages"].as_array().expect("Chat 消息");
            let ids: Vec<_> = messages
                .iter()
                .filter(|item| item["role"] == "tool")
                .map(|item| item["tool_call_id"].as_str().expect("调用标识"))
                .collect();
            assert_eq!(ids, ["read_0", "read_1", "read_2"]);
        }
    }
}

#[test]
fn successful_tool_breaks_consecutive_error_chain() {
    for api in ["chat-completions", "responses"] {
        let replies = vec![
            named(api, "unknown", r#"{"attempt":1}"#, 1, false),
            named(api, "unknown", r#"{"attempt":2}"#, 2, false),
            named(api, "get_current_time", "{}", 3, false),
            named(api, "unknown", r#"{"attempt":3}"#, 4, false),
            named(api, "unknown", r#"{"attempt":4}"#, 5, false),
            if api == "responses" {
                Reply::ResponsesFinal
            } else {
                Reply::ChatFinal
            },
        ];
        let (output, bodies) = run_repl(api, replies, "work\n/exit\n");
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 6, "{api}");
        assert_eq!(run_reason(&output), "completed");
    }
}

#[tokio::test]
async fn trace_records_each_model_step_and_session_reset_for_both_apis() {
    for api in ["chat-completions", "responses"] {
        let path = std::env::temp_dir().join(format!("geer-trace-e2e-{}.sqlite", Uuid::new_v4()));
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let replies = if api == "responses" {
            vec![
                Reply::ResponsesCalls,
                Reply::ResponsesFinal,
                Reply::ResponsesFinal,
                Reply::ResponsesFinal,
            ]
        } else {
            vec![
                Reply::ChatCalls(1),
                Reply::ChatFinal,
                Reply::ChatFinal,
                Reply::ChatFinal,
            ]
        };
        let (output, bodies) = run_repl_with_env(
            api,
            replies,
            "first\nsecond\n/reset\nthird\n/exit\n",
            &[
                ("GEER_AGENT_TRACE_DATABASE", "sqlite"),
                ("GEER_AGENT_TRACE_DATABASE_URL", &url),
            ],
        );
        assert!(
            output.status.success(),
            "{api}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(bodies.len(), 4, "{api}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let shown_sessions: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("Session ID: "))
            .collect();
        assert_eq!(shown_sessions.len(), 2, "{stdout}");
        assert_ne!(shown_sessions[0], shown_sessions[1]);

        let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
            .await
            .unwrap();
        let rows = db.query_all_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT request_id, session_id, agent_run_id, attempts, status, request, response FROM llm_traces"))
            .await.unwrap();
        assert_eq!(rows.len(), 4, "{api}");
        let mut records = Vec::new();
        for body in &bodies {
            let row = rows
                .iter()
                .find(|row| row.try_get::<Value>("", "request").unwrap() == *body)
                .expect("存储的完整请求应与实际 HTTP 请求体一致");
            let id: String = row.try_get("", "request_id").unwrap();
            let session: String = row.try_get("", "session_id").unwrap();
            let run: String = row.try_get("", "agent_run_id").unwrap();
            let attempts: i32 = row.try_get("", "attempts").unwrap();
            let status: String = row.try_get("", "status").unwrap();
            let response: Value = row.try_get("", "response").unwrap();
            assert_eq!(attempts, 1);
            assert_eq!(status, "completed");
            assert!(!response.is_null());
            records.push((id, session, run));
        }
        assert_eq!(records[0].1, shown_sessions[0]);
        assert_eq!(records[1].1, shown_sessions[0]);
        assert_eq!(records[2].1, shown_sessions[0]);
        assert_eq!(records[3].1, shown_sessions[1]);
        assert_eq!(records[0].2, records[1].2);
        assert_ne!(records[1].2, records[2].2);
        assert_eq!(
            records
                .iter()
                .map(|record| &record.0)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn unreachable_trace_database_warns_without_blocking_model_or_exposing_uri() {
    let secret_url = "postgres://trace_user:private_password@127.0.0.1:1/trace";
    let (output, bodies) = run_repl_with_env(
        "chat-completions",
        vec![Reply::ChatFinal],
        "hello\n/exit\n",
        &[
            ("GEER_AGENT_TRACE_DATABASE", "postgres"),
            ("GEER_AGENT_TRACE_DATABASE_URL", secret_url),
        ],
    );
    assert!(output.status.success());
    assert_eq!(bodies.len(), 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("done"));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Trace 数据库连接或初始化失败"), "{stderr}");
    assert!(!stderr.contains("private_password"));
    assert!(!stderr.contains(secret_url));
}

#[test]
fn trace_write_failure_warns_and_preserves_model_answer() {
    let path = std::env::temp_dir().join(format!(
        "geer-trace-write-failure-{}.sqlite",
        Uuid::new_v4()
    ));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let (output, bodies) = run_repl_with_env(
        "chat-completions",
        vec![Reply::ChatFinalDropTrace(path.display().to_string())],
        "hello\n/exit\n",
        &[
            ("GEER_AGENT_TRACE_DATABASE", "sqlite"),
            ("GEER_AGENT_TRACE_DATABASE_URL", &url),
        ],
    );
    assert!(output.status.success());
    assert_eq!(bodies.len(), 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("done"));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Trace 写入失败（request_id="), "{stderr}");
    assert!(!stderr.contains(&url));
    let _ = std::fs::remove_file(path);
}

#[test]
fn invalid_trace_database_selection_fails_before_repl() {
    let output = Command::new(env!("CARGO_BIN_EXE_geer-agent"))
        .env("OPENAI_API_KEY", "test-key")
        .env("OPENAI_MODEL", "test-model")
        .env("GEER_AGENT_TRACE_DATABASE", "unknown")
        .env(
            "GEER_AGENT_TRACE_DATABASE_URL",
            "sqlite://trace.sqlite?mode=rwc",
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("GEER_AGENT_TRACE_DATABASE 只能是"));
}

#[tokio::test]
async fn chat_trace_counts_transport_retry_and_keeps_partial_failure() {
    let path = std::env::temp_dir().join(format!("geer-trace-chat-{}.sqlite", Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let (output, bodies) = run_repl_with_env(
        "chat-completions",
        vec![Reply::RetryableError, Reply::ChatFinal, Reply::ChatPartial],
        "first\nsecond\n/exit\n",
        &[
            ("GEER_AGENT_TRACE_DATABASE", "sqlite"),
            ("GEER_AGENT_TRACE_DATABASE_URL", &url),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(bodies.len(), 3);
    assert_eq!(bodies[0], bodies[1]);
    let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT request, response, status, attempts FROM llm_traces",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let completed = rows
        .iter()
        .find(|row| row.try_get::<Value>("", "request").unwrap() == bodies[0])
        .unwrap();
    let failed = rows
        .iter()
        .find(|row| row.try_get::<Value>("", "request").unwrap() == bodies[2])
        .unwrap();
    assert_eq!(
        completed.try_get::<String>("", "status").unwrap(),
        "completed"
    );
    assert_eq!(completed.try_get::<i32>("", "attempts").unwrap(), 2);
    assert_eq!(failed.try_get::<String>("", "status").unwrap(), "failed");
    assert_eq!(failed.try_get::<i32>("", "attempts").unwrap(), 1);
    assert_eq!(
        failed.try_get::<Value>("", "response").unwrap()["text"],
        "part"
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn trace_redacts_configured_api_key_even_when_it_appears_in_user_input() {
    let path = std::env::temp_dir().join(format!("geer-trace-secret-{}.sqlite", Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let (output, bodies) = run_repl_with_env(
        "chat-completions",
        vec![Reply::ChatFinal],
        "test-key\n/exit\n",
        &[
            ("GEER_AGENT_TRACE_DATABASE", "sqlite"),
            ("GEER_AGENT_TRACE_DATABASE_URL", &url),
        ],
    );
    assert!(output.status.success());
    assert!(bodies[0].to_string().contains("test-key"));
    let db = Database::connect(format!("sqlite://{}?mode=rw", path.display()))
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT request, response FROM llm_traces",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let stored_request: Value = rows[0].try_get("", "request").unwrap();
    assert!(!stored_request.to_string().contains("test-key"));
    assert!(stored_request.to_string().contains("[REDACTED]"));
    drop(db);
    std::fs::remove_file(path).unwrap();
}
