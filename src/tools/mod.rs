//! 工具定义与执行：不引用 `provider` / `repl` / `agent`。

mod bash;
mod feedback;
mod file;
mod query;
mod web_access;

use std::{
    collections::HashSet,
    io,
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
    Search,
    WebSearch,
    WebFetch,
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
    ToolKind::Search,
    ToolKind::WebSearch,
    ToolKind::WebFetch,
];

impl ToolKind {
    fn name(self) -> &'static str {
        match self {
            Self::Time => "get_current_time",
            Self::Bash => "bash",
            Self::Ls => "ls",
            Self::Glob => "glob",
            Self::Rg => "rg",
            Self::Search => "search",
            Self::WebSearch => "web_search",
            Self::WebFetch => "web_fetch",
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
            Self::Ls | Self::Glob | Self::Rg | Self::Search | Self::Read => ExecutionMode::Parallel,
            Self::Time
            | Self::Bash
            | Self::Write
            | Self::Edit
            | Self::WebSearch
            | Self::WebFetch => ExecutionMode::Sequential,
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
    WebSearch(web_access::SearchRequest),
    WebFetch(reqwest::Url),
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
    #[cfg(test)]
    cwd: PathBuf,
    bash_bin: PathBuf,
    grants: HashSet<String>,
    confirm: ConfirmFn,
    web: Option<web_access::WebAccess>,
}

impl Tools {
    pub(crate) fn set_confirm(&mut self, confirm: impl FnMut(&str) -> io::Result<bool> + 'static) {
        self.confirm = Box::new(confirm);
    }

    #[cfg(test)]
    pub(crate) fn allow_all_for_test(&mut self) {
        self.confirm = Box::new(|_| Ok(true));
    }

