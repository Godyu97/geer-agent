use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

use super::{
    ToolOutput,
    feedback::{ToolError, fields, string},
};

const MAX_READ_LINES: usize = 2000;
const MAX_READ_BYTES: usize = 50 * 1024;

pub(super) enum Operation {
    Read { offset: usize, limit: usize },
    Write { content: String },
    Edit { edits: Vec<(String, String)> },
}

impl Operation {
    pub(super) fn parse(name: &str, args: &Value) -> Result<Self, ToolError> {
        match name {
            "read" => {
                fields(args, &["path", "offset", "limit"], "")?;
                Ok(Self::Read {
                    offset: positive_number(args, "offset")?.unwrap_or(1),
                    limit: positive_number(args, "limit")?
                        .unwrap_or(MAX_READ_LINES)
                        .min(MAX_READ_LINES),
                })
            }
            "write" => {
                fields(args, &["path", "content"], "")?;
                Ok(Self::Write {
                    content: string(args, "content")?.to_owned(),
                })
            }
            "edit" => {
                fields(args, &["path", "edits"], "")?;
                let items = args
                    .get("edits")
                    .and_then(Value::as_array)
                    .filter(|items| !items.is_empty())
                    .ok_or_else(|| {
                        ToolError::invalid(
                            "edits",
                            "需要非空 edits 数组。",
                            "提供至少一个包含 oldText 和 newText 的编辑对象。",
                        )
                    })?;
                let mut edits = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let prefix = format!("edits[{index}].");
                    fields(item, &["oldText", "newText"], &prefix)?;
                    let old =
                        string(item, "oldText").map_err(|e| e.at(format!("{prefix}oldText")))?;
                    if old.is_empty() {
                        return Err(ToolError::invalid(
                            format!("{prefix}oldText"),
                            "oldText 不能为空。",
                            "先 read，再复制包含足够上下文的原文作为 oldText。",
                        ));
                    }
                    let new =
                        string(item, "newText").map_err(|e| e.at(format!("{prefix}newText")))?;
                    edits.push((old.to_owned(), new.to_owned()));
                }
                Ok(Self::Edit { edits })
            }
            _ => Err(ToolError::new(
                "unknown_tool",
                "未知文件工具。",
                "使用已声明的工具名。",
            )),
        }
    }

    pub(super) fn run_checked(self, path: &Path) -> Result<ToolOutput, ToolError> {
        match self {
            Self::Read { offset, limit } => read(path, offset, limit),
            Self::Write { content } => write(path, &content),
            Self::Edit { edits } => edit(path, &edits),
        }
    }
}

fn positive_number(args: &Value, name: &str) -> Result<Option<usize>, ToolError> {
    match args.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .map(Some)
            .ok_or_else(|| {
                ToolError::invalid(
                    name,
                    "必须是正整数，单位为行。",
                    "offset 从 1 开始；limit 使用正整数，省略时为 2000。",
                )
            }),
    }
}

pub(super) fn resolve_path(cwd: &Path, raw: &str) -> Result<PathBuf, ToolError> {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'))
        .or_else(|| {
            raw.strip_prefix('\'')
                .and_then(|path| path.strip_suffix('\''))
        })
        .unwrap_or(raw);
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err(ToolError::invalid(
            "path",
            "path 不能为空白或包含 NUL。",
            "提供相对当前 workspace 的路径、本机绝对路径或 ~/ 路径。",
        ));
    }
    let windows_raw = cfg!(windows)
        .then(|| crate::config::from_msys(raw))
        .flatten();
    let raw = windows_raw.as_deref().unwrap_or(raw);
    let path = if raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\") {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| {
                ToolError::invalid(
                    "path",
                    "无法展开 ~：HOME/USERPROFILE 未设置。",
                    "改用绝对路径。",
                )
            })?;
        PathBuf::from(home).join(raw[1..].trim_start_matches(['/', '\\']))
    } else {
        PathBuf::from(raw)
    };
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    Ok(native_separators(crate::config::plain_path(path)))
}

