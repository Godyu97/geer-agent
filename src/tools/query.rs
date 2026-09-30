use std::{
    collections::HashSet,
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::config::bash_arg;

use super::{
    ToolKind, ToolOutput, bash,
    feedback::{ToolError, bounded_lines_result, bounded_result, fields, optional_string, string},
};

const LS_SCRIPT: &str = r#"cd -- "$1" || exit 125
export LC_ALL=C
exec ls -1Ap -- ."#;
const GLOB_SCRIPT: &str = r#"cd -- "$1" || exit 125
exec rg --no-config --files --color never --sort path --path-separator / --glob "$2" -- ."#;
const RG_SCRIPT: &str = r#"args=(--no-config --line-number --with-filename --no-heading --color never --sort path --path-separator /)
if [[ "$3" == yes ]]; then args+=(--glob "$4"); fi
if [[ "$5" == files ]]; then args+=(--files-with-matches); fi
if [[ "$6" == true ]]; then args+=(--fixed-strings); fi
exec rg "${args[@]}" -e "$2" -- "$1""#;
const SEARCH_SCRIPT: &str = r#"args=(--no-config --json --no-follow --max-filesize 1M --max-count 51 --color never --sort path --path-separator /)
if [[ "$3" == yes ]]; then args+=(--glob "$4"); fi
if [[ "$6" == true ]]; then args+=(--fixed-strings); fi
if [[ "$7" == true ]]; then args+=(--ignore-case); fi
exec rg "${args[@]}" -e "$2" -- "$1""#;

pub(super) struct Query {
    kind: ToolKind,
    pattern: String,
    glob: Option<String>,
    output: String,
    fixed_strings: bool,
    ignore_case: bool,
}

impl Query {
    pub fn parse(kind: ToolKind, args: &Value) -> Result<Self, ToolError> {
        let allowed: &[&str] = match kind {
            ToolKind::Ls => &["path"],
            ToolKind::Glob => &["path", "pattern"],
            ToolKind::Search => &[
                "path",
                "pattern",
                "glob",
                "output",
                "fixed_strings",
                "ignore_case",
            ],
            _ => &["path", "pattern", "glob", "output", "fixed_strings"],
        };
        fields(args, allowed, "")?;
        let pattern = if kind == ToolKind::Ls {
            ""
        } else {
            command_string(args, "pattern")?
        };
        let glob = if args.get("glob").is_some() {
            Some(command_string(args, "glob")?.to_owned())
        } else {
            None
        };
        let output = optional_string(args, "output")?.unwrap_or("content");
        if !matches!(output, "content" | "files") {
            return Err(ToolError::invalid(
                "output",
                "只允许 content 或 files。",
                "content 返回正文命中行；files 只列出正文命中的文件路径。",
            ));
        }
        let fixed_strings = match args.get("fixed_strings") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                ToolError::invalid(
                    "fixed_strings",
                    "必须是布尔值。",
                    "字面文本搜索使用 true，正则搜索使用 false；默认 false。",
                )
            })?,
        };
        let ignore_case = match args.get("ignore_case") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                ToolError::invalid(
                    "ignore_case",
                    "必须是布尔值。",
                    "忽略大小写时使用 true；默认 false。",
                )
            })?,
        };
        Ok(Self {
            kind,
            pattern: pattern.to_owned(),
            glob,
            output: output.to_owned(),
            fixed_strings,
            ignore_case,
        })
    }

    fn command(&self, path: &Path) -> (&'static str, Vec<OsString>) {
        let mut args: Vec<OsString> = vec![
            path.to_str()
                .map_or_else(|| path.as_os_str().to_owned(), |raw| bash_arg(raw).into()),
        ];
        let script = match self.kind {
            ToolKind::Ls => LS_SCRIPT,
            ToolKind::Glob => {
                args.push(self.pattern.clone().into());
                GLOB_SCRIPT
            }
            _ => {
                args.extend([
                    self.pattern.clone().into(),
                    if self.glob.is_some() { "yes" } else { "no" }.into(),
                    self.glob.as_deref().unwrap_or("").into(),
                    self.output.clone().into(),
                    self.fixed_strings.to_string().into(),
                ]);
                if self.kind == ToolKind::Search {
                    args.push(self.ignore_case.to_string().into());
                    SEARCH_SCRIPT
                } else {
                    RG_SCRIPT
                }
            }
        };
        (script, args)
    }

    pub async fn run(self, bash_bin: &Path, cwd: &Path, path: &Path) -> ToolOutput {
        if self.kind == ToolKind::Search {
            return self.run_search(bash_bin, cwd, path).await;
        }
        let (script, args) = self.command(path);
        match bash::run_fixed(bash_bin, cwd, script, &args).await {
            Ok(result) => self.format(cwd, path, result),
            Err(error) => ToolError::new(
                if error.kind() == std::io::ErrorKind::TimedOut {
                    "command_timeout"
                } else {
                    "command_unavailable"
                },
                format!("无法执行或捕获 Bash 查询：{error}"),
                "检查配置的 Bash 路径及该环境的 ls/rg；输出超时则缩小查询范围。",
            )
            .output(self.kind.name(), Some(path)),
        }
    }

    async fn run_search(self, bash_bin: &Path, cwd: &Path, path: &Path) -> ToolOutput {
        let started = Instant::now();
        let (root, target) = match search_scope(cwd, path) {
            Ok(scope) => scope,
            Err(error) => return error.output("search", Some(path)),
        };
        let relative = target.strip_prefix(&root).expect("已检查 workspace 边界");
        let relative = if relative.as_os_str().is_empty() {
            Path::new(".")
        } else {
            relative
        };
        let (script, args) = self.command(relative);
        // JSON 包含事件和子匹配元信息，需要比普通查询稍大的有限捕获预算。
        let result = match bash::run_fixed_captured(bash_bin, &root, script, &args, 64 * 1024).await
        {
            Ok(result) => result,
            Err(error) => {
                return ToolError::new(
                    "command_unavailable",
                    error.to_string(),
                    "检查配置的 Bash 路径及该环境的 ripgrep。",
                )
                .output("search", Some(&target));
            }
        };
        let no_matches = result.exit_code == Some(1)
            && result.stderr.bytes.is_empty()
            && !result.stderr.truncated
            && !result.timed_out;
        if result.timed_out || (result.exit_code != Some(0) && !no_matches) {
            return self.format(&root, &target, result);
        }
        let mut lines = Vec::new();
        let mut pending = Vec::new();
        let mut pending_error = None;
        let mut seen = HashSet::new();
        let mut truncated = result.stdout.truncated || result.stderr.truncated;
        for raw in result.stdout.bytes.split_inclusive(|byte| *byte == b'\n') {
            if !raw.ends_with(b"\n") && result.stdout.truncated {
                break;
            }
            let event: Value = match serde_json::from_slice(raw) {
                Ok(event) => event,
                Err(error) => {
                    return ToolError::new(
                        "invalid_search_output",
                        error.to_string(),
                        "ripgrep 未返回有效 JSON 事件；检查版本和执行环境。",
                    )
                    .output("search", Some(&target));
                }
            };
            let data = &event["data"];
            match event["type"].as_str() {
                Some("begin") => {
                    pending.clear();
                    pending_error = None;
                }
                Some("match") => {
                    let Some(raw_path) = data["path"]["text"].as_str() else {
                        return ToolError::new(
                            "invalid_path_encoding",
                            "命中路径不是 UTF-8。",
                            "使用 UTF-8 文件名后重试。",
                        )
                        .output("search", Some(&target));
                    };
                    let hit_path = root.join(raw_path);
                    let resolved_hit = match search_scope(&root, &hit_path) {
                        Ok((_, hit)) => hit,
                        Err(error) => return error.output("search", Some(&hit_path)),
                    };
                    let hit_relative = match resolved_hit.strip_prefix(&root) {
                        Ok(path) => path.to_string_lossy().into_owned(),
                        Err(_) => return outside_workspace().output("search", Some(&hit_path)),
                    };
                    #[cfg(windows)]
                    let hit_relative = hit_relative.replace('\\', "/");
                    if self.output == "files" {
                        if pending.is_empty() {
                            pending.push(hit_relative);
                        }
                    } else {
                        let Some(text) = data["lines"]["text"].as_str() else {
                            pending_error = Some(ToolError::new(
                                "invalid_utf8",
                                "命中行不是 UTF-8 文本。",
                                "search 只返回可读文本；选择 UTF-8 文件后重试。",
                            ));
                            continue;
                        };
                        let Some(number) = data["line_number"].as_u64() else {
                            return ToolError::new(
                                "invalid_search_output",
                                "命中事件缺少行号。",
                                "检查 ripgrep 版本和 JSON 输出。",
                            )
                            .output("search", Some(&target));
                        };
                        let text = text.strip_suffix('\n').unwrap_or(text);
                        let text = text.strip_suffix('\r').unwrap_or(text);
                        pending.push(format!("{hit_relative}:{number}: {text}"));
                    }
                }
                Some("end") => {
                    let mut readable = false;
                    if (!pending.is_empty() || pending_error.is_some())
                        && data["binary_offset"].is_null()
                    {
                        if lines.len() == 50 {
                            truncated = true;
                            pending.clear();
                            pending_error = None;
                            continue;
                        }
                        let Some(raw_path) = data["path"]["text"].as_str() else {
                            return ToolError::new(
                                "invalid_search_output",
                                "文件结束事件缺少路径。",
                                "检查 ripgrep 版本和 JSON 输出。",
                            )
                            .output("search", Some(&target));
                        };
                        let checked = match search_scope(&root, &root.join(raw_path)) {
                            Ok((_, checked)) => checked,
                            Err(error) => return error.output("search", Some(&target)),
                        };
                        // max-count 会提前停止 rg；再有限检查候选文件，避免漏掉后部的 NUL。
                        let scan =
                            tokio::task::spawn_blocking(move || text_file_within_limit(&checked));
                        let remaining = Duration::from_secs(10).saturating_sub(started.elapsed());
                        match tokio::time::timeout(remaining, scan).await {
                            Ok(Ok(Ok(text))) => readable = text,
                            Ok(Ok(Err(error))) => {
                                return ToolError::io(error).output("search", Some(&target));
                            }
                            Ok(Err(error)) => {
                                return ToolError::new(
                                    "execution_failed",
                                    error.to_string(),
                                    "文件类型检查未正常完成；缩小搜索范围后重试。",
                                )
                                .output("search", Some(&target));
                            }
                            Err(_) => {
                                return ToolError::new(
                                    "command_timeout",
                                    "搜索超过 10 秒。",
                                    "缩小 path、pattern 或 glob 后重试。",
                                )
                                .output("search", Some(&target));
                            }
                        }
                    }
                    if readable {
                        if let Some(error) = pending_error.take() {
                            return error.output("search", Some(&target));
                        }
                        for line in pending.drain(..) {
                            if self.output == "files" && !seen.insert(line.clone()) {
                                continue;
                            }
                            if lines.len() == 50 {
                                truncated = true;
                                break;
                            }
                            lines.push(line);
                        }
                    }
                    pending.clear();
                    pending_error = None;
                }
                _ => {}
            }
        }
        let body = lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        let meta = json!({"tool":"search", "status":"ok", "path":target,
            "path_base":root, "output":self.output, "matches":lines.len(),
            "exit_code":result.exit_code, "truncated":truncated});
        ToolOutput::ok(
            bounded_lines_result(meta, &body, bash::MAX_RESULT_CHARS),
            false,
        )
    }

    fn format(&self, cwd: &Path, path: &Path, result: bash::CommandOutput) -> ToolOutput {
        let no_matches = self.kind != ToolKind::Ls
            && result.exit_code == Some(1)
            && result.stderr.bytes.is_empty()
            && !result.stderr.truncated
            && !result.timed_out;
        let success = !result.timed_out && (result.exit_code == Some(0) || no_matches);
        let mut meta = json!({
            "tool":self.kind.name(), "status":if success { "ok" } else { "error" },
            "path":path, "path_base":if matches!(self.kind, ToolKind::Rg | ToolKind::Search) { cwd } else { path },
            "exit_code":result.exit_code, "truncated":result.stdout.truncated || result.stderr.truncated,
        });
        if matches!(self.kind, ToolKind::Rg | ToolKind::Search) {
            meta["output"] = json!(self.output);
        }
        if no_matches {
            meta["matches"] = json!(0);
        }
        if !success {
            let (code, hint) = if result.timed_out {
                (
                    "command_timeout",
                    "命令超过 10 秒，已终止；缩小 path、pattern 或 glob 后重试。",
                )
            } else if matches!(result.exit_code, Some(126 | 127)) {
                (
                    "command_unavailable",
                    "在配置的 Bash 环境中安装 ls/ripgrep 并确保 PATH 可找到该命令。",
                )
            } else if result.exit_code == Some(125) {
                (
                    "invalid_search_root",
                    "无法进入搜索目录；核对 path 是存在且可访问的目录。",
                )
            } else {
                (
                    "command_failed",
                    "查看 stderr，核对 path、权限和模式；正则不支持回溯引用/环视，字面搜索可设 fixed_strings=true。",
                )
            };
            meta["code"] = json!(code);
            meta["hint"] = json!(hint);
        }
        // 错误诊断先于可能很长的部分结果，避免截断吞掉失败原因。
        let mut body = String::new();
        if !result.stderr.bytes.is_empty() {
            meta["has_stderr"] = json!(true);
            body.push_str("stderr:\n");
            body.push_str(&String::from_utf8_lossy(&result.stderr.bytes));
            body.push_str("\nstdout:\n");
        }
        body.push_str(&String::from_utf8_lossy(&result.stdout.bytes));
        ToolOutput {
            text: bounded_result(meta, &body, bash::MAX_RESULT_CHARS),
            success,
            changed: false,
        }
    }
}

