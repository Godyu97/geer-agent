use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use super::*;
use crate::tools::Tools;

struct Reply {
    status: u16,
    headers: String,
    body: Vec<u8>,
    delay: Duration,
    body_delay: Duration,
    chunked: bool,
}

impl Reply {
    fn text(content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: format!("Content-Type: {content_type}\r\n"),
            body: body.into(),
            delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            chunked: false,
        }
    }
    fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            headers: format!("Location: {location}\r\n"),
            ..Self::text("text/plain", "")
        }
    }
}

struct Server {
    url: Url,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            for reply in replies {
                let mut stream = loop {
                    if stopped.load(Ordering::Relaxed) || Instant::now() > deadline {
                        return;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(error) => panic!("mock accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                // 超时测试可能在 TCP 建连后、发送请求前取消；这属于正常断连。
                let Ok(request) = read_request(&mut stream) else {
                    return;
                };
                seen.lock().unwrap().push(request);
                thread::sleep(reply.delay);
                let framing = if reply.chunked {
                    "Transfer-Encoding: chunked\r\n".to_owned()
                } else {
                    format!("Content-Length: {}\r\n", reply.body.len())
                };
                let headers = format!(
                    "HTTP/1.1 {} Mock\r\n{}{framing}Connection: close\r\n\r\n",
                    reply.status, reply.headers
                );
                if stream.write_all(headers.as_bytes()).is_ok() {
                    thread::sleep(reply.body_delay);
                    if reply.chunked {
                        let _ = write!(stream, "{:x}\r\n", reply.body.len());
                        let _ = stream.write_all(&reply.body);
                        let _ = stream.write_all(b"\r\n0\r\n\r\n");
                    } else {
                        let _ = stream.write_all(&reply.body);
                    }
                }
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(worker),
        }
    }

    fn client(&self) -> WebAccess {
        WebAccess {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("geer-agent-test")
                .build()
                .unwrap(),
            endpoint: self.url.clone(),
            search_timeout: Duration::from_millis(500),
            fetch_timeout: Duration::from_millis(500),
        }
    }

    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut block = [0; 4096];
    loop {
        let count = stream.read(&mut block)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock 请求被取消",
            ));
        }
        bytes.extend_from_slice(&block[..count]);
        assert!(bytes.len() <= 2 * MAX_BODY);
        if let Some(end) = bytes.windows(4).position(|item| item == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..end]);
            let length = header
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                return Ok(bytes);
            }
        }
    }
}

fn metadata(output: &ToolOutput) -> Value {
    serde_json::from_str(output.text.split_once("\n\n").unwrap().0).unwrap()
}
fn body(output: &ToolOutput) -> &str {
    output.text.split_once("\n\n").unwrap().1
}
fn rpc(text: &str) -> String {
    json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":text}]}}).to_string()
}

#[tokio::test]
async fn exa_json_and_sse_send_objective_and_return_complete_sources() {
    let url = format!("https://example.com/{}", "a".repeat(3500));
    let text = format!(
        "Title: Rust docs\nURL: {url}\nPublished: N/A\nHighlights:\n  Rust snippet\n---\nTitle: Other\nURL: https://example.org/doc\nHighlights:\nother snippet"
    );
    for sse in [false, true] {
        let response = rpc(&text);
        let server = Server::new(vec![Reply::text(
            if sse {
                "text/event-stream"
            } else {
                "application/json"
            },
            if sse {
                let pretty = serde_json::to_string_pretty(
                    &serde_json::from_str::<Value>(&response).unwrap(),
                )
                .unwrap();
                let data = pretty
                    .lines()
                    .map(|line| format!("data: {line}\r\n"))
                    .collect::<String>();
                format!(
                    ": keepalive\r\ndata: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}}\r\n\r\nevent: message\r\n{data}\r\n"
                )
            } else {
                response
            },
        )]);
        let result = server
            .client()
            .search(SearchRequest::parse(&json!({"query":"Rust docs", "num_results":2})).unwrap())
            .await;
        assert!(result.success, "{}", result.text);
        assert_eq!(metadata(&result)["provider"], "exa");
        assert_eq!(metadata(&result)["results"], 2);
        assert!(body(&result).contains(&url));
        assert!(body(&result).contains("Rust snippet"));
        let requests = server.requests.lock().unwrap();
        let raw = String::from_utf8_lossy(&requests[0]);
        assert!(
            raw.to_ascii_lowercase()
                .contains("user-agent: geer-agent-test")
        );
        assert!(!raw.to_ascii_lowercase().contains("authorization:"));
        let args: Value = serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(args["method"], "tools/call");
        assert_eq!(
            args["params"]["arguments"],
            json!({"query":"Rust docs", "objective":"Rust docs", "numResults":2})
        );
    }
}

