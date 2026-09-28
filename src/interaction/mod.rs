//! 界面无关的会话操作与命令语义，供终端界面和 Agent 共用。

mod diagnostic;

use std::{error::Error, io};

pub(crate) use diagnostic::{DiagnosticBuffer, emit_diagnostic};

#[derive(Clone, Debug)]
pub(crate) struct SessionStatus {
    pub(crate) model: String,
    pub(crate) session_id: String,
    pub(crate) context_tokens: u64,
    pub(crate) context_window_tokens: u64,
    pub(crate) turn_tokens: u64,
    pub(crate) total_tokens: u64,
    pub(crate) usage_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Usage {
    pub(crate) input: u64,
    pub(crate) output: u64,
}

pub(crate) trait Session {
    fn session_id(&self) -> &str;

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

    async fn flush(&mut self) -> String;

    async fn compact(&mut self) -> Result<String, Box<dyn Error>>;

    async fn sessions(&self) -> Result<Vec<String>, Box<dyn Error>>;

    async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>>;
}

pub(crate) fn help_text() -> &'static str {
    "可用命令：\n  /help                 显示帮助\n  /compact              压缩旧对话\n  /sessions             列出当前目录的近期会话\n  /open <session-id>    打开会话\n  /resume <session-id>  打开会话（兼容命令）\n  /new                  开始新会话\n  /reset                开始新会话\n  /save                 保存所有待写会话\n  /exit                 退出程序"
}

pub(crate) enum Input {
    Empty,
    Help,
    Reset,
    New,
    Save,
    Compact,
    Sessions,
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
        "/sessions" => Input::Sessions,
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
