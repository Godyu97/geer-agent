//! 共用会话操作只返回执行结果，具体输入与展示交给 UI。

use std::io;

use super::{Input, MemoryCommand, Session, Usage};
use crate::memory::{MemoryEntry, MemoryPreview};
use crate::session::{DeletePreview, DeleteReport, delete_ids};

#[derive(Debug)]
pub(crate) enum CommandOutcome {
    Empty,
    Help,
    NewSession {
        session_id: String,
        reset: bool,
    },
    Saved(String),
    Compacted(String),
    Sessions(Vec<String>),
    DeletePreview(DeletePreview),
    Deleted(DeleteReport),
    Workspace(String),
    WorkspaceChanged(String),
    Opened {
        message: String,
        session_id: String,
    },
    Exit,
    Unknown(String),
    Message,
    Memories {
        entries: Vec<MemoryEntry>,
        query: Option<String>,
    },
    MemoryChanged(String),
    MemoryPreview(MemoryPreview),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Input,
    Message,
    Compact,
    Sessions,
    DeletePreview,
    Delete,
    Workspace,
    Open,
    Memory,
}

#[derive(Debug)]
pub(crate) struct CommandError {
    pub(crate) operation: Operation,
    pub(crate) message: String,
}

impl CommandError {
    fn new(operation: Operation, error: impl ToString) -> Self {
        Self {
            operation,
            message: error.to_string(),
        }
    }
}

pub(crate) async fn save(session: &mut impl Session) -> String {
    session.flush().await
}

pub(crate) async fn execute<S, F, U>(
    session: &mut S,
    command: Input,
    on_delta: F,
    on_usage: U,
) -> Result<CommandOutcome, CommandError>
where
    S: Session,
    F: FnMut(&str) -> io::Result<()>,
    U: FnMut(Option<Usage>) -> io::Result<()>,
{
    Ok(match command {
        Input::Empty => CommandOutcome::Empty,
        Input::Invalid(message) => return Err(CommandError::new(Operation::Input, message)),
        Input::Help => CommandOutcome::Help,
        Input::Memory(command) => {
            if !command.confirmed()
                && let Some(action) = command.action()
            {
                return session
                    .preview_memory(action)
                    .await
                    .map(CommandOutcome::MemoryPreview)
                    .map_err(|error| CommandError::new(Operation::Memory, error));
            }
            let result = async {
                Ok::<_, Box<dyn std::error::Error>>(match command {
                    MemoryCommand::List => CommandOutcome::Memories {
                        entries: session.memories().await?,
                        query: None,
                    },
                    MemoryCommand::Search(query) => CommandOutcome::Memories {
                        entries: session.search_memory(&query).await?,
                        query: Some(query),
                    },
                    MemoryCommand::Add(content) => {
                        let (entry, changed) = session.add_memory(&content).await?;
                        CommandOutcome::MemoryChanged(format!(
                            "{}：{}",
                            if changed {
                                "已添加记忆"
                            } else {
                                "记忆已存在"
                            },
                            entry.id
                        ))
                    }
                    MemoryCommand::Edit { id, content } => {
                        let (entry, changed) = session.edit_memory(&id, &content).await?;
                        CommandOutcome::MemoryChanged(format!(
                            "{}：{}",
                            if changed {
                                "已更新记忆"
                            } else {
                                "记忆未改变"
                            },
                            entry.id
                        ))
                    }
                    MemoryCommand::Delete { id, .. } => CommandOutcome::MemoryChanged(format!(
                        "{}：{id}",
                        if session.delete_memory(&id).await? {
                            "已删除全局记忆"
                        } else {
                            "记忆已不存在"
                        }
                    )),
                    MemoryCommand::Clear { .. } => CommandOutcome::MemoryChanged(format!(
                        "已清空 {} 条全局长期记忆。会话与 Trace 保留。",
                        session.clear_memories().await?
                    )),
                })
            }
            .await;
            result.map_err(|error| CommandError::new(Operation::Memory, error))?
        }
        command @ (Input::Reset | Input::New) => CommandOutcome::NewSession {
            reset: matches!(command, Input::Reset),
            session_id: session.new_session().await,
        },
        Input::Save => CommandOutcome::Saved(save(session).await),
        Input::Compact => CommandOutcome::Compacted(
            session
                .compact()
                .await
                .map_err(|error| CommandError::new(Operation::Compact, error))?,
        ),
        Input::Sessions(scope) => CommandOutcome::Sessions(
            session
                .sessions(scope)
                .await
                .map_err(|error| CommandError::new(Operation::Sessions, error))?,
        ),
        Input::Delete { ids, confirmed } => {
            // 面板也能直接构造 Input，因此不能只依赖文本解析器校验删除目标。
            let ids =
                delete_ids(&ids).map_err(|error| CommandError::new(Operation::Input, error))?;
            if confirmed {
                CommandOutcome::Deleted(
                    session
                        .delete_sessions(&ids)
                        .await
                        .map_err(|error| CommandError::new(Operation::Delete, error))?,
                )
            } else {
                CommandOutcome::DeletePreview(
                    session
                        .preview_delete(&ids)
                        .await
                        .map_err(|error| CommandError::new(Operation::DeletePreview, error))?,
                )
            }
        }
        Input::Workspace(None) => CommandOutcome::Workspace(session.workspace()),
        Input::Workspace(Some(path)) => CommandOutcome::WorkspaceChanged(
            session
                .set_workspace(&path)
                .await
                .map_err(|error| CommandError::new(Operation::Workspace, error))?,
        ),
        Input::Open(id) => {
            let message = session
                .open(&id)
                .await
                .map_err(|error| CommandError::new(Operation::Open, error))?;
            CommandOutcome::Opened {
                message,
                session_id: session.session_id().to_owned(),
            }
        }
        Input::Exit => CommandOutcome::Exit,
        Input::Unknown(command) => CommandOutcome::Unknown(command),
        Input::Message(message) => {
            session
                .handle_message(&message, on_delta, on_usage)
                .await
                .map_err(|error| CommandError::new(Operation::Message, error))?;
            CommandOutcome::Message
        }
    })
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