#[tokio::test]
async fn exa_failures_are_distinct_from_explicit_empty_results() {
    for (status, text, code) in [
        (403, "", Some("http_forbidden")),
        (429, "", Some("http_rate_limited")),
        (
            200,
            json!({"jsonrpc":"2.0", "id":1, "error":{"code":-1,"message":"failed"}})
                .to_string()
                .as_str(),
            Some("rpc_error"),
        ),
        (200, "not JSON or SSE", Some("protocol_error")),
        (
            200,
            r#"{"jsonrpc":"2.0","id":99,"result":{}}"#,
            Some("protocol_error"),
        ),
    ] {
        let mut reply = Reply::text("application/json", text);
        reply.status = status;
        let server = Server::new(vec![reply]);
        let result = server
            .client()
            .search(SearchRequest::parse(&json!({"query":"x"})).unwrap())
            .await;
        assert!(!result.success);
        assert_eq!(metadata(&result)["code"].as_str(), code);
    }
    for (text, expected) in [
        ("No results found", None),
        ("unrecognized response", Some("protocol_error")),
    ] {
        let server = Server::new(vec![Reply::text("application/json", rpc(text))]);
        let result = server
            .client()
            .search(SearchRequest::parse(&json!({"query":"x"})).unwrap())
            .await;
        assert_eq!(result.success, expected.is_none());
        assert_eq!(metadata(&result)["code"].as_str(), expected);
    }
    let server = Server::new(vec![Reply::text("application/json", json!({"jsonrpc":"2.0","id":1,"result":{"isError":true,"content":[{"type":"text","text":"failure"}]}}).to_string())]);
    let result = server
        .client()
        .search(SearchRequest::parse(&json!({"query":"x"})).unwrap())
        .await;
    assert_eq!(metadata(&result)["code"], "rpc_error");
}

#[tokio::test]
async fn web_responses_have_body_and_total_network_budgets() {
    for search in [true, false] {
        for chunked in [false, true] {
            let mut reply = Reply::text("text/plain", "x".repeat(MAX_BODY + 1));
            reply.chunked = chunked;
            let server = Server::new(vec![reply]);
            let result = if search {
                server
                    .client()
                    .search(SearchRequest::parse(&json!({"query":"x"})).unwrap())
                    .await
            } else {
                server
                    .client()
                    .fetch(server.url.clone(), &mut |_| Ok(true))
                    .await
            };
            assert_eq!(metadata(&result)["code"], "response_too_large");
        }
        let mut reply = Reply::text("text/plain", "slow");
        reply.body_delay = Duration::from_millis(150);
        let server = Server::new(vec![reply]);
        let mut web = server.client();
        web.search_timeout = Duration::from_millis(50);
        web.fetch_timeout = Duration::from_millis(50);
        let result = if search {
            web.search(SearchRequest::parse(&json!({"query":"x"})).unwrap())
                .await
        } else {
            web.fetch(server.url.clone(), &mut |_| Ok(true)).await
        };
        assert_eq!(metadata(&result)["code"], "network_timeout");
    }
    let server = Server::new(
        (0..3)
            .map(|_| {
                let mut reply = Reply::redirect("/next");
                reply.delay = Duration::from_millis(40);
                reply
            })
            .collect(),
    );
    let mut web = server.client();
    web.fetch_timeout = Duration::from_millis(90);
    let result = web
        .fetch(server.url.clone(), &mut |_| panic!("同来源不应再次确认"))
        .await;
    assert_eq!(metadata(&result)["code"], "network_timeout");
    let server = Server::new(vec![Reply::text("text/plain", "must not be requested")]);
    let mut web = server.client();
    web.fetch_timeout = Duration::ZERO;
    let result = web.fetch(server.url.clone(), &mut |_| Ok(true)).await;
    assert_eq!(metadata(&result)["code"], "network_timeout");
    assert_eq!(server.count(), 0);
}

