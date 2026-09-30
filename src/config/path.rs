//! Windows 与 Linux 共用的路径表示转换；只做字符串变换，不访问文件系统。

use std::path::PathBuf;

/// `fs::canonicalize` 在 Windows 上返回 `\\?\C:\...`；Git Bash 会把这类参数解析成
/// `\?C:...`，模型也会照抄进命令，所以对外统一使用普通盘符路径。
pub(crate) fn plain_path(path: PathBuf) -> PathBuf {
    if cfg!(windows)
        && let Some(plain) = path.to_str().and_then(strip_verbatim)
    {
        return PathBuf::from(plain);
    }
    path
}

fn strip_verbatim(raw: &str) -> Option<String> {
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        return Some(format!(r"\\{rest}"));
    }
    let rest = raw.strip_prefix(r"\\?\")?;
    has_drive(rest).then(|| rest.to_owned())
}

fn has_drive(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// 交给 Bash 的路径参数：Windows 下为 `F:/repo/src`，Git Bash 与原生 Windows 程序都能识别。
pub(crate) fn bash_arg(raw: &str) -> String {
    if !cfg!(windows) {
        return raw.to_owned();
    }
    strip_verbatim(raw)
        .unwrap_or_else(|| raw.to_owned())
        .replace('\\', "/")
}

/// Windows 盘符路径在 Git Bash 中的写法，例如 `F:\repo` → `/f/repo`；非盘符路径返回 None。
pub(crate) fn msys_style(raw: &str) -> Option<String> {
    let raw = strip_verbatim(raw).unwrap_or_else(|| raw.to_owned());
    if !has_drive(&raw) {
        return None;
    }
    let drive = raw[..1].to_ascii_lowercase();
    let rest = raw[2..].replace('\\', "/");
    let rest = rest.trim_start_matches('/');
    Some(if rest.is_empty() {
        format!("/{drive}")
    } else {
        format!("/{drive}/{rest}")
    })
}

/// 模型常把 Git Bash 输出里的 `/f/repo/a.txt` 当作路径传给文件工具；Windows 下还原为 `F:\repo\a.txt`。
pub(crate) fn from_msys(raw: &str) -> Option<String> {
    let rest = raw.strip_prefix('/')?;
    let mut chars = rest.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    let tail = chars.as_str();
    if !(tail.is_empty() || tail.starts_with('/')) {
        return None;
    }
    Some(format!(
        "{}:\\{}",
        drive.to_ascii_uppercase(),
        tail.trim_start_matches('/').replace('/', "\\")
    ))
}

#[cfg(test)]
mod tests {
    use super::{from_msys, msys_style, strip_verbatim};

    #[test]
    fn converts_between_windows_and_git_bash_forms() {
        assert_eq!(
            strip_verbatim(r"\\?\F:\arzopa\calendar").as_deref(),
            Some(r"F:\arzopa\calendar")
        );
        assert_eq!(
            strip_verbatim(r"\\?\UNC\server\share\a").as_deref(),
            Some(r"\\server\share\a")
        );
        assert_eq!(strip_verbatim(r"\\?\Volume{x}\a"), None);
        assert_eq!(strip_verbatim("/home/user"), None);

        assert_eq!(
            msys_style(r"\\?\F:\arzopa\calendar").as_deref(),
            Some("/f/arzopa/calendar")
        );
        assert_eq!(msys_style(r"C:\").as_deref(), Some("/c"));
        assert_eq!(msys_style("/home/user"), None);

        assert_eq!(
            from_msys("/f/repo/a.txt").as_deref(),
            Some(r"F:\repo\a.txt")
        );
        assert_eq!(from_msys("/c").as_deref(), Some(r"C:\"));
        assert_eq!(from_msys("/home/user"), None);
        assert_eq!(from_msys("/usr/bin"), None);
    }
}
