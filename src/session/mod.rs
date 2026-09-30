//! 会话状态、workspace 与持久化契约；模型消息的所有权仍在 prompt。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

mod deletion;
mod display;
mod runtime;

pub(crate) use deletion::{
    DELETE_USAGE, DeleteItem, DeletePreview, DeleteReport, DeleteState, DeleteTarget,
    StoreDeletion, delete_ids,
};
pub(crate) use display::{session_title, short_id};
pub(crate) use runtime::SessionEntry;
pub(crate) use runtime::{SessionManager, SessionRuntime};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Workspace(PathBuf);

impl Workspace {
    pub(crate) fn current() -> Result<Self, String> {
        let cwd = env::current_dir().map_err(|error| format!("无法读取启动目录：{error}"))?;
        Self::from_path(cwd)
    }

    pub(crate) fn parse(raw: &str, base: &Workspace) -> Result<Self, String> {
        let input = raw.trim();
        if input.is_empty() {
            return Err("Workspace 路径不能为空。".to_owned());
        }
        let input = strip_paired_quotes(input)?;
        let path = if input == "~" || input.starts_with("~/") || input.starts_with("~\\") {
            let home = env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .ok_or_else(|| "无法展开 ~：未设置 HOME 或 USERPROFILE。".to_owned())?;
            PathBuf::from(home).join(input[1..].trim_start_matches(['/', '\\']))
        } else {
            PathBuf::from(input)
        };
        Self::from_path(if path.is_absolute() {
            path
        } else {
            base.as_path().join(path)
        })
    }

    pub(crate) fn from_stored(raw: &str) -> Result<Self, String> {
        let path = PathBuf::from(raw);
        if !path.is_absolute() {
            return Err("会话记录中的 workspace 不是绝对路径。".to_owned());
        }
        Self::from_path(path)
    }

    fn from_path(path: PathBuf) -> Result<Self, String> {
        let canonical = fs::canonicalize(&path)
            .map_err(|error| format!("Workspace {} 无法访问：{error}", path.display()))?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| format!("Workspace {} 无法读取：{error}", canonical.display()))?;
        if !metadata.is_dir() {
            return Err(format!("Workspace {} 不是目录。", canonical.display()));
        }
        fs::read_dir(&canonical)
            .map_err(|error| format!("Workspace {} 不可访问：{error}", canonical.display()))?;
        Ok(Self(canonical))
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn as_str(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl fmt::Display for Workspace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}

fn strip_paired_quotes(input: &str) -> Result<&str, String> {
    let first = input.chars().next();
    let last = input.chars().next_back();
    match (first, last) {
        (Some(quote @ ('\'' | '"')), Some(end)) if quote == end && input.len() >= 2 => {
            let inner = &input[quote.len_utf8()..input.len() - end.len_utf8()];
            if inner.is_empty() {
                Err("Workspace 路径不能为空。".to_owned())
            } else {
                Ok(inner)
            }
        }
        (Some('\'' | '"'), _) | (_, Some('\'' | '"')) => {
            Err("Workspace 路径的引号不配对。".to_owned())
        }
        _ => Ok(input),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionEvent {
    pub(crate) id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) session_id: String,
    pub(crate) kind: String,
    pub(crate) payload: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SessionRecord {
    pub(crate) id: String,
    pub(crate) workspace: String,
    pub(crate) api: String,
    pub(crate) model: String,
    pub(crate) endpoint: String,
    pub(crate) snapshot: Value,
    pub(crate) head_event_id: Option<String>,
    pub(crate) revision: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) uncertain_tools: bool,
}

#[cfg(test)]
mod tests {
    use super::Workspace;
    use std::fs;
    use uuid::Uuid;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("geer-workspace-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn resolves_relative_quoted_unicode_and_rejects_invalid_paths() {
        let root = temp_dir("root");
        let nested = root.join("中文 space");
        fs::create_dir_all(&nested).unwrap();
        let base = Workspace::from_path(root.clone()).unwrap();
        assert_eq!(
            Workspace::parse("'中文 space'", &base).unwrap().as_path(),
            fs::canonicalize(&nested).unwrap()
        );
        assert_eq!(Workspace::parse(".", &base).unwrap(), base);
        assert!(
            Workspace::parse("'missing", &base)
                .unwrap_err()
                .contains("引号")
        );
        assert!(Workspace::parse("missing", &base).is_err());
        let file = root.join("file.txt");
        fs::write(&file, "x").unwrap();
        assert!(
            Workspace::parse(file.to_str().unwrap(), &base)
                .unwrap_err()
                .contains("不是目录")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn expands_home_when_available() {
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        else {
            return;
        };
        let base = Workspace::current().unwrap();
        assert_eq!(
            Workspace::parse("~", &base).unwrap().as_path(),
            fs::canonicalize(home).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonicalizes_symbolic_links() {
        use std::os::unix::fs::symlink;
        let root = temp_dir("link");
        let target = root.join("target");
        let link = root.join("link");
        fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();
        let base = Workspace::from_path(root.clone()).unwrap();
        assert_eq!(Workspace::parse("link", &base).unwrap().as_path(), target);
        fs::remove_dir_all(root).unwrap();
    }
}
