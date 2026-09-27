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
        let mut metadata = json!({
            "tool": tool, "status": "error", "path": path,
            "code": self.code, "field": self.field, "message": self.message,
            "hint": self.hint, "changed": false,
        });
        if matches!(tool, "ls" | "glob" | "rg") {
            metadata["exit_code"] = Value::Null;
            metadata["truncated"] = json!(false);
        }
        ToolOutput::error(bounded_result(metadata, "", MAX_RESULT_CHARS))
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
pub(super) fn bounded_result(mut metadata: Value, body: &str, max: usize) -> String {
    let mut shortened = false;
    if let Some(object) = metadata.as_object_mut() {
        for value in object.values_mut() {
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
        "\n[输出已截断；请缩小 path/pattern/glob 后重试，或用 read 读取已定位文件。]"
    } else {
        ""
    };
    if truncated {
        metadata["truncated"] = json!(true);
        header = metadata.to_string();
    }
    let keep = max.saturating_sub(header.chars().count() + 2 + marker.chars().count());
    format!(
        "{header}\n\n{}{marker}",
        body.chars().take(keep).collect::<String>()
    )
}
