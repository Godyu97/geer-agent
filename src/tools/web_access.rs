use std::{
    io,
    time::{Duration, Instant},
};

use reqwest::{Client, Response, Url, header};
use serde_json::{Value, json};

use super::{
    ToolOutput,
    feedback::{ToolError, bounded_result, fields, string},
};

const EXA_ENDPOINT: &str = "https://mcp.exa.ai/mcp?tools=web_search_exa";
const SEARCH_TIMEOUT: Duration = Duration::from_secs(25);
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BODY: usize = 1024 * 1024;
pub(super) const MAX_OUTPUT: usize = 12_000;
const MAX_URL: usize = 4096;
const MAX_REDIRECTS: usize = 5;

pub(super) struct SearchRequest {
    pub query: String,
    pub num_results: u64,
}

impl SearchRequest {
    pub fn parse(args: &Value) -> Result<Self, ToolError> {
        fields(args, &["query", "num_results"], "")?;
        let query = string(args, "query")?;
        if query.trim().is_empty() || query.contains('\0') || query.chars().count() > 4096 {
            return Err(ToolError::invalid(
                "query",
                "查询不能为空、包含 NUL 或超过 4096 字符。",
                "提供简短明确的搜索查询。",
            ));
        }
        let num_results = match args.get("num_results") {
            None => 5,
            Some(value) => value
                .as_u64()
                .filter(|number| (1..=20).contains(number))
                .ok_or_else(|| {
                    ToolError::invalid(
                        "num_results",
                        "必须是 1 到 20 的整数。",
                        "默认返回 5 项，最多 20 项。",
                    )
                })?,
        };
        Ok(Self {
            query: query.to_owned(),
            num_results,
        })
    }
}

pub(super) fn parse_fetch(args: &Value) -> Result<Url, ToolError> {
    fields(args, &["url"], "")?;
    checked_url(string(args, "url")?)
}

fn checked_url(raw: &str) -> Result<Url, ToolError> {
    let invalid = || {
        ToolError::invalid(
            "url",
            "只接受不含账号密码的完整 HTTP(S) URL，最多 4096 字符。",
            "提供完整 http:// 或 https:// URL；确认时会显示实际访问地址。",
        )
    };
    if raw.chars().count() > MAX_URL || raw.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = Url::parse(raw).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.as_str().chars().count() > MAX_URL
    {
        return Err(invalid());
    }
    Ok(url)
}

pub(super) struct WebAccess {
    client: Client,
    endpoint: Url,
    search_timeout: Duration,
    fetch_timeout: Duration,
}

