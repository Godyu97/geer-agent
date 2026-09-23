//! 模型协议层：可被 `repl` / `agent` 引用。本模块不引用 `tools`。

pub(crate) mod openai;

use std::{error::Error, io};

use async_openai::types::responses::OutputItem;
use async_openai::types::{chat::ChatCompletionRequestMessage, responses::InputItem};
use serde_json::Value;

pub(crate) enum Messages {
    Chat(Vec<ChatCompletionRequestMessage>),
    Responses {
        instructions: String,
        input: Vec<InputItem>,
    },
}

/// 给模型看的工具说明书；与 Chat Completions / Responses 的请求类型无关。
#[derive(Debug, Clone)]
pub(crate) struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug)]
pub(crate) struct ModelStep {
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub output: Vec<OutputItem>,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

/// 单步对话：工具循环由 `agent` 编排。
///
pub(crate) trait ChatProvider {
    async fn complete_step<F>(
        &mut self,
        messages: Messages,
        tools: &[ToolSpec],
        on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>;
}