#[tokio::test]
async fn fetch_html_cleans_scripts_and_preserves_lists_code_and_text() {
    let server = Server::new(vec![Reply::text(
        "text/html; charset=UTF-8",
        "<html><head><style>style-secret</style></head><body><script>script-secret</script><p>Hello &amp; Rust</p><ul><li>first</li><li>second</li></ul><pre>fn main() {\n    code();\n}</pre><p>After</p></body></html>",
    )]);
    let result = server
        .client()
        .fetch(server.url.clone(), &mut |_| Ok(true))
        .await;
    assert!(result.success, "{}", result.text);
    assert!(body(&result).contains("Hello & Rust"));
    assert!(body(&result).contains("* first"));
    assert!(body(&result).contains("    code();"));
    assert!(!body(&result).contains("secret"));
    assert_eq!(metadata(&result)["url"], server.url.as_str());
    for (mime, text) in [
        ("text/markdown", "# Title\n\n  text\n"),
        ("application/json", "{\"ok\":true}"),
        ("text/plain; charset=iso-8859-1", "café"),
    ] {
        let bytes = if mime.contains("iso-") {
            b"caf\xe9".to_vec()
        } else {
            text.as_bytes().to_vec()
        };
        let server = Server::new(vec![Reply::text(mime, bytes)]);
        let result = server
            .client()
            .fetch(server.url.clone(), &mut |_| Ok(true))
            .await;
        assert!(result.success, "{}", result.text);
        assert_eq!(body(&result), text);
    }
    for (mime, bytes, code) in [
        (
            "application/pdf",
            b"%PDF".to_vec(),
            "unsupported_content_type",
        ),
        ("text/plain", b"a\0b".to_vec(), "invalid_text"),
        ("text/plain", vec![0xff], "invalid_text"),
    ] {
        let server = Server::new(vec![Reply::text(mime, bytes)]);
        let result = server
            .client()
            .fetch(server.url.clone(), &mut |_| Ok(true))
            .await;
        assert_eq!(metadata(&result)["code"], code);
    }
}

#[tokio::test]
async fn fetch_confirms_every_call_and_refusal_sends_no_request() {
    let server = Server::new(vec![
        Reply::text("text/plain", "ok"),
        Reply::text("text/plain", "ok"),
    ]);
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    tools.web = Some(server.client());
    tools.grant_for_test("web_fetch");
    let prompts = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&prompts);
    tools.set_confirm(move |prompt| {
        seen.borrow_mut().push(prompt.to_owned());
        Ok(false)
    });
    let args = json!({"url":server.url.as_str()}).to_string();
    let denied = tools.execute_recorded("web_fetch", &args).await;
    assert_eq!(metadata(&denied)["code"], "authorization_denied");
    assert_eq!(server.count(), 0);
    assert!(prompts.borrow()[0].contains(server.url.as_str()));
    let seen = std::rc::Rc::clone(&prompts);
    tools.set_confirm(move |prompt| {
        seen.borrow_mut().push(prompt.to_owned());
        Ok(true)
    });
    for _ in 0..2 {
        assert!(tools.execute_recorded("web_fetch", &args).await.success);
    }
    assert_eq!(prompts.borrow().len(), 3);
    assert_eq!(server.count(), 2);
    tools.enabled = false;
    assert_eq!(tools.execute("web_fetch", &args).await, "工具已关闭。");
    assert_eq!(server.count(), 2);
}

#[tokio::test]
async fn fetch_redirects_reconfirm_cross_origin_stop_on_refusal_and_limit_loops() {
    for allow in [false, true] {
        let target = Server::new(vec![Reply::text("text/plain", "target")]);
        let source = Server::new(vec![Reply::redirect(target.url.as_str())]);
        let mut prompts = Vec::new();
        let result = source
            .client()
            .fetch(source.url.clone(), &mut |prompt| {
                prompts.push(prompt.to_owned());
                Ok(allow)
            })
            .await;
        assert_eq!(prompts.len(), 1);
        assert!(
            prompts[0].contains(source.url.as_str()) && prompts[0].contains(target.url.as_str())
        );
        assert_eq!(target.count(), usize::from(allow));
        assert_eq!(result.success, allow);
        if allow {
            assert_eq!(metadata(&result)["final_url"], target.url.as_str());
        } else {
            assert_eq!(metadata(&result)["code"], "authorization_denied");
        }
    }
    let server = Server::new(vec![
        Reply::redirect("/next"),
        Reply::text("text/plain", "same"),
    ]);
    let result = server
        .client()
        .fetch(server.url.clone(), &mut |_| panic!("同来源无需额外授权"))
        .await;
    assert!(result.success);
    assert_eq!(metadata(&result)["redirects"], 1);
    let server = Server::new((0..6).map(|_| Reply::redirect("/loop")).collect());
    let result = server
        .client()
        .fetch(server.url.clone(), &mut |_| Ok(true))
        .await;
    assert_eq!(metadata(&result)["code"], "too_many_redirects");
    assert_eq!(server.count(), 6);
    let target = Server::new(vec![Reply::text("text/plain", "ok")]);
    let source = Server::new(vec![Reply::redirect(target.url.as_str())]);
    let mut web = source.client();
    web.fetch_timeout = Duration::from_millis(40);
    let result = web
        .fetch(source.url.clone(), &mut |_| {
            thread::sleep(Duration::from_millis(80));
            Ok(true)
        })
        .await;
    assert!(
        result.success,
        "用户确认等待不消耗网络预算：{}",
        result.text
    );
}