impl WebAccess {
    #[cfg(test)]
    pub(super) fn for_test(endpoint: &str) -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("geer-agent-test")
                .build()
                .expect("测试 HTTP 客户端"),
            endpoint: Url::parse(endpoint).expect("测试 endpoint"),
            search_timeout: Duration::from_secs(2),
            fetch_timeout: Duration::from_secs(2),
        }
    }

    pub fn new() -> Result<Self, ToolError> {
        let client = Client::builder()
            .user_agent(concat!("geer-agent/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(FETCH_TIMEOUT)
            .build()
            .map_err(network_error)?;
        Ok(Self {
            client,
            endpoint: Url::parse(EXA_ENDPOINT).map_err(|error| {
                ToolError::new(
                    "invalid_endpoint",
                    error.to_string(),
                    "检查内置 Exa endpoint。",
                )
            })?,
            search_timeout: SEARCH_TIMEOUT,
            fetch_timeout: FETCH_TIMEOUT,
        })
    }

    pub async fn search(&self, request: SearchRequest) -> ToolOutput {
        match self.search_checked(&request).await {
            Ok((sources, limited)) => search_output(&request, sources, limited),
            Err(error) => error_output("web_search", None, None, error),
        }
    }

    async fn search_checked(
        &self,
        request: &SearchRequest,
    ) -> Result<(Vec<Source>, bool), ToolError> {
        let body = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{
            "name":"web_search_exa", "arguments":{"query":request.query,
                "objective":request.query, "numResults":request.num_results}}});
        let bytes = timed(self.search_timeout, async {
            let response = self
                .client
                .post(self.endpoint.clone())
                .header(header::ACCEPT, "application/json, text/event-stream")
                .json(&body)
                .send()
                .await
                .map_err(network_error)?;
            check_status(&response)?;
            bounded_body(response).await
        })
        .await?;
        let value = rpc_response(&bytes)?;
        if let Some(error) = value.get("error") {
            return Err(ToolError::new(
                "rpc_error",
                format!("Exa RPC 错误：{error}"),
                "检查 query 和服务状态；这是失败，不是空结果。",
            ));
        }
        let result = value
            .get("result")
            .ok_or_else(|| protocol_error("RPC 响应缺少 result。"))?;
        if result["isError"] == true {
            return Err(ToolError::new(
                "rpc_error",
                format!("Exa 工具失败：{}", result["content"]),
                "检查 query 和服务状态后重试。",
            ));
        }
        let content = result["content"]
            .as_array()
            .ok_or_else(|| protocol_error("MCP result 缺少 content 数组。"))?;
        let mut sources = Vec::new();
        let mut explicit_empty = false;
        for block in content {
            if block["type"] != "text" {
                continue;
            }
            let text = block["text"]
                .as_str()
                .ok_or_else(|| protocol_error("MCP 文本块缺少 text。"))?;
            let lower = text.trim().to_ascii_lowercase();
            explicit_empty |= lower.starts_with("no results")
                || lower.starts_with("no search results")
                || lower.starts_with("no relevant results");
            sources.extend(parse_sources(text)?);
        }
        if sources.is_empty() && !explicit_empty {
            return Err(protocol_error(
                "Exa 响应没有可识别的来源，也未明确报告无结果。",
            ));
        }
        let limited = sources.len() > request.num_results as usize;
        sources.truncate(request.num_results as usize);
        Ok((sources, limited))
    }

    pub async fn fetch(
        &self,
        url: Url,
        confirm: &mut dyn FnMut(&str) -> io::Result<bool>,
    ) -> ToolOutput {
        let original = url.clone();
        let mut current = url;
        let mut remaining = self.fetch_timeout;
        let mut redirects = 0;
        loop {
            if remaining.is_zero() {
                return error_output(
                    "web_fetch",
                    Some(&original),
                    Some(&current),
                    ToolError::new(
                        "network_timeout",
                        "网络访问超过总耗时限制。",
                        "稍后重试或选择响应更快的来源。",
                    ),
                );
            }
            let started = Instant::now();
            let step = timed(remaining, self.fetch_step(current.clone())).await;
            remaining = remaining.saturating_sub(started.elapsed());
            match step {
                Err(error) => {
                    return error_output("web_fetch", Some(&original), Some(&current), error);
                }
                Ok(FetchStep::Page {
                    status,
                    content_type,
                    bytes,
                }) => {
                    let byte_count = bytes.len();
                    let text = match readable_text(bytes, &content_type).await {
                        Ok(text) => text,
                        Err(error) => {
                            return error_output(
                                "web_fetch",
                                Some(&original),
                                Some(&current),
                                error,
                            );
                        }
                    };
                    return ToolOutput::ok(
                        bounded_result(
                            json!({"tool":"web_fetch", "status":"ok",
                        "url":original.as_str(), "final_url":current.as_str(), "http_status":status,
                        "content_type":content_type, "bytes":byte_count, "redirects":redirects,
                        "untrusted":true, "truncated":false}),
                            &text,
                            MAX_OUTPUT,
                        ),
                        false,
                    );
                }
                Ok(FetchStep::Redirect(next)) => {
                    if redirects == MAX_REDIRECTS {
                        return error_output(
                            "web_fetch",
                            Some(&original),
                            Some(&current),
                            ToolError::new(
                                "too_many_redirects",
                                "网页重定向超过 5 次。",
                                "确认最终地址后重新请求该 URL。",
                            ),
                        );
                    }
                    if current.origin() != next.origin() {
                        let prompt = format!(
                            "\n工具 web_fetch 请求跨来源重定向授权：\n来源 URL：{current}\n目标 URL：{next}\n仅允许访问本次目标地址。[y/N] "
                        );
                        let result = confirm(&prompt);
                        let error = match result {
                            Ok(true) => None,
                            Ok(false) => Some(ToolError::new(
                                "authorization_denied",
                                "用户拒绝重定向，目标地址未访问。",
                                "停止此访问；不要改用其他工具绕过拒绝。",
                            )),
                            Err(error) => Some(ToolError::new(
                                "authorization_failed",
                                error.to_string(),
                                "恢复交互输入后重新请求授权。",
                            )),
                        };
                        if let Some(error) = error {
                            return error_output(
                                "web_fetch",
                                Some(&original),
                                Some(&current),
                                error,
                            );
                        }
                    }
                    redirects += 1;
                    current = next;
                }
            }
        }
    }

    async fn fetch_step(&self, url: Url) -> Result<FetchStep, ToolError> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(network_error)?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| protocol_error("重定向响应缺少合法 Location。"))?;
            let target = url
                .join(location)
                .map_err(|error| protocol_error(error.to_string()))?;
            return Ok(FetchStep::Redirect(checked_url(target.as_str())?));
        }
        check_status(&response)?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("text/plain")
            .to_owned();
        check_content_type(&content_type)?;
        Ok(FetchStep::Page {
            status,
            content_type,
            bytes: bounded_body(response).await?,
        })
    }
}

