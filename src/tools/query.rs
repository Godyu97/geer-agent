use std::{ffi::OsString, path::Path};

use serde_json::{Value, json};

use super::{
    ToolKind, ToolOutput, bash,
    feedback::{ToolError, bounded_result, fields, optional_string, string},
};

const LS_SCRIPT: &str = r#"cd -- "$1" || exit 125
export LC_ALL=C
exec ls -1Ap -- ."#;
const GLOB_SCRIPT: &str = r#"cd -- "$1" || exit 125
exec rg --no-config --files --color never --sort path --glob "$2" -- ."#;
const RG_SCRIPT: &str = r#"args=(--no-config --line-number --with-filename --no-heading --color never --sort path)
if [[ "$3" == yes ]]; then args+=(--glob "$4"); fi
if [[ "$5" == files ]]; then args+=(--files-with-matches); fi
if [[ "$6" == true ]]; then args+=(--fixed-strings); fi
exec rg "${args[@]}" -e "$2" -- "$1""#;

pub(super) struct Query {
    kind: ToolKind,
    pattern: String,
    glob: Option<String>,
    output: String,
    fixed_strings: bool,
}

impl Query {
    pub fn parse(kind: ToolKind, args: &Value) -> Result<Self, ToolError> {
        let allowed: &[&str] = match kind {
            ToolKind::Ls => &["path"],
            ToolKind::Glob => &["path", "pattern"],
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
        Ok(Self {
            kind,
            pattern: pattern.to_owned(),
            glob,
            output: output.to_owned(),
            fixed_strings,
        })
    }

    fn command(&self, path: &Path) -> (&'static str, Vec<OsString>) {
        let mut args = vec![path.as_os_str().to_owned()];
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
                RG_SCRIPT
            }
        };
        (script, args)
    }

    pub async fn run(self, bash_bin: &Path, cwd: &Path, path: &Path) -> ToolOutput {
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

    fn format(&self, cwd: &Path, path: &Path, result: bash::CommandOutput) -> ToolOutput {
        let no_matches = self.kind != ToolKind::Ls
            && result.exit_code == Some(1)
            && result.stderr.bytes.is_empty()
            && !result.stderr.truncated
            && !result.timed_out;
        let success = !result.timed_out && (result.exit_code == Some(0) || no_matches);
        let mut meta = json!({
            "tool":self.kind.name(), "status":if success { "ok" } else { "error" },
            "path":path, "path_base":if self.kind == ToolKind::Rg { cwd } else { path },
            "exit_code":result.exit_code, "truncated":result.stdout.truncated || result.stderr.truncated,
        });
        if self.kind == ToolKind::Rg {
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
