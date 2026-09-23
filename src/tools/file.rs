use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde_json::Value;

const MAX_READ_LINES: usize = 2000;
const MAX_READ_BYTES: usize = 50 * 1024;

pub(super) enum Operation {
    Read { offset: usize, limit: usize },
    Write { content: String },
    Edit { edits: Vec<(String, String)> },
}

impl Operation {
    pub(super) fn parse(name: &str, args: &Value) -> Result<Self, String> {
        match name {
            "read" => Ok(Self::Read {
                offset: positive_number(args, "offset")?.unwrap_or(1),
                limit: positive_number(args, "limit")?
                    .unwrap_or(MAX_READ_LINES)
                    .min(MAX_READ_LINES),
            }),
            "write" => Ok(Self::Write {
                content: args
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or("write 需要 content 字符串。")?
                    .to_owned(),
            }),
            "edit" => {
                let items = args
                    .get("edits")
                    .and_then(Value::as_array)
                    .filter(|items| !items.is_empty())
                    .ok_or("edit 需要非空 edits 数组。")?;
                let mut edits = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let old = item
                        .get("oldText")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                        .ok_or_else(|| format!("edits[{index}].oldText 必须是非空字符串。"))?;
                    let new = item
                        .get("newText")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("edits[{index}].newText 必须是字符串。"))?;
                    edits.push((old.to_owned(), new.to_owned()));
                }
                Ok(Self::Edit { edits })
            }
            _ => Err(format!("未知文件工具：{name}")),
        }
    }

    pub(super) fn run(self, path: &Path) -> String {
        let result = match self {
            Self::Read { offset, limit } => read(path, offset, limit),
            Self::Write { content } => write(path, &content),
            Self::Edit { edits } => edit(path, &edits),
        };
        result.unwrap_or_else(|error| format!("文件工具失败：{error}"))
    }
}

fn positive_number(args: &Value, name: &str) -> Result<Option<usize>, String> {
    match args.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| *number > 0)
            .map(Some)
            .ok_or_else(|| format!("{name} 必须是正整数。")),
    }
}

pub(super) fn resolve_path(cwd: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("path 不能为空。".to_owned());
    }
    let path = if raw == "~" || raw.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or("无法展开 ~：HOME 未设置。")?;
        PathBuf::from(home).join(raw.strip_prefix("~/").unwrap_or(""))
    } else {
        PathBuf::from(raw)
    };
    Ok(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

fn read(path: &Path, offset: usize, limit: usize) -> Result<String, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    for _ in 1..offset {
        if read_line_capped(&mut reader, 0)
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err(format!("offset {offset} 超出文件范围。"));
        }
    }
    let mut output = String::new();
    let mut lines = 0;
    let mut has_more = false;
    for _ in 0..limit.min(MAX_READ_LINES) {
        // 预留续读提示的空间，让整个结果仍在单次字节上限内。
        let remaining = (MAX_READ_BYTES - 64) - output.len();
        let Some((bytes, too_long)) =
            read_line_capped(&mut reader, remaining).map_err(|error| error.to_string())?
        else {
            break;
        };
        if too_long {
            if output.is_empty() {
                return Err(format!(
                    "第 {offset} 行超过 50 KiB，建议使用 bash 分段读取。"
                ));
            }
            has_more = true;
            break;
        }
        let line =
            String::from_utf8(bytes).map_err(|_| "文件内容不是有效 UTF-8 文本。".to_owned())?;
        output.push_str(&line);
        lines += 1;
    }
    if lines == 0 && !has_more {
        return if offset == 1 {
            Ok("(空文件)".to_owned())
        } else {
            Err(format!("offset {offset} 超出文件范围。"))
        };
    }
    if !has_more
        && read_line_capped(&mut reader, 0)
            .map_err(|error| error.to_string())?
            .is_some()
    {
        has_more = true;
    }
    if has_more {
        output.push_str(&format!("\n[后续请使用 offset={}]", offset + lines));
    }
    Ok(output)
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

fn write(path: &Path, content: &str) -> Result<String, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, content).map_err(|error| error.to_string())?;
    Ok(format!("已写入 {}。", path.display()))
}