enum FetchStep {
    Redirect(Url),
    Page {
        status: u16,
        content_type: String,
        bytes: Vec<u8>,
    },
}

async fn timed<T>(
    limit: Duration,
    future: impl std::future::Future<Output = Result<T, ToolError>>,
) -> Result<T, ToolError> {
    tokio::time::timeout(limit, future).await.map_err(|_| {
        ToolError::new(
            "network_timeout",
            "网络访问超过总耗时限制。",
            "稍后重试或选择响应更快的来源。",
        )
    })?
}

async fn bounded_body(mut response: Response) -> Result<Vec<u8>, ToolError> {
    let too_large = || {
        ToolError::new(
            "response_too_large",
            "响应体超过 1 MiB，读取已停止。",
            "选择更小或更具体的页面；未返回完整响应。",
        )
    };
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BODY as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        if chunk.len() > MAX_BODY.saturating_sub(bytes.len()) {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn check_status(response: &Response) -> Result<(), ToolError> {
    if response.status().is_success() {
        return Ok(());
    }
    let code = match response.status().as_u16() {
        403 => "http_forbidden",
        429 => "http_rate_limited",
        _ => "http_error",
    };
    Err(ToolError::new(
        code,
        format!("HTTP {}", response.status()),
        "检查地址或稍后重试；HTTP 错误不是空结果。",
    ))
}

fn network_error(error: reqwest::Error) -> ToolError {
    ToolError::new(
        if error.is_timeout() {
            "network_timeout"
        } else {
            "network_error"
        },
        error.to_string(),
        "检查网络、代理或 TLS 状态后重试。",
    )
}

fn protocol_error(message: impl Into<String>) -> ToolError {
    ToolError::new(
        "protocol_error",
        message,
        "服务返回格式不符合约定；检查服务状态后重试。",
    )
}

pub(super) fn error_output(
    tool: &str,
    url: Option<&Url>,
    final_url: Option<&Url>,
    error: ToolError,
) -> ToolOutput {
    ToolOutput::error(bounded_result(
        json!({"tool":tool, "status":"error", "provider":if tool=="web_search" {Some("exa")} else {None},
        "url":url.map(Url::as_str), "final_url":final_url.map(Url::as_str),
        "code":error.code, "field":error.field, "message":error.message, "hint":error.hint,
        "truncated":false, "changed":false}),
        "",
        MAX_OUTPUT,
    ))
}

fn rpc_response(bytes: &[u8]) -> Result<Value, ToolError> {
    let text = std::str::from_utf8(bytes).map_err(|error| protocol_error(error.to_string()))?;
    let mut messages = Vec::new();
    if text.trim_start().starts_with('{') {
        messages.push(
            serde_json::from_str::<Value>(text)
                .map_err(|error| protocol_error(error.to_string()))?,
        );
    } else {
        let mut data = Vec::new();
        for line in text.lines().chain(std::iter::once("")) {
            if line.is_empty() {
                if !data.is_empty() {
                    let event = data.join("\n");
                    messages.push(
                        serde_json::from_str::<Value>(&event)
                            .map_err(|error| protocol_error(error.to_string()))?,
                    );
                    data.clear();
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                data.push(value.strip_prefix(' ').unwrap_or(value));
            }
        }
    }
    messages
        .into_iter()
        .find(|value| {
            value["jsonrpc"] == "2.0"
                && value["id"] == 1
                && (value.get("result").is_some() || value.get("error").is_some())
        })
        .ok_or_else(|| protocol_error("未收到配对的 JSON-RPC 响应。"))
}

#[derive(Default)]
struct Source {
    title: String,
    url: String,
    snippet: String,
}

fn parse_sources(text: &str) -> Result<Vec<Source>, ToolError> {
    let mut sources = Vec::new();
    let mut current = Source::default();
    let mut highlights = false;
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("Title: ") {
            if !current.url.is_empty() {
                sources.push(current);
            }
            current = Source {
                title: title.to_owned(),
                ..Source::default()
            };
            highlights = false;
        } else if let Some(url) = line.strip_prefix("URL: ") {
            checked_url(url.trim()).map_err(|_| protocol_error("搜索来源 URL 无效或过长。"))?;
            current.url = url.trim().to_owned();
        } else if line.starts_with("Highlights:") || line.starts_with("Text:") {
            highlights = true;
            let extra = line
                .split_once(':')
                .map(|(_, extra)| extra.trim_start())
                .unwrap_or("");
            current.snippet.push_str(extra);
        } else if highlights && line != "---" {
            current.snippet.push('\n');
            current.snippet.push_str(line);
        }
    }
    if !current.url.is_empty() {
        sources.push(current);
    }
    Ok(sources)
}

fn search_output(request: &SearchRequest, sources: Vec<Source>, mut truncated: bool) -> ToolOutput {
    let mut body = String::new();
    let mut count = 0;
    // 先为所有标题和完整 URL 留位，余下预算分给摘要；绝不留下半个来源地址。
    let headers: Vec<String> = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            format!(
                "{}. {}\nURL: {}\n",
                index + 1,
                source.title.chars().take(300).collect::<String>(),
                source.url
            )
        })
        .collect();
    let mut available = MAX_OUTPUT - 700;
    let mut kept = 0;
    for text in &headers {
        if text.chars().count() + 2 > available {
            truncated = true;
            break;
        }
        available -= text.chars().count() + 2;
        kept += 1;
    }
    for (index, (source, title)) in sources.iter().zip(headers).take(kept).enumerate() {
        body.push_str(&title);
        let allowance = available / (kept - index);
        let snippet: String = source.snippet.trim().chars().take(allowance).collect();
        truncated |= source.snippet.trim().chars().count() > snippet.chars().count()
            || source.title.chars().count() > 300;
        available -= snippet.chars().count();
        body.push_str(&snippet);
        body.push_str("\n\n");
        count += 1;
    }
    ToolOutput::ok(
        bounded_result(
            json!({"tool":"web_search", "status":"ok", "provider":"exa",
        "results":count, "requested":request.num_results, "untrusted":true, "truncated":truncated}),
            &body,
            MAX_OUTPUT,
        ),
        false,
    )
}

