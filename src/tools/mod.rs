//! 工具定义与执行：不引用 `provider` / `repl` / `agent`。

mod bash;
mod feedback;
mod file;
mod query;

use std::{
    collections::HashSet,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Local;
use futures_util::future::join_all;
use serde_json::{Value, json};

use feedback::{ToolError, fields, optional_string, string};

type ConfirmFn = Box<dyn FnMut(&str) -> io::Result<bool>>;

#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Time,
    Bash,
    Ls,
    Glob,
    Rg,
    Read,
    Write,
    Edit,
}

const BUILTINS: &[ToolKind] = &[
    ToolKind::Time,
    ToolKind::Bash,
    ToolKind::Ls,
    ToolKind::Glob,
    ToolKind::Rg,
    ToolKind::Read,
    ToolKind::Write,
    ToolKind::Edit,
];

impl ToolKind {
    fn name(self) -> &'static str {
        match self {
            Self::Time => "get_current_time",
            Self::Bash => "bash",
            Self::Ls => "ls",
            Self::Glob => "glob",
            Self::Rg => "rg",
            Self::Read => "read",
            Self::Write => "write",
            Self::Edit => "edit",
        }
    }

    fn find(name: &str) -> Option<Self> {
        BUILTINS.iter().copied().find(|kind| kind.name() == name)
    }

    fn mode(self) -> ExecutionMode {
        match self {
            Self::Ls | Self::Glob | Self::Rg | Self::Read => ExecutionMode::Parallel,
            Self::Time | Self::Bash | Self::Write | Self::Edit => ExecutionMode::Sequential,
        }
    }
}

