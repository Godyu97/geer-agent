//! 界面无关的会话契约、命令语义与执行入口，供所有 UI 复用。

mod command;
mod diagnostic;

#[cfg(test)]
mod tests;

use crate::session::{DELETE_USAGE, DeletePreview, DeleteReport, SessionEntry, delete_ids};
use serde::Serialize;
use std::{error::Error, io};

pub(crate) use command::{CommandError, CommandOutcome, Operation, execute, save};
pub(crate) use diagnostic::{DiagnosticBuffer, emit_diagnostic};

#[cfg(test)]
pub(crate) mod test_support;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SessionStatus {
    pub(crate) model: String,
    pub(crate) session_id: String,
    pub(crate) session_title: String,
    pub(crate) workspace: String,
    pub(crate) context_tokens: u64,
    pub(crate) context_window_tokens: u64,
    pub(crate) turn_tokens: u64,
    pub(crate) total_tokens: u64,
    pub(crate) usage_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Usage {
    pub(crate) input: u64,
    pub(crate) output: u64,
}

pub(crate) trait Session {
    fn session_id(&self) -> &str;

    fn workspace(&self) -> String;

    fn status(&self) -> SessionStatus;

    async fn handle_message<F, U>(
        &mut self,
        input: &str,
        on_delta: F,
        on_usage: U,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
        U: FnMut(Option<Usage>) -> io::Result<()>;

    async fn new_session(&mut self) -> String;

    async fn set_workspace(&mut self, path: &str) -> Result<String, Box<dyn Error>>;

    async fn flush(&mut self) -> String;

    async fn compact(&mut self) -> Result<String, Box<dyn Error>>;

    async fn sessions(&self, scope: SessionScope) -> Result<Vec<String>, Box<dyn Error>>;

    async fn session_entries(
        &self,
        scope: SessionScope,
    ) -> Result<Vec<SessionEntry>, Box<dyn Error>>;

    async fn preview_delete(&self, ids: &[String]) -> Result<DeletePreview, Box<dyn Error>>;

    async fn delete_sessions(&mut self, ids: &[String]) -> Result<DeleteReport, Box<dyn Error>>;

    async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>>;
}

pub(crate) fn help_text() -> &'static str {
    "可用命令：\n  /help                 显示帮助\n  /workspace            显示当前 workspace\n  /workspace <path>     切换 workspace 并新建会话\n  /compact              压缩旧对话\n  /sessions             列出当前 workspace 的近期会话\n  /sessions --all       列出全部 workspace 的近期会话\n  /delete [--yes] <id> [id...]  删除会话（保留 Trace）\n  /open <session-id>    打开会话及其 workspace\n  /resume <session-id>  打开会话（兼容命令）\n  /new                  在当前 workspace 开始新会话\n  /reset                在当前 workspace 开始新会话\n  /save                 保存所有待写会话\n  /exit                 退出程序"
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionScope {
    Current,
    All,
}

pub(crate) enum Input {
    Empty,
    Help,
    Reset,
    New,
    Save,
    Compact,
    Sessions(SessionScope),
    Delete { ids: Vec<String>, confirmed: bool },
    Invalid(String),
    Workspace(Option<String>),
    Open(String),
    Exit,
    Unknown(String),
    Message(String),
}

pub(crate) fn parse_input(line: &str) -> Input {
    let input = line.trim();
    match input {
        "" => Input::Empty,
        "/help" => Input::Help,
        "/reset" => Input::Reset,
        "/new" => Input::New,
        "/save" => Input::Save,
        "/compact" => Input::Compact,
        "/sessions" => Input::Sessions(SessionScope::Current),
        "/sessions --all" => Input::Sessions(SessionScope::All),
        input if input.split_whitespace().next() == Some("/delete") => {
            let mut confirmed = false;
            let mut ids = Vec::new();
            for token in input.split_whitespace().skip(1) {
                if token == "--yes" && !confirmed {
                    confirmed = true;
                } else if token.starts_with('-') {
                    return Input::Invalid(DELETE_USAGE.to_owned());
                } else {
                    ids.push(token.to_owned());
                }
            }
            match delete_ids(&ids) {
                Ok(ids) => Input::Delete { ids, confirmed },
                Err(error) => Input::Invalid(error),
            }
        }
        "/workspace" => Input::Workspace(None),
        input if input.starts_with("/workspace ") && !input[11..].trim().is_empty() => {
            Input::Workspace(Some(input[11..].trim().to_owned()))
        }
        input if input.starts_with("/resume ") && !input[8..].trim().is_empty() => {
            Input::Open(input[8..].trim().to_owned())
        }
        input if input.starts_with("/open ") && !input[6..].trim().is_empty() => {
            Input::Open(input[6..].trim().to_owned())
        }
        "/exit" => Input::Exit,
        input if input.starts_with('/') => Input::Unknown(input.to_owned()),
        input => Input::Message(input.to_owned()),
    }
}