    #[cfg(test)]
    pub(crate) fn set_web_endpoint_for_test(&mut self, endpoint: &str) {
        self.web = Some(web_access::WebAccess::for_test(endpoint));
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
            #[cfg(test)]
            cwd: std::env::current_dir()?,
            bash_bin,
            grants: HashSet::new(),
            confirm: Box::new(|_| Ok(false)),
            web: None,
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

    #[cfg(test)]
    pub(crate) async fn execute_batch(&mut self, calls: &[(&str, &str)]) -> Vec<ToolExecution> {
        let cwd = self.cwd.clone();
        self.execute_batch_in(&cwd, calls).await
    }

    pub(crate) async fn execute_batch_in(
        &mut self,
        cwd: &Path,
        calls: &[(&str, &str)],
    ) -> Vec<ToolExecution> {
        let mut results = Vec::with_capacity(calls.len());
        let mut index = 0;
        while index < calls.len() {
            if Self::execution_mode(calls[index].0) == ExecutionMode::Sequential {
                let started = Instant::now();
                let output = self
                    .execute_recorded_in(cwd, calls[index].0, calls[index].1)
                    .await;
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
                prepared.push(self.prepare(cwd, calls[index].0, calls[index].1));
                index += 1;
            }
            // 所有授权在派发前逐项完成；join_all 保留输入次序，写入和 Bash 是段边界。
            let pending = prepared.into_iter().map(|job| async {
                let started = Instant::now();
                let output = match job {
                    Ok(job) => Self::run_prepared(&self.bash_bin, cwd, job).await,
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

    fn prepare(
        &mut self,
        cwd: &Path,
        name: &str,
        args_json: &str,
    ) -> Result<PreparedCall, ToolOutput> {
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
                ToolKind::Ls | ToolKind::Glob | ToolKind::Rg | ToolKind::Search => {
                    let query = query::Query::parse(kind, &args)?;
                    let mut resolved =
                        file::resolve_path(cwd, optional_string(&args, "path")?.unwrap_or("."))?;
                    path = Some(resolved.clone());
                    if kind == ToolKind::Search {
                        resolved = query::search_scope(cwd, &resolved)?.1;
                        path = Some(resolved.clone());
                    }
                    Ok(PreparedCall::Query {
                        path: resolved,
                        query,
                    })
                }
                ToolKind::WebSearch => Ok(PreparedCall::WebSearch(
                    web_access::SearchRequest::parse(&args)?,
                )),
                ToolKind::WebFetch => Ok(PreparedCall::WebFetch(web_access::parse_fetch(&args)?)),
                ToolKind::Read | ToolKind::Write | ToolKind::Edit => {
                    let operation = file::Operation::parse(name, &args)?;
                    let resolved = file::resolve_path(cwd, string(&args, "path")?)?;
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
            PreparedCall::Query { path, .. } if name == "search" => {
                format!("workspace：{}\n搜索路径：{}", cwd.display(), path.display())
            }
            PreparedCall::File { path, .. } | PreparedCall::Query { path, .. } => {
                format!("路径：{}", path.display())
            }
            PreparedCall::WebSearch(request) => {
                format!("后端：Exa 免 Key MCP\n发送查询：{}", request.query)
            }
            PreparedCall::WebFetch(url) => format!("访问 URL：{url}"),
        };
        let error = match self.authorize(name, &detail) {
            Ok(true) => return Ok(prepared),
            Ok(false) => ToolError::new(
                "authorization_denied",
                "用户拒绝授权，工具未执行。",
                "停止此操作；不要改用其他工具绕过拒绝。",
            ),
            Err(error) => ToolError::new(
                "authorization_failed",
                format!("无法确认授权：{error}"),
                "恢复交互输入后重新请求授权。",
            ),
        };
        Err(match &prepared {
            PreparedCall::WebFetch(url) => web_access::error_output(name, Some(url), None, error),
            _ => error.output(name, path.as_deref()),
        })
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
            PreparedCall::WebSearch(_) | PreparedCall::WebFetch(_) => ToolError::new(
                "execution_failed",
                "网页工具必须串行执行。",
                "检查工具注册表的调度模式。",
            )
            .output("web_access", None),
        }
    }

    #[cfg(test)]
    pub(crate) async fn execute(&mut self, name: &str, args_json: &str) -> String {
        self.execute_recorded(name, args_json).await.text
    }

    #[cfg(test)]
    pub(crate) async fn execute_recorded(&mut self, name: &str, args_json: &str) -> ToolOutput {
        let cwd = self.cwd.clone();
        self.execute_recorded_in(&cwd, name, args_json).await
    }

    pub(crate) async fn execute_recorded_in(
        &mut self,
        cwd: &Path,
        name: &str,
        args_json: &str,
    ) -> ToolOutput {
        match self.prepare(cwd, name, args_json) {
            Ok(job) => {
                if matches!(job, PreparedCall::WebSearch(_) | PreparedCall::WebFetch(_))
                    && self.web.is_none()
                {
                    match web_access::WebAccess::new() {
                        Ok(web) => self.web = Some(web),
                        Err(error) => return error.output(name, None),
                    }
                }
                match job {
                    PreparedCall::WebSearch(request) => {
                        self.web
                            .as_ref()
                            .expect("已初始化网页客户端")
                            .search(request)
                            .await
                    }
                    PreparedCall::WebFetch(url) => {
                        self.web
                            .as_ref()
                            .expect("已初始化网页客户端")
                            .fetch(url, self.confirm.as_mut())
                            .await
                    }
                    _ => Self::run_prepared(&self.bash_bin, cwd, job).await,
                }
            }
            Err(output) => output,
        }
    }

    fn authorize(&mut self, name: &str, detail: &str) -> io::Result<bool> {
        if name == "web_fetch" {
            return (self.confirm)(&format!(
                "\n工具 web_fetch 请求本次访问授权：\n{detail}\n本次可访问该 HTTP(S) 地址，包括本机或内网；跨来源重定向会再次确认。[y/N] "
            ));
        }
        if self.grants.contains(name) {
            return Ok(true);
        }
        let scope = match name {
            "bash" => "执行任意 Bash 命令",
            "search" => "在当前 workspace 范围内搜索正文",
            "web_search" => "向 Exa 发送查询并搜索网页",
            _ => "对本机任意路径执行该工具的固定文件操作",
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
        let path = json!({"type":"string","minLength":1,"description":"文件或目录路径；相对当前 workspace，支持绝对路径和 ~/。不能为空白。"});
        let query_path = json!({"type":"string","minLength":1,"default":".","description":"目标路径；省略为当前 workspace。ls/glob 要求目录，rg 可为文件或目录；支持绝对路径和 ~/。"});
        let pattern = json!({"type":"string","minLength":1,"description":"ripgrep 路径通配模式，相对搜索根；如 **/*.rs。不是正文正则。"});
        BUILTINS.iter().copied().map(|kind| {
            let (description, properties, required) = match kind {
                ToolKind::Time => ("获取当前系统本地日期与时间。", json!({}), vec![]),
                ToolKind::Bash => ("在当前 workspace 执行 Bash（Linux bash / Windows Git Bash，非 cmd/PowerShell），非交互，10 秒/2000 字符。目录用 ls、路径用 glob、正文用 search；读取或局部修改用 read/edit。首次需授权。", json!({"command":{"type":"string","minLength":1,"description":"兼容 Linux bash 与 Git Bash 的命令：POSIX 语法、GNU 工具、/ 分隔路径；不等待输入、不常驻后台。"}}), vec!["command"]),
                ToolKind::Ls => ("列出目录直接子项，含隐藏项，目录以 / 结尾；不递归。截断时缩小 path 或改用 glob。例：{\"path\":\"src\"}。", json!({"path":query_path}), vec![]),
                ToolKind::Glob => ("按路径通配查找文件，不搜索正文。沿用 rg 忽略规则；显式 glob 可覆盖忽略/隐藏过滤，不跟随目录符号链接。结果相对 path，截断时收窄范围。例：{\"pattern\":\"**/*.rs\",\"path\":\"src\"}。", json!({"path":query_path,"pattern":pattern}), vec!["pattern"]),
                ToolKind::Rg => ("按正文正则搜索，glob 仅过滤路径；files 只列正文命中的文件。沿用 rg 忽略规则，显式 path/glob 可覆盖默认过滤；正则不支持环视/回溯引用。截断时收窄范围，命中后用 read。例：{\"pattern\":\"fn .*test\",\"path\":\"src\",\"output\":\"files\"}。", json!({"path":query_path,"pattern":{"type":"string","minLength":1,"description":"非空正文正则；空白有意义，不是文件名模式。"},"glob":pattern,"output":{"type":"string","enum":["content","files"],"default":"content","description":"content 返回路径、1 起始行号及命中行；files 只列正文命中的文件路径。"},"fixed_strings":{"type":"boolean","default":false,"description":"true 将 pattern 当作字面文本，不解释正则元字符。"}}), vec!["pattern"]),
                ToolKind::Search => ("在当前 workspace 内搜索正文，默认区分大小写的正则；返回相对 workspace 的路径:行号: 原文，可直接用 read。沿用 rg 忽略规则，不跟随目录链接，跳过二进制及 >1 MiB 文件；最多 50 项/2000 字符，首次需授权。", json!({"path":{"type":"string","minLength":1,"default":".","description":"文件或目录，规范化后必须位于当前 workspace 内。"},"pattern":{"type":"string","minLength":1},"glob":pattern,"output":{"type":"string","enum":["content","files"],"default":"content"},"fixed_strings":{"type":"boolean","default":false},"ignore_case":{"type":"boolean","default":false}}), vec!["pattern"]),
                ToolKind::WebSearch => ("通过 Exa 免 Key MCP 搜索网页，返回标题、完整 URL、摘要及实际后端。默认 5 项，最多 20 项；25 秒/1 MiB/12000 字符。首次需确认向 Exa 发送查询，命中后用 web_fetch 阅读来源。", json!({"query":{"type":"string","minLength":1,"maxLength":4096},"num_results":{"type":"integer","minimum":1,"maximum":20,"default":5}}), vec!["query"]),
                ToolKind::WebFetch => ("抓取 HTTP(S) 的 HTML 或文本，返回来源和最终 URL、响应信息及可读正文。支持本机/内网；每次确认完整 URL，跨来源重定向再次确认，最多 5 次。网络 15 秒/1 MiB/12000 字符；不支持 PDF 或 JavaScript 渲染。", json!({"url":{"type":"string","minLength":1,"maxLength":4096,"description":"完整 HTTP(S) URL，不能含账号密码。"}}), vec!["url"]),
                ToolKind::Read => ("读 UTF-8 原文；元信息/正文以空行分隔。empty=true 为空，status=error 未读到正文，按 hint 修正勿推测。续读用 next_offset。最多 2000 行/50 KiB。例：{\"path\":\"src/main.rs\"}。", json!({"path":path,"offset":{"type":"integer","minimum":1,"default":1,"description":"起始行号，从 1 开始；续读直接使用上次 next_offset。"},"limit":{"type":"integer","minimum":1,"default":2000,"description":"最多读取的行数；超过 2000 仍按 2000 截取，完整行还受字节预算限制。"}}), vec!["path"]),
                ToolKind::Write => ("创建或整体覆盖文本，自动建父目录；局部修改用 edit。相同内容不写盘，changed 表示是否改变。例：{\"path\":\"notes.txt\",\"content\":\"hello\\n\"}。", json!({"path":path,"content":{"type":"string","description":"完整文件内容，不是补丁；空字符串表示空文件。覆盖前先 read 确认要保留的内容。"}}), vec!["path","content"]),
                ToolKind::Edit => ("局部精确修改。先 read，复制含上下文的原文为 oldText；每项在修改前原文中唯一且互不重叠，任一失败均不写盘。例：{\"path\":\"a.txt\",\"edits\":[{\"oldText\":\"hello\",\"newText\":\"hi\"}]}。", json!({"path":path,"edits":{"type":"array","minItems":1,"description":"同一原文件上的替换列表，不按前一项结果顺序匹配。","items":{"type":"object","properties":{"oldText":{"type":"string","minLength":1,"description":"原文中唯一的非空精确文本；不能包含 read 元信息或搜索行号前缀。"},"newText":{"type":"string","description":"替换文本；空字符串表示删除匹配片段。"}},"required":["oldText","newText"],"additionalProperties":false}}}), vec!["path","edits"]),
            };
            Spec { name:kind.name(), description, parameters:json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false}) }
        }).collect()
    }
}

#[cfg(test)]
mod tests;
