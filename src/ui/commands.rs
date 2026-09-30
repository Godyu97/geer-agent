//! 不同 UI 共用的命令错误文案，不参与会话执行。

use crate::interaction::{CommandError, Operation};

pub(super) fn error_text(error: &CommandError) -> String {
    let prefix = match error.operation {
        Operation::Input => return error.message.clone(),
        Operation::Message => "模型请求失败",
        Operation::Compact => "上下文压缩失败",
        Operation::Sessions => "会话列表读取失败",
        Operation::DeletePreview => "删除预览失败",
        Operation::Delete => "会话删除失败",
        Operation::Workspace => "Workspace 切换失败",
        Operation::Open => "会话恢复失败",
    };
    format!("{prefix}：{}", error.message)
}
