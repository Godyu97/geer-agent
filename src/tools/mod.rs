//! 工具定义与执行：不引用 `provider` / `repl` / `agent`。

mod bash;
mod file;

use std::{
    collections::HashSet,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

use chrono::Local;
use serde_json::{Value, json};

type ConfirmFn = Box<dyn FnMut(&str) -> io::Result<bool>>;

#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub(crate) struct Tools {
    enabled: bool,
    cwd: PathBuf,
    grants: HashSet<String>,
    confirm: ConfirmFn,
}

impl Tools {
    pub(crate) fn new(enabled: bool) -> io::Result<Self> {
        Ok(Self {
            enabled,
            cwd: std::env::current_dir()?,
            grants: HashSet::new(),
            confirm: Box::new(confirm_cli),
        })
    }

    pub(crate) fn reset(&mut self) {
        self.grants.clear();
    }

    pub(crate) fn specs(&self) -> Vec<Spec> {
        if self.enabled {
            Self::definitions()
        } else {
            Vec::new()
        }
    }

    fn definitions() -> Vec<Spec> {
        vec![
            Spec {
                name: "get_current_time",
                description: "获取当前系统本地日期与时间。",
                parameters: json!({"type":"object","properties":{},"additionalProperties":false}),
            },
            Spec {
                name: "bash",
                description: "在启动目录执行 Bash 命令；首次调用需要本次会话授权。",
                parameters: json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false}),
            },
            Spec {
                name: "read",
                description: "读取 UTF-8 文本文件，支持从指定行继续读取；首次调用需要本次会话授权。",
                parameters: json!({"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer","minimum":1},"limit":{"type":"integer","minimum":1}},"required":["path"],"additionalProperties":false}),
            },
            Spec {
                name: "write",
                description: "创建或覆盖文本文件，自动创建父目录；首次调用需要本次会话授权。",
                parameters: json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}),
            },
            Spec {
                name: "edit",
                description: "对一个文件做多处精确文本替换；每个 oldText 必须在原文件中唯一且互不重叠。",
                parameters: json!({"type":"object","properties":{"path":{"type":"string"},"edits":{"type":"array","minItems":1,"items":{"type":"object","properties":{"oldText":{"type":"string"},"newText":{"type":"string"}},"required":["oldText","newText"],"additionalProperties":false}}},"required":["path","edits"],"additionalProperties":false}),
            },
        ]
    }

    pub(crate) async fn execute(&mut self, name: &str, args_json: &str) -> String {
        if !self.enabled {
            return "工具已关闭。".to_owned();
        }
        let args: Value = match serde_json::from_str(args_json) {
            Ok(value) => value,
            Err(error) => return format!("工具参数不是合法 JSON：{error}"),
        };
        if !args.is_object() {
            return "工具参数必须是 JSON 对象。".to_owned();
        }
        match name {
            "get_current_time" if args.as_object().is_some_and(|map| map.is_empty()) => {
                Local::now().to_rfc3339()
            }
            "get_current_time" => "get_current_time 不接受参数。".to_owned(),
            "bash" => {
                let Some(command) = args
                    .get("command")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                else {
                    return "bash 需要非空 command 字符串。".to_owned();
                };
                match self.authorize("bash", &format!("命令：{command}")) {
                    Ok(true) => bash::run(&self.cwd, command).await,
                    Ok(false) => "用户拒绝授权，命令未执行。".to_owned(),
                    Err(error) => format!("无法确认 Bash 命令：{error}"),
                }
            }
            "read" | "write" | "edit" => {
                let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
                    return format!("{name} 需要 path 字符串。");
                };
                let path = match file::resolve_path(&self.cwd, raw_path) {
                    Ok(path) => path,
                    Err(error) => return error,
                };
                let operation = match file::Operation::parse(name, &args) {
                    Ok(operation) => operation,
                    Err(error) => return error,
                };
                match self.authorize(name, &format!("路径：{}", path.display())) {
                    Ok(true) => operation.run(&path),
                    Ok(false) => format!("用户拒绝授权，{name} 未执行。"),
                    Err(error) => format!("无法确认 {name}：{error}"),
                }
            }
            _ => format!("未知工具：{name}"),
        }
    }

    fn authorize(&mut self, name: &str, detail: &str) -> io::Result<bool> {
        if self.grants.contains(name) {
            return Ok(true);
        }
        let prompt = format!(
            "\n工具 {name} 请求授权：\n{detail}\n允许后，本次会话内该工具可继续访问本机任意路径或执行命令。[/reset 可撤销] [y/N] "
        );
        if (self.confirm)(&prompt)? {
            self.grants.insert(name.to_owned());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn confirm_cli(prompt: &str) -> io::Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(prompt.as_bytes())?;
    stdout.flush()?;
    drop(stdout);
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        rc::Rc,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::Tools;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[tokio::test]
    async fn disabled_and_invalid_arguments() {
        let mut tools = Tools::new(false).expect("工作目录存在");
        assert!(tools.specs().is_empty());
        assert_eq!(Tools::new(true).expect("工作目录存在").specs().len(), 5);
        assert_eq!(
            tools.execute("get_current_time", "{}").await,
            "工具已关闭。"
        );
        let mut tools = Tools::new(true).expect("工作目录存在");
        assert!(
            tools
                .execute("get_current_time", "{")
                .await
                .contains("JSON")
        );
        assert!(
            tools
                .execute("get_current_time", "[]")
                .await
                .contains("对象")
        );
        assert!(
            tools
                .execute("get_current_time", "{\"x\":1}")
                .await
                .contains("不接受")
        );
        assert!(tools.execute("unknown", "{}").await.contains("未知"));
    }

    #[tokio::test]
    async fn grants_are_per_tool_and_reset_revokes_them() {
        let mut tools = Tools::new(true).expect("工作目录存在");
        let prompts = Rc::new(Cell::new(0));
        let seen = Rc::clone(&prompts);
        tools.confirm = Box::new(move |prompt| {
            assert!(prompt.contains("本次会话"));
            seen.set(seen.get() + 1);
            Ok(true)
        });
        for _ in 0..2 {
            let result = tools.execute("bash", r#"{"command":"printf ok"}"#).await;
            assert!(result.contains("退出状态：0"), "{result}");
        }
        assert_eq!(prompts.get(), 1);
        tools.reset();
        tools.execute("bash", r#"{"command":"printf ok"}"#).await;
        assert_eq!(prompts.get(), 2);

        tools.reset();
        tools.confirm = Box::new(|_| Ok(false));
        let denied = tools
            .execute("bash", r#"{"command":"printf denied"}"#)
            .await;
        assert!(denied.contains("未执行"));
        assert!(!tools.grants.contains("bash"));
    }

    #[tokio::test]
    async fn file_tools_use_independent_session_grants_and_absolute_paths() {
        let dir = std::env::temp_dir().join(format!(
            "geer-agent-auth-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("创建目录");
        let path = dir.join("file.txt");
        fs::write(&path, "hello").expect("准备文件");
        let path_json =
            serde_json::to_string(&path.to_string_lossy().to_string()).expect("编码路径");
        let mut tools = Tools::new(true).expect("工作目录存在");
        let prompts = Rc::new(Cell::new(0));
        let seen = Rc::clone(&prompts);
        tools.confirm = Box::new(move |prompt| {
            assert!(prompt.contains("路径："));
            seen.set(seen.get() + 1);
            Ok(true)
        });
        let read = format!("{{\"path\":{path_json}}}");
        assert_eq!(tools.execute("read", &read).await, "hello");
        assert_eq!(tools.execute("read", &read).await, "hello");
        assert_eq!(prompts.get(), 1);
        let write = format!("{{\"path\":{path_json},\"content\":\"world\"}}");
        assert!(tools.execute("write", &write).await.contains("已写入"));
        assert_eq!(prompts.get(), 2);
        let edit = format!(
            "{{\"path\":{path_json},\"edits\":[{{\"oldText\":\"world\",\"newText\":\"done\"}}]}}"
        );
        assert!(tools.execute("edit", &edit).await.contains("已编辑"));
        assert_eq!(prompts.get(), 3);
        tools.reset();
        assert_eq!(tools.execute("read", &read).await, "done");
        assert_eq!(prompts.get(), 4);
        fs::remove_dir_all(dir).expect("清理目录");
    }
}
