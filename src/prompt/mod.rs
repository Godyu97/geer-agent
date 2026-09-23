//! 统一组装发给模型的默认提示和运行环境信息。

use std::{io, path::Path, time::Duration};

use tokio::{process::Command, time::timeout};

mod conversation;

pub(crate) use conversation::Prompt;

const BASH_VERSION_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) async fn load(bash_bin: &Path) -> io::Result<String> {
    let bash_version = bash_version(bash_bin).await?;
    Ok(compose(&system_version().await, &bash_version))
}

async fn bash_version(bash_bin: &Path) -> io::Result<String> {
    let mut command = Command::new(bash_bin);
    command.arg("--version").kill_on_drop(true);
    let output = timeout(BASH_VERSION_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Bash {} 的版本探测超时", bash_bin.display()),
            )
        })?
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("无法运行 Bash {}：{error}", bash_bin.display()),
            )
        })?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Bash {} 的版本探测失败：退出状态 {}",
            bash_bin.display(),
            output.status
        )));
    }
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if !first_line.starts_with("GNU bash, version ") || first_line.len() > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Bash {} 的版本输出无法识别。", bash_bin.display()),
        ));
    }
    Ok(first_line)
}

async fn system_version() -> String {
    match std::env::consts::OS {
        "linux" => {
            let release = std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|content| parse_pretty_name(&content))
                .unwrap_or_else(|| "未知发行版".to_owned());
            let kernel = command_first_line("uname", &["-r"])
                .await
                .unwrap_or_else(|| "未知内核".to_owned());
            format_system_version("linux", &release, &kernel)
        }
        "windows" => {
            let version = command_first_line("cmd", &["/C", "ver"])
                .await
                .unwrap_or_else(|| "未知".to_owned());
            format_system_version("windows", &version, "")
        }
        "macos" => {
            let version = command_first_line("sw_vers", &["-productVersion"])
                .await
                .unwrap_or_else(|| "未知".to_owned());
            format!("macOS {version}")
        }
        other => format!("{other} 版本未知"),
    }
}

fn parse_pretty_name(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim_matches('"').trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

async fn command_first_line(program: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args).kill_on_drop(true);
    let output = timeout(BASH_VERSION_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    output
        .status
        .success()
        .then(|| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_owned()
        })
        .filter(|value| !value.is_empty())
}

fn format_system_version(os: &str, release: &str, kernel: &str) -> String {
    match os {
        "linux" => format!("Linux {release}; 内核 {kernel}"),
        "windows" => release.to_owned(),
        _ => format!("{os} 版本未知"),
    }
}

fn compose(system: &str, bash: &str) -> String {
    format!(
        "你是本机运行的助手。回答和建议的命令应参考以下环境信息。\n<context_data>\nsystem_version: {}\nbash_version: {}\n</context_data>",
        escape_xml(system),
        escape_xml(bash)
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{bash_version, compose, format_system_version, parse_pretty_name};

    #[test]
    fn formats_platform_versions_and_unknowns() {
        assert_eq!(
            format_system_version("linux", "Fedora 44", "7.2"),
            "Linux Fedora 44; 内核 7.2"
        );
        assert_eq!(
            format_system_version("windows", "Microsoft Windows [Version 11]", ""),
            "Microsoft Windows [Version 11]"
        );
        assert_eq!(format_system_version("other", "", ""), "other 版本未知");
        assert_eq!(
            parse_pretty_name("NAME=Fedora\nPRETTY_NAME=\"Fedora Linux 44\"\n"),
            Some("Fedora Linux 44".to_owned())
        );
        let prompt = compose("Linux <test>", "GNU bash, version 5.3");
        assert!(prompt.contains("<context_data>"));
        assert!(prompt.contains("Linux &lt;test&gt;"));
        assert!(prompt.contains("bash_version: GNU bash, version 5.3"));
    }

    #[tokio::test]
    async fn probes_default_bash_and_rejects_invalid_path() {
        assert!(
            bash_version(Path::new("bash"))
                .await
                .expect("Bash 可用")
                .starts_with("GNU bash, version ")
        );
        let error = bash_version(Path::new("/definitely/missing/geer-agent-bash"))
            .await
            .expect_err("路径无效应失败");
        assert!(error.to_string().contains("无法运行 Bash"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probes_custom_path_and_rejects_non_bash_output() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let path =
            std::env::temp_dir().join(format!("geer-agent-version-test-{}", std::process::id()));
        fs::write(&path, "#!/bin/sh\nprintf 'GNU bash, version 9.9\\n'\n")
            .expect("创建版本探测程序");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("设置执行权限");
        assert_eq!(
            bash_version(&path).await.expect("识别版本"),
            "GNU bash, version 9.9"
        );
        let invalid_path = path.with_extension("invalid");
        fs::write(&invalid_path, "#!/bin/sh\nprintf 'other shell\\n'\n")
            .expect("创建非 Bash 版本探测程序");
        fs::set_permissions(&invalid_path, fs::Permissions::from_mode(0o700))
            .expect("设置执行权限");
        let error = bash_version(&invalid_path)
            .await
            .expect_err("无法识别非 Bash 输出");
        fs::remove_file(&path).expect("清理测试程序");
        fs::remove_file(&invalid_path).expect("清理非 Bash 测试程序");
        assert!(error.to_string().contains("无法识别"));
    }
}