pub(super) fn search_scope(cwd: &Path, path: &Path) -> Result<(PathBuf, PathBuf), ToolError> {
    let root = cwd.canonicalize().map_err(ToolError::io)?;
    let target = path.canonicalize().map_err(ToolError::io)?;
    if !target.starts_with(&root) {
        return Err(outside_workspace());
    }
    if root.to_str().is_none() || target.to_str().is_none() {
        return Err(ToolError::new(
            "invalid_path_encoding",
            "规范化路径不能表示为 UTF-8。",
            "使用 UTF-8 的目录和文件名后重试。",
        ));
    }
    Ok((root, target))
}

fn outside_workspace() -> ToolError {
    ToolError::new(
        "outside_workspace",
        "搜索路径经过规范化后位于 workspace 外。",
        "仅在当前 workspace 内搜索；不要改用其他工具绕过边界。",
    )
}

fn text_file_within_limit(path: &Path) -> std::io::Result<bool> {
    const MAX_FILE: u64 = 1024 * 1024;
    let metadata = path.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE {
        return Ok(false);
    }
    let mut file = File::open(path)?.take(MAX_FILE + 1);
    let mut block = [0_u8; 8192];
    let mut total = 0;
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            return Ok(true);
        }
        total += count;
        if total > MAX_FILE as usize || block[..count].contains(&0) {
            return Ok(false);
        }
    }
}