/// Windows 接受 `/` 与 `\` 混用，但工具结果里混用会误导模型；统一成 `\`。
fn native_separators(path: PathBuf) -> PathBuf {
    if cfg!(windows)
        && let Some(raw) = path.to_str()
        && raw.contains('/')
    {
        return PathBuf::from(raw.replace('/', "\\"));
    }
    path
}

fn read_metadata(path: &Path, offset: usize, lines: usize, has_more: bool) -> Value {
    json!({"tool":"read", "status":"ok", "path":path, "lines":lines,
        "empty":lines == 0,
        "message":if lines == 0 { "文件为空，没有文本正文。" } else { "成功读取，正文如下。" },
        "start_line": (lines > 0).then_some(offset),
        "end_line": (lines > 0).then(|| offset.saturating_add(lines - 1)),
        "truncated":has_more, "next_offset":has_more.then(|| offset.saturating_add(lines))})
}

fn read(path: &Path, offset: usize, limit: usize) -> Result<ToolOutput, ToolError> {
    if !fs::metadata(path).map_err(ToolError::io)?.is_file() {
        return Err(ToolError::new(
            "not_a_file",
            "目标不是普通文件，未读取正文。",
            "目录请先用 ls/glob 定位文件；其他类型请用 bash 检查后再读取文本文件。",
        )
        .at("path"));
    }
    let file = File::open(path).map_err(ToolError::io)?;
    let mut reader = BufReader::new(file);
    let out_of_range = || {
        ToolError::new(
            "offset_out_of_range",
            format!("offset {offset} 超出文件范围。"),
            "使用上次 read 返回的 next_offset；不确定范围时从 offset=1 重新读取。",
        )
        .at("offset")
    };
    for _ in 1..offset {
        if read_line_capped(&mut reader, 0)
            .map_err(ToolError::io)?
            .is_none()
        {
            return Err(out_of_range());
        }
    }
    // 按实际路径和最长数字计算元信息空间，完整结果（含 JSON 与分隔符）不超过 50 KiB。
    let mut largest_header = read_metadata(path, offset, MAX_READ_LINES, true);
    largest_header["end_line"] = json!(usize::MAX);
    largest_header["next_offset"] = json!(usize::MAX);
    largest_header["truncated"] = json!(false);
    let budget = MAX_READ_BYTES.saturating_sub(largest_header.to_string().len() + 2);
    let mut body = String::new();
    let mut lines = 0;
    let mut has_more = false;
    for _ in 0..limit.min(MAX_READ_LINES) {
        let Some((bytes, too_long)) =
            read_line_capped(&mut reader, budget.saturating_sub(body.len()))
                .map_err(ToolError::io)?
        else {
            break;
        };
        if too_long {
            if lines == 0 {
                return Err(ToolError::new(
                    "line_too_long",
                    format!("第 {offset} 行无法放入完整结果的 50 KiB 限制。"),
                    "使用 bash 对该行进行针对性的分段读取；增加 limit 无法解决单行超限。",
                ));
            }
            has_more = true;
            break;
        }
        let line = String::from_utf8(bytes).map_err(|_| {
            ToolError::new(
                "invalid_utf8",
                "请求片段不是有效 UTF-8 文本。",
                "核对文件编码；二进制或其他编码的文件请用 bash 检查或转换。",
            )
        })?;
        if line.contains('\0') {
            return Err(ToolError::new(
                "non_text",
                "请求片段包含 NUL，无法作为文本正文读取。",
                "使用 bash 检查文件类型或转换为 UTF-8 文本，再重新读取；不要推测正文。",
            ));
        }
        body.push_str(&line);
        lines += 1;
    }
    if lines == 0 && offset != 1 {
        return Err(out_of_range());
    }
    if !has_more
        && read_line_capped(&mut reader, 0)
            .map_err(ToolError::io)?
            .is_some()
    {
        has_more = true;
    }
    Ok(ToolOutput::ok(
        format!("{}\n\n{body}", read_metadata(path, offset, lines, has_more)),
        false,
    ))
}

