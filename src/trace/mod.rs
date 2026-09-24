//! 模型调用的记录契约；不包含具体数据库类型。

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TraceStatus {
    Completed,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TraceRecord {
    pub(crate) request_id: String,
    pub(crate) session_id: String,
    pub(crate) agent_run_id: String,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) api: String,
    pub(crate) model: String,
    pub(crate) started_at_ms: i64,
    pub(crate) duration_ms: i64,
    pub(crate) attempts: i32,
    pub(crate) status: TraceStatus,
    pub(crate) input_tokens: Option<i64>,
    pub(crate) output_tokens: Option<i64>,
    pub(crate) request: Value,
    pub(crate) response: Option<Value>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TraceError(pub(crate) String);

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for TraceError {}

#[derive(Debug)]
#[allow(dead_code)] // 批量接口本次供代码调用和契约测试，不由 REPL 直接读取。
pub(crate) struct BatchWriteItem {
    pub(crate) request_id: String,
    pub(crate) result: Result<(), TraceError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TraceCursor {
    pub(crate) started_at_ms: i64,
    pub(crate) request_id: String,
}

#[derive(Debug)]
#[allow(dead_code)] // 分页 Reader 本次不暴露终端命令。
pub(crate) struct TracePage {
    pub(crate) items: Vec<TraceRecord>,
    pub(crate) next_cursor: Option<TraceCursor>,
}

#[allow(dead_code)] // 读取接口由后端契约测试和后续程序调用，本次不增加 REPL 查询命令。
pub(crate) trait TraceWriter {
    async fn write_one(&self, record: &TraceRecord) -> Result<(), TraceError>;
    async fn write_batch(&self, records: &[TraceRecord])
    -> Result<Vec<BatchWriteItem>, TraceError>;
}

#[allow(dead_code)] // 读取接口由后端契约测试和后续程序调用，本次不增加 REPL 查询命令。
pub(crate) trait TraceReader {
    async fn get_one(&self, request_id: &str) -> Result<Option<TraceRecord>, TraceError>;
    async fn get_batch(
        &self,
        request_ids: &[String],
    ) -> Result<Vec<Option<TraceRecord>>, TraceError>;
    async fn list_session_page(
        &self,
        session_id: &str,
        after: Option<&TraceCursor>,
        limit: u64,
    ) -> Result<TracePage, TraceError>;
}

#[derive(Default)]
pub(crate) struct TraceCapture {
    pub(crate) request: Value,
    response: Option<Value>,
    partial_text: String,
    partial_calls: BTreeMap<u32, PartialCall>,
    pub(crate) provider_response_id: Option<String>,
    pub(crate) attempts: i32,
    attempt_counter: Option<Arc<AtomicI32>>,
}

#[derive(Default, Serialize)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

impl TraceCapture {
    pub(crate) fn new() -> Self {
        Self {
            attempts: 1,
            ..Self::default()
        }
    }

    pub(crate) fn count_attempts_with(&mut self, counter: Arc<AtomicI32>) {
        self.attempt_counter = Some(counter);
    }

    pub(crate) fn attempts(&self) -> i32 {
        self.attempt_counter
            .as_ref()
            .map_or(self.attempts, |counter| {
                counter.load(Ordering::Relaxed).max(1)
            })
    }

    pub(crate) fn clear_partial(&mut self) {
        self.response = None;
        self.partial_text.clear();
        self.partial_calls.clear();
        self.provider_response_id = None;
    }

    pub(crate) fn set_request<T: Serialize>(
        &mut self,
        request: &T,
    ) -> Result<(), serde_json::Error> {
        self.request = serde_json::to_value(request)?;
        Ok(())
    }

    pub(crate) fn append_text(&mut self, text: &str) {
        self.partial_text.push_str(text);
    }

    pub(crate) fn append_call_delta(
        &mut self,
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
    ) {
        let call = self.partial_calls.entry(index).or_default();
        if let Some(id) = id {
            call.id.push_str(id);
        }
        if let Some(name) = name {
            call.name.push_str(name);
        }
        if let Some(arguments) = arguments {
            call.arguments.push_str(arguments);
        }
    }

    pub(crate) fn replace_call(&mut self, index: u32, id: &str, name: &str, arguments: &str) {
        self.partial_calls.insert(
            index,
            PartialCall {
                id: id.to_owned(),
                name: name.to_owned(),
                arguments: arguments.to_owned(),
            },
        );
    }

    pub(crate) fn set_response<T: Serialize>(
        &mut self,
        response: &T,
    ) -> Result<(), serde_json::Error> {
        self.response = Some(serde_json::to_value(response)?);
        Ok(())
    }

    pub(crate) fn set_failure_response<T: Serialize>(
        &mut self,
        response: &T,
    ) -> Result<(), serde_json::Error> {
        let mut value = serde_json::to_value(response)?;
        if !self.partial_text.is_empty() || !self.partial_calls.is_empty() {
            let partial = json!({
                "text": self.partial_text,
                "tool_calls": self.partial_calls,
            });
            if let Value::Object(fields) = &mut value {
                fields.insert("received_before_failure".to_owned(), partial);
            } else {
                value = json!({"response": value, "received_before_failure": partial});
            }
        }
        self.response = Some(value);
        Ok(())
    }

    pub(crate) fn response(&self) -> Option<Value> {
        self.response.clone().or_else(|| {
            (!self.partial_text.is_empty() || !self.partial_calls.is_empty()).then(|| {
                json!({
                    "text": self.partial_text,
                    "tool_calls": self.partial_calls,
                })
            })
        })
    }
}

pub(crate) fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn redact_text(text: &str, secrets: &[&str]) -> String {
    secrets
        .iter()
        .filter(|secret| !secret.is_empty())
        .fold(text.to_owned(), |text, secret| {
            text.replace(*secret, "[REDACTED]")
        })
}

pub(crate) fn redact_json(value: &mut Value, secrets: &[&str]) {
    match value {
        Value::String(text) => *text = redact_text(text, secrets),
        Value::Array(items) => items.iter_mut().for_each(|item| redact_json(item, secrets)),
        Value::Object(fields) => {
            let old = std::mem::take(fields);
            for (key, mut value) in old {
                redact_json(&mut value, secrets);
                fields.insert(redact_text(&key, secrets), value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{TraceCapture, redact_json};

    #[test]
    fn partial_capture_combines_stream_chunks() {
        let mut capture = TraceCapture::new();
        capture.append_text("hel");
        capture.append_text("lo");
        capture.append_call_delta(0, Some("id"), Some("read"), Some("{"));
        capture.append_call_delta(0, None, None, Some("}"));
        let response = capture.response().unwrap();
        assert_eq!(response["text"], "hello");
        assert_eq!(response["tool_calls"]["0"]["arguments"], "{}");
    }

    #[test]
    fn configured_secret_is_removed_from_nested_request_and_response() {
        let mut value =
            serde_json::json!({"input": ["api-key", {"url": "mongodb://user:pass@host/db"}]});
        redact_json(&mut value, &["api-key", "mongodb://user:pass@host/db"]);
        assert_eq!(value["input"][0], "[REDACTED]");
        assert_eq!(value["input"][1]["url"], "[REDACTED]");
    }
}
