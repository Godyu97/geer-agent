use std::ffi::OsStr;

pub(crate) fn background_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    let command = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        let mut command = command;
        // 标准流接管道并不阻止 Windows 给控制台子程序新建窗口。
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    command
}