fn check_content_type(content_type: &str) -> Result<(), ToolError> {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime.starts_with("text/")
        || matches!(
            mime.as_str(),
            "application/json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-javascript"
        )
        || mime.ends_with("+json")
        || mime.ends_with("+xml")
    {
        Ok(())
    } else {
        Err(ToolError::new(
            "unsupported_content_type",
            format!("不支持的二进制响应类型：{mime}"),
            "仅抓取 HTML、Markdown、JSON 等文本；PDF 和其他二进制格式暂不支持。",
        ))
    }
}

async fn readable_text(bytes: Vec<u8>, content_type: &str) -> Result<String, ToolError> {
    let declared = content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['\'', '"']))
    });
    let encoding = match declared {
        Some(label) => encoding_rs::Encoding::for_label(label.as_bytes()).ok_or_else(|| {
            ToolError::new(
                "unsupported_charset",
                format!("未知字符编码：{label}"),
                "选择使用标准字符编码的文本页面。",
            )
        })?,
        None => encoding_rs::Encoding::for_bom(&bytes)
            .map_or(encoding_rs::UTF_8, |(encoding, _)| encoding),
    };
    let (text, _, invalid) = encoding.decode(&bytes);
    if invalid || text.contains('\0') {
        return Err(ToolError::new(
            "invalid_text",
            "响应包含二进制数据或无法按声明编码解码。",
            "选择可读文本页面；不会把二进制误当作正文。",
        ));
    }
    let mime = content_type.split(';').next().unwrap_or("").trim();
    if mime.eq_ignore_ascii_case("text/html") || mime.eq_ignore_ascii_case("application/xhtml+xml")
    {
        let owned = text.into_owned();
        tokio::task::spawn_blocking(move || {
            html2text::config::plain()
                .no_link_wrapping()
                .allow_width_overflow()
                .string_from_read(owned.as_bytes(), 120)
        })
        .await
        .map_err(|error| {
            ToolError::new(
                "html_parse_error",
                error.to_string(),
                "选择其他静态文本来源。",
            )
        })?
        .map_err(|error| {
            ToolError::new(
                "html_parse_error",
                error.to_string(),
                "选择其他静态文本来源。",
            )
        })
    } else {
        Ok(text.into_owned())
    }
}

#[cfg(test)]
mod tests;
