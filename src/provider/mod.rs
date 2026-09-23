//! 模型协议层：可被 `repl` / `agent` 引用。本模块不引用 `tools`。

pub(crate) mod openai;

use std::{error::Error, io};

use serde_json::Value;

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
}

/// 单步对话：工具循环由 `agent` 编排。
///
/// Chat Completions 在 `apply_tool_results` / `finish_turn` 时立刻写入 history；
/// Responses 先放 pending，再在 `finish_turn` / `commit_turn` 提交。
pub(crate) trait ChatProvider {
    fn begin_turn(&mut self, user_input: &str);

    async fn complete_step<F>(
        &mut self,
        tools: &[ToolSpec],
        on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>;

    fn apply_tool_results(&mut self, text: String, results: &[(ToolCall, String)]);
    fn finish_turn(&mut self, text: String);
    fn commit_turn(&mut self);
    fn rollback_turn(&mut self);
    fn reset(&mut self);
}