#[tokio::test]
async fn web_output_truncation_keeps_full_urls_and_search_grants_are_per_session() {
    let url = format!("https://example.com/{}", "a".repeat(4000));
    let sources = (0..4)
        .map(|_| Source {
            title: "Title".into(),
            url: url.clone(),
            snippet: "中".repeat(20_000),
        })
        .collect();
    let result = search_output(
        &SearchRequest {
            query: "x".into(),
            num_results: 5,
        },
        sources,
        false,
    );
    assert!(result.text.chars().count() <= MAX_OUTPUT);
    assert_eq!(metadata(&result)["truncated"], true);
    for line in body(&result)
        .lines()
        .filter(|line| line.starts_with("URL: "))
    {
        assert_eq!(line, &format!("URL: {url}"));
    }
    let server = Server::new(vec![Reply::text("text/plain", "中".repeat(20_000))]);
    let long = server
        .url
        .join(&format!("{}?query=full", "a".repeat(3500)))
        .unwrap();
    let result = server.client().fetch(long.clone(), &mut |_| Ok(true)).await;
    assert!(result.text.chars().count() <= MAX_OUTPUT);
    assert_eq!(metadata(&result)["url"], long.as_str());
    assert_eq!(metadata(&result)["final_url"], long.as_str());
    assert_eq!(metadata(&result)["truncated"], true);
    let server = Server::new(
        (0..3)
            .map(|_| Reply::text("application/json", rpc("No results found")))
            .collect(),
    );
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    tools.web = Some(server.client());
    let prompts = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = std::rc::Rc::clone(&prompts);
    tools.set_confirm(move |prompt| {
        assert!(prompt.contains("Exa"));
        seen.set(seen.get() + 1);
        Ok(true)
    });
    for _ in 0..2 {
        assert!(
            tools
                .execute_recorded("web_search", r#"{"query":"x"}"#)
                .await
                .success
        );
    }
    assert_eq!(prompts.get(), 1);
    tools.reset();
    assert!(
        tools
            .execute_recorded("web_search", r#"{"query":"x"}"#)
            .await
            .success
    );
    assert_eq!(prompts.get(), 2);
}

#[tokio::test]
async fn invalid_web_inputs_fail_before_confirmation_or_network() {
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    tools.set_confirm(|_| panic!("无效参数不应请求授权"));
    for args in [
        json!({"url":"file:///tmp/a"}),
        json!({"url":"http://user:secret@localhost/"}),
        json!({"url":"https://example.com", "extra":true}),
        json!({"url":format!("https://example.com/{}","a".repeat(4096))}),
    ] {
        let result = tools.execute_recorded("web_fetch", &args.to_string()).await;
        assert_eq!(metadata(&result)["code"], "invalid_argument");
    }
    for args in [
        json!({"query":" "}),
        json!({"query":"x", "num_results":21}),
        json!({"query":"x", "num_results":1.5}),
    ] {
        let result = tools
            .execute_recorded("web_search", &args.to_string())
            .await;
        assert_eq!(metadata(&result)["code"], "invalid_argument");
    }
    assert!(tools.web.is_none());
}

#[tokio::test]
#[ignore = "真实网络手动验收，不作为自动测试依赖"]
async fn real_web_access_smoke() {
    let web = WebAccess::new().unwrap();
    let result = web
        .search(SearchRequest {
            query: "Rust std fs read_to_string official documentation".into(),
            num_results: 2,
        })
        .await;
    assert!(result.success, "{}", result.text);
    assert!(metadata(&result)["results"].as_u64().unwrap() > 0);
    eprintln!("真实 Exa 搜索：{}", metadata(&result));
    let result = web
        .fetch(
            checked_url("https://doc.rust-lang.org/std/fs/fn.read_to_string.html").unwrap(),
            &mut |_| Ok(true),
        )
        .await;
    assert!(result.success, "{}", result.text);
    assert!(body(&result).contains("read_to_string"));
    eprintln!("真实静态网页抓取：{}", metadata(&result));
}