fn command_string<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    let value = string(args, name)?;
    if value.is_empty() || value.contains('\0') {
        return Err(ToolError::invalid(
            name,
            "模式不能为空或包含 NUL。",
            "提供非空模式；pattern 表示正文正则，glob 工具的 pattern 和 rg.glob 表示路径通配。",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_status_and_complete_output_budget() {
        let query = Query::parse(ToolKind::Rg, &json!({"pattern":"x"})).expect("解析");
        for (exit_code, timeout, stderr, expected) in [
            (Some(1), false, "", None),
            (Some(1), false, "Permission denied", Some("command_failed")),
            (Some(2), false, "regex parse error", Some("command_failed")),
            (
                Some(125),
                false,
                "not a directory",
                Some("invalid_search_root"),
            ),
            (Some(127), false, "not found", Some("command_unavailable")),
            (None, true, "", Some("command_timeout")),
        ] {
            let result = query.format(
                Path::new("/tmp"),
                Path::new("/tmp/test"),
                bash::CommandOutput {
                    exit_code,
                    timed_out: timeout,
                    lingering: false,
                    stdout: bash::Capture {
                        bytes: Vec::new(),
                        truncated: false,
                    },
                    stderr: bash::Capture {
                        bytes: stderr.as_bytes().to_vec(),
                        truncated: false,
                    },
                },
            );
            let meta: Value = serde_json::from_str(result.text.split_once("\n\n").expect("分隔").0)
                .expect("元信息");
            assert_eq!(result.success, expected.is_none());
            assert_eq!(meta["code"].as_str(), expected);
            if expected.is_none() {
                assert_eq!(meta["matches"], 0);
            }
            assert!(result.text.contains(stderr));
        }
        let long_path = "/\"\n中".repeat(4000);
        let result = query.format(
            Path::new(&long_path),
            Path::new(&long_path),
            bash::CommandOutput {
                exit_code: Some(2),
                timed_out: false,
                lingering: false,
                stdout: bash::Capture {
                    bytes: "中".repeat(8000).into_bytes(),
                    truncated: true,
                },
                stderr: bash::Capture {
                    bytes: b"regex error".to_vec(),
                    truncated: false,
                },
            },
        );
        assert!(result.text.chars().count() <= 2000);
        assert!(result.text.contains("regex error"));
        assert!(result.text.contains("输出已截断"));
        let meta: Value = serde_json::from_str(result.text.split_once("\n\n").expect("分隔").0)
            .expect("元信息仍为有效 JSON");
        assert_eq!(meta["truncated"], true);
        assert_eq!(meta["metadata_truncated"], true);
    }
}
