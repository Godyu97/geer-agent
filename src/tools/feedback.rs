use std::{io, path::Path};

use serde_json::{Value, json};

use super::{ToolOutput, bash::MAX_RESULT_CHARS};

#[derive(Debug)]
pub(super) struct ToolError {
    pub code: &'static str,
    pub message: String,
    pub hint: &'static str,
    pub field: Option<String>,
}

impl ToolError {
    pub fn new(code: &'static str, message: impl Into<String>, hint: &'static str) -> Self {
        Self {
            code,
            message: message.into(),
            hint,
            field: None,
        }
    }

    pub fn at(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn invalid(
        field: impl Into<String>,
        message: impl Into<String>,
        hint: &'static str,
    ) -> Self {
        Self::new("invalid_argument", message, hint).at(field)
    }

    pub fn io(error: io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => "not_found",
            io::ErrorKind::PermissionDenied => "permission_denied",
            io::ErrorKind::InvalidData => "invalid_utf8",
            _ => "io_error",
        };
        Self::new(
            code,
            error.to_string(),
            "核对 path、文件类型和访问权限后重试；文本工具只接受 UTF-8。",
        )
    }

    pub fn output(self, tool: &str, path: Option<&Path>) -> ToolOutput {
        let message = if tool == "read" {
            format!("读取失败，未获得文件正文：{}", self.message)
        } else {
            self.message
        };
        let mut metadata = json!({
            "tool": tool, "status": "error", "path": path,
            "code": self.code, "field": self.field, "message": message,
            "hint": self.hint, "changed": false,
        });
        if tool == "read" {
            metadata["empty"] = Value::Null;
        }
        if matches!(tool, "ls" | "glob" | "rg" | "search") {
            metadata["exit_code"] = Value::Null;
            metadata["truncated"] = json!(false);
        }
        let max = if matches!(tool, "web_search" | "web_fetch") {
            12000
        } else {
            MAX_RESULT_CHARS
        };
        ToolOutput::error(bounded_result(metadata, "", max))
    }
}

pub(super) fn fields(args: &Value, allowed: &[&str], prefix: &str) -> Result<(), ToolError> {
    let map = args.as_object().ok_or_else(|| {
        ToolError::invalid(
            prefix,
            "参数必须是 JSON 对象。",
            "传入对象，不要使用数组或 null。",
        )
    })?;
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ToolError::invalid(
                format!("{prefix}{key}"),
                "不接受此字段。",
                "删除未知字段，按工具定义提供参数。",
            ));
        }
    }
    Ok(())
}

pub(super) fn string<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    args.get(name).and_then(Value::as_str).ok_or_else(|| {
        ToolError::invalid(
            name,
            "必须提供字符串。",
            "使用字符串；可选字段使用默认值时请省略，不要传 null。",
        )
    })
}

pub(super) fn optional_string<'a>(
    args: &'a Value,
    name: &str,
) -> Result<Option<&'a str>, ToolError> {
    args.get(name).map(|_| string(args, name)).transpose()
}

// 元信息单独序列化，截断正文时不会留下损坏的 JSON；超长参数也不能挤掉诊断。
pub(super) fn bounded_result(metadata: Value, body: &str, max: usize) -> String {
    render_result(metadata, body, max, false)
}

pub(super) fn bounded_lines_result(metadata: Value, body: &str, max: usize) -> String {
    render_result(metadata, body, max, true)
}

fn render_result(mut metadata: Value, body: &str, max: usize, complete_lines: bool) -> String {
    let web = matches!(metadata["tool"].as_str(), Some("web_search" | "web_fetch"));
    let mut shortened = false;
    if let Some(object) = metadata.as_object_mut() {
        for (key, value) in object.iter_mut() {
            if web && matches!(key.as_str(), "url" | "final_url") {
                continue;
            }
            if let Some(text) = value.as_str()
                && value.to_string().chars().count() > 160
            {
                let mut short: String = text.chars().take(150).collect();
                while json!(format!("{short}…")).to_string().chars().count() > 160 {
                    short.pop();
                }
                *value = json!(format!("{short}…"));
                shortened = true;
            }
        }
    }
    if shortened {
        metadata["metadata_truncated"] = json!(true);
    }
    let mut header = metadata.to_string();
    let truncated =
        metadata["truncated"] == true || header.chars().count() + 2 + body.chars().count() > max;
    let marker = if truncated {
        match metadata["tool"].as_str() {
            Some("web_search") => {
                "\n[输出已截断；请收窄 query 或减少 num_results，再用 web_fetch 读取来源。]"
            }
            Some("web_fetch") => "\n[输出已截断；尚未获得完整网页正文，请选择更具体的页面。]",
            Some("memory_search" | "memory_write") => {
                "\n[记忆输出已截断；请缩小搜索关键词，完整内容可在记忆管理界面查看。]"
            }
            _ => "\n[输出已截断；请缩小 path/pattern/glob 后重试，或用 read 读取已定位文件。]",
        }
    } else {
        ""
    };
    if truncated {
        metadata["truncated"] = json!(true);
        header = metadata.to_string();
    }
    let keep = max.saturating_sub(header.chars().count() + 2 + marker.chars().count());
    let mut content: String = body.chars().take(keep).collect();
    if complete_lines && content.len() < body.len() {
        content.truncate(content.rfind('\n').map_or(0, |index| index + 1));
    }
    format!("{header}\n\n{}{marker}", content)
}