fn read_line_capped(reader: &mut impl BufRead, max: usize) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut line = Vec::new();
    let mut seen = false;
    let mut over = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(seen.then_some((line, over)));
        }
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        let done = available[..end].last() == Some(&b'\n');
        let keep = max.saturating_sub(line.len()).min(end);
        line.extend_from_slice(&available[..keep]);
        over |= keep < end;
        reader.consume(end);
        seen = true;
        if done {
            return Ok(Some((line, over)));
        }
    }
}

fn modification(tool: &str, path: &Path, changed: bool) -> ToolOutput {
    ToolOutput::ok(
        json!({"tool":tool, "status":"ok", "path":path, "changed":changed,
        "message":if changed { "文件已改变。" } else { "内容相同，文件未改变。" }})
        .to_string(),
        changed,
    )
}

fn write(path: &Path, content: &str) -> Result<ToolOutput, ToolError> {
    match fs::read(path) {
        Ok(existing) if existing == content.as_bytes() => {
            return Ok(modification("write", path, false));
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(ToolError::io(error)),
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ToolError::io)?;
    }
    fs::write(path, content).map_err(ToolError::io)?;
    Ok(modification("write", path, true))
}

fn edit(path: &Path, edits: &[(String, String)]) -> Result<ToolOutput, ToolError> {
    let raw = fs::read_to_string(path).map_err(ToolError::io)?;
    let (bom, body) = match raw.strip_prefix('\u{feff}') {
        Some(body) => ("\u{feff}", body),
        None => ("", raw.as_str()),
    };
    let crlf = body.contains("\r\n");
    let normalized = body.replace("\r\n", "\n");
    let mut spans = Vec::with_capacity(edits.len());
    for (index, (old, new)) in edits.iter().enumerate() {
        let old = old.replace("\r\n", "\n");
        let new = new.replace("\r\n", "\n");
        let Some(start) = normalized.find(&old) else {
            return Err(ToolError::new(
                "old_text_not_found",
                "oldText 未找到，文件未修改。",
                "重新 read 目标区域，复制原文中的空白、缩进和上下文后再 edit。",
            )
            .at(format!("edits[{index}].oldText")));
        };
        // find/rfind 也能识别 aaa 中两个互相重叠的 aa，match_indices 只枚举不重叠匹配。
        if normalized.rfind(&old) != Some(start) {
            return Err(ToolError::new(
                "ambiguous_old_text",
                "oldText 至少匹配两处，文件未修改。",
                "read 目标区域，为 oldText 增加前后文，直到在原文件中唯一。",
            )
            .at(format!("edits[{index}].oldText")));
        }
        spans.push((start, start + old.len(), new, index));
    }
    spans.sort_by_key(|span| span.0);
    for pair in spans.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(ToolError::new(
                "overlapping_edits",
                format!(
                    "edits[{}] 与 edits[{}] 区间重叠，文件未修改。",
                    pair[0].3, pair[1].3
                ),
                "将冲突项合成一项包含完整上下文的编辑；所有 oldText 都必须来自同一份原文。",
            )
            .at(format!("edits[{}]", pair[1].3)));
        }
    }
    let mut updated = String::new();
    let mut cursor = 0;
    for (start, end, new, _) in spans {
        updated.push_str(&normalized[cursor..start]);
        updated.push_str(&new);
        cursor = end;
    }
    updated.push_str(&normalized[cursor..]);
    if updated == normalized {
        return Ok(modification("edit", path, false));
    }
    let final_content = if crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };
    fs::write(path, format!("{bom}{final_content}")).map_err(ToolError::io)?;
    Ok(modification("edit", path, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("geer-file-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("创建测试目录");
        path
    }

    fn parts(output: &ToolOutput) -> (Value, &str) {
        let (header, body) = output.text.split_once("\n\n").expect("元信息与原文分开");
        (serde_json::from_str(header).expect("合法元信息"), body)
    }

    #[test]
    fn paginated_read_round_trips_original_bytes_and_reports_ranges() {
        let dir = temp_dir();
        let path = resolve_path(&dir, "nested/test.txt").expect("解析路径");
        for original in [
            "一\n二\n三\n",
            "\u{feff}一\r\n二\r\n末尾",
            "\n\n",
            "无尾换行",
        ] {
            write(&path, original).expect("写入文件");
            let mut offset = 1;
            let mut all = String::new();
            loop {
                let result = read(&path, offset, 1).expect("读取片段");
                let (meta, body) = parts(&result);
                assert_eq!(meta["path"], json!(path));
                assert_eq!(meta["start_line"], offset);
                assert_eq!(meta["end_line"], offset);
                assert!(!result.changed);
                all.push_str(body);
                match meta["next_offset"].as_u64() {
                    Some(next) => {
                        assert_eq!(next as usize, offset + 1);
                        offset = next as usize;
                    }
                    None => break,
                }
            }
            assert_eq!(all, original);
        }
        assert_eq!(
            read(&path, 99, 1).expect_err("越界").code,
            "offset_out_of_range"
        );
        fs::write(&path, [0xff, 0xfe]).expect("写入无效文本");
        assert_eq!(
            read(&path, 1, 1).expect_err("非法编码").code,
            "invalid_utf8"
        );
        fs::remove_dir_all(dir).expect("清理");
    }

    #[test]
    fn read_caps_complete_result_and_preserves_complete_lines() {
        let dir = temp_dir();
        let path = dir.join("large.txt");
        fs::write(&path, "中\n".repeat(2100)).expect("多行文本");
        let first = read(&path, 1, 2100).expect("按 2000 行截取");
        assert_eq!(parts(&first).0["next_offset"], 2001);
        assert_eq!(parts(&first).1.lines().count(), 2000);
        let original = format!("{}尾行", format!("{}\r\n", "中".repeat(7000)).repeat(4));
        fs::write(&path, &original).expect("大文本");
        let mut offset = 1;
        let mut all = String::new();
        loop {
            let result = read(&path, offset, 2000).expect("按字节截取");
            assert!(result.text.len() <= MAX_READ_BYTES);
            let (meta, body) = parts(&result);
            all.push_str(body);
            match meta["next_offset"].as_u64() {
                Some(next) => {
                    assert!(next as usize > offset);
                    offset = next as usize;
                }
                None => break,
            }
        }
        assert_eq!(all, original);
        fs::write(&path, "中".repeat(30_000)).expect("超长单行");
        assert_eq!(read(&path, 1, 1).expect_err("超长行").code, "line_too_long");
        fs::write(&path, "").expect("空文件");
        let empty = read(&path, 1, 1).expect("空文件可读");
        let (meta, body) = parts(&empty);
        assert_eq!(meta["lines"], 0);
        assert!(meta["start_line"].is_null());
        assert!(meta["next_offset"].is_null());
        assert_eq!(meta["truncated"], false);
        assert_eq!(meta["empty"], true);
        assert!(meta["message"].as_str().unwrap().contains("文件为空"));
        assert!(body.is_empty());
        assert!(read(&path, 2, 1).is_err());
        fs::remove_dir_all(dir).expect("清理");
    }

    #[test]
    fn paths_resolve_relative_absolute_and_quoted_without_altering_inner_characters() {
        let dir = temp_dir();
        let target = dir.join("nested folder").join("正文.txt");
        for raw in [
            "nested folder/正文.txt",
            "\"nested folder/正文.txt\"",
            " 'nested folder/正文.txt' ",
        ] {
            assert_eq!(
                resolve_path(&dir, raw).unwrap(),
                native_separators(target.clone())
            );
        }
        let absolute = target.to_string_lossy();
        assert_eq!(resolve_path(&dir.join("other"), &absolute).unwrap(), target);
        assert_eq!(
            resolve_path(&dir, &format!("\"{absolute}\"")).unwrap(),
            target
        );
        for raw in ["", " \t ", "\"\"", "''", "a\0b"] {
            assert_eq!(
                resolve_path(&dir, raw).unwrap_err().code,
                "invalid_argument"
            );
        }
        #[cfg(unix)]
        assert_eq!(
            resolve_path(&dir, "./a'\"b.txt").unwrap(),
            dir.join("./a'\"b.txt")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_rejects_directory_and_binary_instead_of_returning_empty_content() {
        let dir = temp_dir();
        assert_eq!(read(&dir, 1, 1).unwrap_err().code, "not_a_file");
        let path = dir.join("binary.bin");
        fs::write(&path, b"header\0data").unwrap();
        assert_eq!(read(&path, 1, 1).unwrap_err().code, "non_text");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn edits_validate_against_original_and_preserve_bom_crlf() {
        let dir = temp_dir();
        let path = dir.join("edit.txt");
        fs::write(&path, "\u{feff}alpha\r\nbeta\r\ngamma\r\n").expect("准备");
        assert!(
            edit(
                &path,
                &[
                    ("alpha".into(), "beta".into()),
                    ("beta".into(), "second".into())
                ]
            )
            .expect("以原文为准")
            .changed
        );
        let updated = "\u{feff}beta\r\nsecond\r\ngamma\r\n";
        assert_eq!(fs::read_to_string(&path).expect("读取"), updated);
        for (edits, code) in [
            (
                vec![("beta".into(), "x".into()), ("missing".into(), "y".into())],
                "old_text_not_found",
            ),
            (
                vec![
                    ("beta".into(), "x".into()),
                    ("beta\nsecond".into(), "y".into()),
                ],
                "overlapping_edits",
            ),
            (
                vec![("beta".into(), "new".into()), ("new".into(), "y".into())],
                "old_text_not_found",
            ),
        ] {
            let error = edit(&path, &edits).expect_err("整体拒绝");
            assert_eq!(error.code, code);
            assert!(error.field.is_some());
            assert!(error.message.contains("未修改"));
            assert_eq!(fs::read_to_string(&path).expect("读取"), updated);
        }
        for original in ["aaa", "aa aa", "中中中"] {
            fs::write(&path, original).expect("准备歧义");
            let old = if original.contains('中') {
                "中中"
            } else {
                "aa"
            };
            assert_eq!(
                edit(&path, &[(old.into(), "x".into())])
                    .expect_err("重复包含重叠起点")
                    .code,
                "ambiguous_old_text"
            );
            assert_eq!(fs::read_to_string(&path).expect("读取"), original);
        }
        fs::remove_dir_all(dir).expect("清理");
    }

    #[test]
    fn noops_do_not_write_and_new_empty_files_are_changes() {
        let dir = temp_dir();
        let path = dir.join("new.txt");
        assert!(write(&path, "").expect("创建空文件").changed);
        assert!(!write(&path, "").expect("空文件不变").changed);
        write(&path, "\u{feff}alpha\r\nbeta\r\n").expect("准备");
        let before = fs::metadata(&path)
            .expect("属性")
            .modified()
            .expect("修改时间");
        assert!(
            !write(&path, "\u{feff}alpha\r\nbeta\r\n")
                .expect("相同内容")
                .changed
        );
        assert!(
            !edit(&path, &[("alpha\nbeta".into(), "alpha\r\nbeta".into())])
                .expect("相同编辑")
                .changed
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("属性")
                .modified()
                .expect("修改时间"),
            before
        );
        assert!(write(&dir, "不能覆盖目录").is_err());
        fs::remove_dir_all(dir).expect("清理");
    }
}
