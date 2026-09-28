//! 模型协议层：可被 `repl` / `agent` 引用。本模块不引用 `tools`。

pub(crate) mod openai;

use std::{error::Error, io};

use crate::trace::TraceCapture;
use async_openai::types::responses::{OutputItem, OutputMessageContent, OutputStatus};
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

pub(crate) struct SummaryStep {
    pub(crate) text: String,
    pub(crate) usage: Option<TokenUsage>,
}

impl TryFrom<ModelStep> for SummaryStep {
    type Error = io::Error;

    fn try_from(step: ModelStep) -> Result<Self, Self::Error> {
        if !step.calls.is_empty()
            || step
                .output
                .iter()
                .any(|item| matches!(item, OutputItem::FunctionCall(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "摘要响应请求了工具。",
            ));
        }
        if step.output.iter().any(|item| matches!(item, OutputItem::Message(message) if message.status != OutputStatus::Completed)) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "摘要响应未完成。"));
        }
        let text = if step.output.is_empty() {
            step.text
        } else {
            step.output
                .iter()
                .filter_map(|item| match item {
                    OutputItem::Message(message) => Some(
                        message
                            .content
                            .iter()
                            .filter_map(|content| match content {
                                OutputMessageContent::OutputText(text) => Some(text.text.as_str()),
                                OutputMessageContent::Refusal(_) => None,
                            })
                            .collect::<Vec<_>>()
                            .join(""),
                    ),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let text = text.trim().to_owned();
        if text.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "摘要响应为空。"));
        }
        Ok(Self {
            text,
            usage: step.usage,
        })
    }
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
        capture: &mut TraceCapture,
        on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>;

    async fn summarize(
        &mut self,
        messages: Messages,
        max_output_tokens: u32,
        capture: &mut TraceCapture,
    ) -> Result<SummaryStep, Box<dyn Error>> {
        let _ = max_output_tokens;
        self.complete_step(messages, &[], capture, |_| Ok(()))
            .await?
            .try_into()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use async_openai::types::responses::{
        AssistantRole, OutputMessage, OutputMessageContent, OutputStatus, OutputTextContent,
    };

    use super::{ModelStep, SummaryStep, ToolCall};
    use async_openai::types::responses::OutputItem;

    #[test]
    fn summary_reads_responses_text_and_rejects_invalid_output() {
        let output = OutputItem::Message(OutputMessage {
            content: vec![OutputMessageContent::OutputText(OutputTextContent {
                annotations: vec![],
                logprobs: None,
                text: " summary ".into(),
            })],
            id: "msg_1".into(),
            role: AssistantRole::Assistant,
            phase: None,
            status: OutputStatus::Completed,
        });
        let summary = SummaryStep::try_from(ModelStep {
            text: String::new(),
            calls: vec![],
            output: vec![output.clone()],
            usage: None,
        })
        .unwrap();
        assert_eq!(summary.text, "summary");
        assert!(
            SummaryStep::try_from(ModelStep {
                text: " ".into(),
                calls: vec![],
                output: vec![],
                usage: None,
            })
            .is_err()
        );
        assert!(
            SummaryStep::try_from(ModelStep {
                text: "text".into(),
                calls: vec![ToolCall::default()],
                output: vec![],
                usage: None,
            })
            .is_err()
        );
        let OutputItem::Message(mut message) = output else {
            unreachable!()
        };
        message.status = OutputStatus::Incomplete;
        assert!(
            SummaryStep::try_from(ModelStep {
                text: String::new(),
                calls: vec![],
                output: vec![OutputItem::Message(message)],
                usage: None,
            })
            .is_err()
        );
    }
}