fn validate_registry(kinds: &[ToolKind]) -> io::Result<()> {
    let mut names = HashSet::new();
    for kind in kinds {
        if !names.insert(kind.name()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("重复工具名：{}", kind.name()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionMode {
    Parallel,
    Sequential,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolOutput {
    pub text: String,
    pub success: bool,
    pub changed: bool,
}

#[derive(Debug)]
pub(crate) struct ToolExecution {
    pub output: ToolOutput,
    pub duration: Duration,
}

enum PreparedCall {
    Time,
    Bash(String),
    File {
        kind: ToolKind,
        path: PathBuf,
        operation: file::Operation,
    },
    Query {
        path: PathBuf,
        query: query::Query,
    },
}

impl ToolOutput {
    fn ok(text: String, changed: bool) -> Self {
        Self {
            text,
            success: true,
            changed,
        }
    }
    fn error(text: String) -> Self {
        Self {
            text,
            success: false,
            changed: false,
        }
    }
}

pub(crate) struct Tools {
    enabled: bool,
    cwd: PathBuf,
    bash_bin: PathBuf,
    grants: HashSet<String>,
    confirm: ConfirmFn,
}

impl Tools {
    #[cfg(test)]
    pub(crate) fn allow_all_for_test(&mut self) {
        self.confirm = Box::new(|_| Ok(true));
    }

    #[cfg(test)]
    pub(crate) fn grant_for_test(&mut self, name: &str) {
        self.grants.insert(name.to_owned());
    }

    #[cfg(test)]
    pub(crate) fn granted_for_test(&self, name: &str) -> bool {
        self.grants.contains(name)
    }

    pub(crate) fn execution_mode(name: &str) -> ExecutionMode {
        ToolKind::find(name).map_or(ExecutionMode::Sequential, ToolKind::mode)
    }

    pub(crate) fn new(enabled: bool, bash_bin: PathBuf) -> io::Result<Self> {
        validate_registry(BUILTINS)?;
        Ok(Self {
            enabled,
            cwd: std::env::current_dir()?,
            bash_bin,
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

    pub(crate) async fn execute_batch(&mut self, calls: &[(&str, &str)]) -> Vec<ToolExecution> {
        let mut results = Vec::with_capacity(calls.len());
        let mut index = 0;
        while index < calls.len() {
            if Self::execution_mode(calls[index].0) == ExecutionMode::Sequential {
                let started = Instant::now();
                let output = self.execute_recorded(calls[index].0, calls[index].1).await;
                results.push(ToolExecution {
                    output,
                    duration: started.elapsed(),
                });
                index += 1;
                continue;
            }
            let mut prepared = Vec::new();
            while index < calls.len()
                && Self::execution_mode(calls[index].0) == ExecutionMode::Parallel
            {
                prepared.push(self.prepare(calls[index].0, calls[index].1));
                index += 1;
            }
            // 所有授权在派发前逐项完成；join_all 保留输入次序，写入和 Bash 是段边界。
            let pending = prepared.into_iter().map(|job| async {
                let started = Instant::now();
                let output = match job {
                    Ok(job) => Self::run_prepared(&self.bash_bin, &self.cwd, job).await,
                    Err(output) => output,
                };
                ToolExecution {
                    output,
                    duration: started.elapsed(),
                }
            });
            results.extend(join_all(pending).await);
        }
        results
    }

    fn prepare(&mut self, name: &str, args_json: &str) -> Result<PreparedCall, ToolOutput> {
        if !self.enabled {
            return Err(ToolOutput::error("工具已关闭。".to_owned()));
        }
        let mut path = None;
        let prepared = (|| -> Result<PreparedCall, ToolError> {
            let args: Value = serde_json::from_str(args_json).map_err(|_| {
                ToolError::invalid("$", "参数不是合法 JSON。", "使用工具定义要求的 JSON 对象。")
            })?;
            if !args.is_object() {
                return Err(ToolError::invalid(
                    "$",
                    "参数必须是 JSON 对象。",
                    "提供字段对象，不要使用数组或 null。",
                ));
            }
            let kind = ToolKind::find(name).ok_or_else(|| {
                ToolError::new("unknown_tool", "未知工具。", "使用当前声明的工具名。")
            })?;
            match kind {
                ToolKind::Time => {
                    fields(&args, &[], "")?;
                    Ok(PreparedCall::Time)
                }
                ToolKind::Bash => {
                    fields(&args, &["command"], "")?;
                    let command = string(&args, "command")?;
                    if command.trim().is_empty() || command.contains('\0') {
                        return Err(ToolError::invalid(
                            "command",
                            "命令不能为空或包含 NUL。",
                            "提供有效 Bash 命令。",
                        ));
                    }
                    Ok(PreparedCall::Bash(command.to_owned()))
                }
                ToolKind::Ls | ToolKind::Glob | ToolKind::Rg => {
                    let query = query::Query::parse(kind, &args)?;
                    let resolved = file::resolve_path(
                        &self.cwd,
                        optional_string(&args, "path")?.unwrap_or("."),
                    )?;
                    path = Some(resolved.clone());
                    Ok(PreparedCall::Query {
                        path: resolved,
                        query,
                    })
                }
                ToolKind::Read | ToolKind::Write | ToolKind::Edit => {
                    let operation = file::Operation::parse(name, &args)?;
                    let resolved = file::resolve_path(&self.cwd, string(&args, "path")?)?;
                    path = Some(resolved.clone());
                    Ok(PreparedCall::File {
                        kind,
                        path: resolved,
                        operation,
                    })
                }
            }
        })()
        .map_err(|error| error.output(name, path.as_deref()))?;
        let detail = match &prepared {
            PreparedCall::Time => return Ok(prepared),
            PreparedCall::Bash(command) => format!("命令：{command}"),
            PreparedCall::File { path, .. } | PreparedCall::Query { path, .. } => {
                format!("路径：{}", path.display())
            }
        };
        match self.authorize(name, &detail) {
            Ok(true) => Ok(prepared),
            Ok(false) => Err(ToolError::new(
                "authorization_denied",
                "用户拒绝授权，工具未执行。",
                "停止此操作；不要改用其他工具绕过拒绝。",
            )
            .output(name, path.as_deref())),
            Err(error) => Err(ToolError::new(
                "authorization_failed",
                format!("无法确认授权：{error}"),
                "恢复交互输入后重新请求授权。",
            )
            .output(name, path.as_deref())),
        }
    }

    async fn run_prepared(bash_bin: &Path, cwd: &Path, job: PreparedCall) -> ToolOutput {
        match job {
            PreparedCall::Time => ToolOutput::ok(Local::now().to_rfc3339(), false),
            PreparedCall::Bash(command) => {
                let (text, success) = bash::run_with_status(bash_bin, cwd, &command).await;
                ToolOutput {
                    text,
                    success,
                    changed: false,
                }
            }
            PreparedCall::File {
                kind,
                path,
                operation,
            } => {
                let target = path.clone();
                match tokio::task::spawn_blocking(move || operation.run_checked(&target)).await {
                    Ok(Ok(output)) => output,
                    Ok(Err(error)) => error.output(kind.name(), Some(&path)),
                    Err(error) => ToolError::new(
                        "execution_failed",
                        error.to_string(),
                        "文件任务未正常完成；重新 read 确认状态后再操作。",
                    )
                    .output(kind.name(), Some(&path)),
                }
            }
            PreparedCall::Query { path, query } => query.run(bash_bin, cwd, &path).await,
        }
    }

    #[cfg(test)]
    pub(crate) async fn execute(&mut self, name: &str, args_json: &str) -> String {
        self.execute_recorded(name, args_json).await.text
    }

    pub(crate) async fn execute_recorded(&mut self, name: &str, args_json: &str) -> ToolOutput {
        match self.prepare(name, args_json) {
            Ok(job) => Self::run_prepared(&self.bash_bin, &self.cwd, job).await,
            Err(output) => output,
        }
    }

    fn authorize(&mut self, name: &str, detail: &str) -> io::Result<bool> {
        if self.grants.contains(name) {
            return Ok(true);
        }
        let scope = if name == "bash" {
            "执行任意 Bash 命令"
        } else {
            "对本机任意路径执行该工具的固定文件操作"
        };
        let prompt = format!(
            "\n工具 {name} 请求授权：\n{detail}\n允许后，本次会话内该工具可继续{scope}。[/reset 可撤销] [y/N] "
        );
        if (self.confirm)(&prompt)? {
            self.grants.insert(name.to_owned());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn definitions() -> Vec<Spec> {
        let path = json!({"type":"string","minLength":1,"description":"文件或目录路径；相对启动目录，支持绝对路径和 ~/。不能为空白。"});
        let query_path = json!({"type":"string","minLength":1,"default":".","description":"目标路径；省略为启动目录。ls/glob 要求目录，rg 可为文件或目录；支持绝对路径和 ~/。"});
        let pattern = json!({"type":"string","minLength":1,"description":"ripgrep 路径通配模式，相对搜索根；如 **/*.rs。不是正文正则。"});
        BUILTINS.iter().copied().map(|kind| {
            let (description, properties, required) = match kind {
                ToolKind::Time => ("获取当前系统本地日期与时间。", json!({}), vec![]),
                ToolKind::Bash => ("在启动目录执行 Bash 命令，10 秒/2000 字符。目录用 ls、路径用 glob、正文用 rg；读取或局部修改用 read/edit。首次需授权。", json!({"command":{"type":"string","minLength":1,"description":"完整 Bash 命令；用于构建、测试或专用工具不支持的操作。"}}), vec!["command"]),
                ToolKind::Ls => ("列出目录直接子项，含隐藏项，目录以 / 结尾；不递归。截断时缩小 path 或改用 glob。例：{\"path\":\"src\"}。", json!({"path":query_path}), vec![]),
                ToolKind::Glob => ("按路径通配查找文件，不搜索正文。沿用 rg 忽略规则；显式 glob 可覆盖忽略/隐藏过滤，不跟随目录符号链接。结果相对 path，截断时收窄范围。例：{\"pattern\":\"**/*.rs\",\"path\":\"src\"}。", json!({"path":query_path,"pattern":pattern}), vec!["pattern"]),
                ToolKind::Rg => ("按正文正则搜索，glob 仅过滤路径；files 只列正文命中的文件。沿用 rg 忽略规则，显式 path/glob 可覆盖默认过滤；正则不支持环视/回溯引用。截断时收窄范围，命中后用 read。例：{\"pattern\":\"fn .*test\",\"path\":\"src\",\"output\":\"files\"}。", json!({"path":query_path,"pattern":{"type":"string","minLength":1,"description":"非空正文正则；空白有意义，不是文件名模式。"},"glob":pattern,"output":{"type":"string","enum":["content","files"],"default":"content","description":"content 返回路径、1 起始行号及命中行；files 只列正文命中的文件路径。"},"fixed_strings":{"type":"boolean","default":false,"description":"true 将 pattern 当作字面文本，不解释正则元字符。"}}), vec!["pattern"]),
                ToolKind::Read => ("读取 UTF-8 原文；JSON 元信息与正文以空行分隔，续读用 next_offset（行号）。最多 2000 行/50 KiB；超长单行用 bash 分段。例：{\"path\":\"src/main.rs\",\"offset\":1,\"limit\":80}。", json!({"path":path,"offset":{"type":"integer","minimum":1,"default":1,"description":"起始行号，从 1 开始；续读直接使用上次 next_offset。"},"limit":{"type":"integer","minimum":1,"default":2000,"description":"最多读取的行数；超过 2000 仍按 2000 截取，完整行还受字节预算限制。"}}), vec!["path"]),
                ToolKind::Write => ("创建或整体覆盖文本，自动建父目录；局部修改用 edit。相同内容不写盘，changed 表示是否改变。例：{\"path\":\"notes.txt\",\"content\":\"hello\\n\"}。", json!({"path":path,"content":{"type":"string","description":"完整文件内容，不是补丁；空字符串表示空文件。覆盖前先 read 确认要保留的内容。"}}), vec!["path","content"]),
                ToolKind::Edit => ("局部精确修改。先 read，复制含上下文的原文为 oldText；每项在修改前原文中唯一且互不重叠，任一失败均不写盘。例：{\"path\":\"a.txt\",\"edits\":[{\"oldText\":\"hello\",\"newText\":\"hi\"}]}。", json!({"path":path,"edits":{"type":"array","minItems":1,"description":"同一原文件上的替换列表，不按前一项结果顺序匹配。","items":{"type":"object","properties":{"oldText":{"type":"string","minLength":1,"description":"原文中唯一的非空精确文本；不能包含 read 元信息或搜索行号前缀。"},"newText":{"type":"string","description":"替换文本；空字符串表示删除匹配片段。"}},"required":["oldText","newText"],"additionalProperties":false}}}), vec!["path","edits"]),
            };
            Spec { name:kind.name(), description, parameters:json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false}) }
        }).collect()
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
mod tests;