fn edit(path: &Path, edits: &[(String, String)]) -> Result<String, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
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
        let mut matches = normalized.match_indices(&old);
        let Some((start, _)) = matches.next() else {
            return Err(format!("edits[{index}].oldText 未找到。"));
        };
        if matches.next().is_some() {
            return Err(format!("edits[{index}].oldText 匹配多处。"));
        }
        spans.push((start, start + old.len(), new));
    }
    spans.sort_by_key(|span| span.0);
    for pair in spans.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err("编辑区间重叠。".to_owned());
        }
    }
    let mut updated = String::new();
    let mut cursor = 0;
    for (start, end, new) in spans {
        updated.push_str(&normalized[cursor..start]);
        updated.push_str(&new);
        cursor = end;
    }
    updated.push_str(&normalized[cursor..]);
    let final_content = if crlf {
        updated.replace('\n', "\r\n")
    } else {
        updated
    };
    fs::write(path, format!("{bom}{final_content}")).map_err(|error| error.to_string())?;
    Ok(format!("已编辑 {} 处：{}。", edits.len(), path.display()))
}

#[cfg(test)]
mod tests {
    use super::{edit, read, resolve_path, write};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "geer-agent-file-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("创建测试目录");
        path
    }

    #[test]
    fn read_write_and_offsets() {
        let dir = temp_dir();
        let path = resolve_path(&dir, "nested/test.txt").expect("解析相对路径");
        write(&path, "一\n二\n三\n").expect("写入新文件");
        assert!(read(&path, 1, 2).expect("读取前两行").contains("offset=3"));
        assert_eq!(
            read(&path, 2, 1).expect("读取第二行"),
            "二\n\n[后续请使用 offset=3]"
        );
        assert!(read(&path, 99, 1).is_err());
        write(&path, "覆盖").expect("覆盖文件");
        assert_eq!(fs::read_to_string(&path).expect("读取文件"), "覆盖");
        fs::write(&path, [0xff, 0xfe]).expect("写入无效文本");
        assert!(read(&path, 1, 1).is_err());
        fs::remove_dir_all(dir).expect("清理测试目录");
    }

    #[test]
    fn read_caps_lines_and_bytes_without_splitting_utf8() {
        let dir = temp_dir();
        let path = dir.join("large.txt");
        fs::write(&path, "中\n".repeat(2100)).expect("准备多行文件");
        let first = read(&path, 1, 2100).expect("读取首段");
        assert!(first.contains("offset=2001"));
        assert!(first.len() <= 50 * 1024);
        fs::write(&path, "中".repeat(30_000)).expect("准备超长单行");
        assert!(
            read(&path, 1, 1)
                .expect_err("超长行应报错")
                .contains("50 KiB")
        );
        fs::write(&path, "").expect("准备空文件");
        assert_eq!(read(&path, 1, 1).expect("空文件可读"), "(空文件)");
        fs::remove_dir_all(dir).expect("清理测试目录");
    }

    #[test]
    fn edit_all_or_nothing_and_preserve_crlf_bom() {
        let dir = temp_dir();
        let path = dir.join("edit.txt");
        fs::write(&path, "\u{feff}alpha\r\nbeta\r\ngamma\r\n").expect("准备文件");
        edit(
            &path,
            &[
                ("alpha".into(), "one".into()),
                ("gamma".into(), "three".into()),
            ],
        )
        .expect("两处修改");
        let updated = fs::read_to_string(&path).expect("读取结果");
        assert_eq!(updated, "\u{feff}one\r\nbeta\r\nthree\r\n");
        assert!(edit(&path, &[("missing".into(), "x".into())]).is_err());
        assert!(
            edit(
                &path,
                &[("one".into(), "x".into()), ("one\nbeta".into(), "y".into())]
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).expect("确认未改变"), updated);
        fs::write(&path, "same same").expect("准备重复文本");
        assert!(edit(&path, &[("same".into(), "x".into())]).is_err());
        fs::remove_dir_all(dir).expect("清理测试目录");
    }
}
