use std::{collections::BTreeMap, error::Error, io, time::Duration};

use async_openai::{
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
    types::chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequestArgs,
        CreateChatCompletionStreamResponse, FinishReason, FunctionCall,
    },
};
use futures_util::{Stream, StreamExt};
use tokio::time::{Instant, timeout_at};

use crate::{
    config::Config,
    provider::{ModelStep, ToolCall, ToolSpec},
};

const REPLY_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub(crate) struct Chat {
    client: Client<OpenAIConfig>,
    model: String,
    history: Vec<ChatCompletionRequestMessage>,
}

impl Chat {
    pub(crate) fn new(config: &Config) -> Self {
        let client_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone())
            .with_api_base(config.base_url.clone());

        Self {
            client: Client::with_config(client_config),
            model: config.model.clone(),
            history: Vec::new(),
        }
    }

    pub(crate) async fn complete_step<F>(
        &mut self,
        tools: &[ToolSpec],
        mut on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        self.request_reply(tools, &mut on_delta).await
    }

    pub(crate) fn apply_tool_results(&mut self, text: String, results: &[(ToolCall, String)]) {
        let calls: Vec<ToolCall> = results
            .iter()
            .enumerate()
            .map(|(index, (call, _))| {
                let mut call = call.clone();
                if call.id.is_empty() {
                    call.id = format!("call_{}_{index}", self.history.len());
                }
                call
            })
            .collect();
        self.history.push(ChatCompletionRequestMessage::Assistant(
            ChatCompletionRequestAssistantMessage {
                content: (!text.is_empty())
                    .then_some(ChatCompletionRequestAssistantMessageContent::Text(text)),
                tool_calls: Some(
                    calls
                        .iter()
                        .map(|call| {
                            ChatCompletionMessageToolCalls::Function(
                                ChatCompletionMessageToolCall {
                                    id: call.id.clone(),
                                    function: FunctionCall {
                                        name: call.name.clone(),
                                        arguments: call.args.clone(),
                                    },
                                },
                            )
                        })
                        .collect(),
                ),
                ..Default::default()
            },
        ));
        for (call, result) in calls.iter().zip(results.iter().map(|(_, result)| result)) {
            self.history.push(ChatCompletionRequestMessage::Tool(
                ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::Text(result.clone()),
                    tool_call_id: call.id.clone(),
                },
            ));
        }
    }

    /// Chat Completions 没有 pending：出错或轮次用尽时 history 已经是最终状态。
    #[allow(clippy::unused_self, clippy::needless_pass_by_ref_mut)]
    pub(crate) fn commit_turn(&mut self) {}

    pub(crate) fn reset(&mut self) {
        self.history.clear();
    }

    pub(crate) fn begin_turn(&mut self, user_input: &str) {
        self.history.push(ChatCompletionRequestMessage::User(
            ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(user_input.to_owned()),
                name: None,
            },
        ));
    }

    pub(crate) fn finish_turn(&mut self, answer: String) {
        self.history.push(ChatCompletionRequestMessage::Assistant(
            ChatCompletionRequestAssistantMessage {
                content: Some(ChatCompletionRequestAssistantMessageContent::Text(answer)),
                ..Default::default()
            },
        ));
    }

    pub(crate) fn rollback_turn(&mut self) {
        self.history.pop();
    }

    async fn request_reply<F>(
        &self,
        tools: &[ToolSpec],
        on_delta: &mut F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        let mut request = CreateChatCompletionRequestArgs::default();
        request
            .model(self.model.clone())
            .messages(self.history.clone());
        if !tools.is_empty() {
            request.tools(super::chat_tools(tools));
        }
        let request = request.build()?;
        let deadline = Instant::now() + REPLY_IDLE_TIMEOUT;
        let stream = timeout_at(deadline, self.client.chat().create_stream(request))
            .await
            .map_err(|_| timeout_error())??;

        collect_reply(stream, on_delta, REPLY_IDLE_TIMEOUT, deadline).await
    }
}

async fn collect_reply<S, F>(
    mut stream: S,
    on_delta: &mut F,
    idle_timeout: Duration,
    mut deadline: Instant,
) -> Result<ModelStep, Box<dyn Error>>
where
    S: Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Unpin,
    F: FnMut(&str) -> io::Result<()>,
{
    let mut answer = String::new();
    let mut calls: BTreeMap<u32, ToolCall> = BTreeMap::new();

    loop {
        let chunk = timeout_at(deadline, stream.next())
            .await
            .map_err(|_| timeout_error())?
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "模型响应未完成便断开。")
            })??;

        for choice in chunk.choices {
            if choice.index != 0 {
                continue;
            }

            if let Some(content) = choice.delta.content.or(choice.delta.refusal)
                && !content.is_empty()
            {
                on_delta(&content)?;
                answer.push_str(&content);
                if !content.trim().is_empty() {
                    deadline = Instant::now() + idle_timeout;
                }
            }

            if let Some(deltas) = choice.delta.tool_calls {
                for delta in deltas {
                    let call = calls.entry(delta.index).or_default();
                    if let Some(id) = delta.id {
                        call.id.push_str(&id);
                    }
                    if let Some(function) = delta.function {
                        if let Some(name) = function.name {
                            call.name.push_str(&name);
                        }
                        if let Some(args) = function.arguments {
                            call.args.push_str(&args);
                        }
                    }
                    deadline = Instant::now() + idle_timeout;
                }
            }

            if let Some(reason) = choice.finish_reason {
                if !calls.is_empty()
                    && matches!(reason, FinishReason::ToolCalls | FinishReason::Stop)
                {
                    if calls.values().any(|call| call.name.is_empty()) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "模型工具调用缺少名称。",
                        )
                        .into());
                    }
                    return Ok(ModelStep {
                        text: answer,
                        calls: calls.into_values().collect(),
                    });
                }
                return match reason {
                    FinishReason::Stop | FinishReason::Length if !answer.trim().is_empty() => {
                        Ok(ModelStep {
                            text: answer,
                            calls: Vec::new(),
                        })
                    }
                    FinishReason::Stop | FinishReason::Length => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "模型已结束，但没有返回可显示的文本。",
                    )
                    .into()),
                    other => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("模型未返回完整文本（结束原因：{other:?}）。"),
                    )
                    .into()),
                };
            }
        }
    }
}

fn timeout_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "模型长时间没有返回可显示的文本，已取消本轮请求。",
    )
}

#[cfg(test)]
mod tests {
    use std::{io, time::Duration};

    use async_openai::{
        error::OpenAIError,
        types::chat::{
            ChatChoiceStream, ChatCompletionMessageToolCallChunk,
            ChatCompletionStreamResponseDelta, CreateChatCompletionStreamResponse, FinishReason,
            FunctionCallStream,
        },
    };
    use futures_util::{StreamExt, stream};
    use tokio::time::Instant;

    use super::{Chat, collect_reply};
    use crate::config::{Config, OpenAiApi};

    fn test_chat() -> Chat {
        Chat::new(&Config {
            api_key: "test-key".to_owned(),
            model: "test-model".to_owned(),
            base_url: "https://example.invalid/v1".to_owned(),
            api: OpenAiApi::ChatCompletions,
            tools_enabled: true,
        })
    }

    #[test]
    fn completed_turns_are_kept_and_failed_turn_is_rolled_back() {
        let mut chat = test_chat();
        chat.begin_turn("remember this");
        chat.finish_turn("I will remember.".to_owned());
        assert_eq!(chat.history.len(), 2);

        chat.begin_turn("this request will fail");
        chat.rollback_turn();
        assert_eq!(chat.history.len(), 2);
    }

    #[test]
    fn reset_clears_all_messages() {
        let mut chat = test_chat();
        chat.begin_turn("hello");
        chat.finish_turn("hi".to_owned());

        chat.reset();

        assert!(chat.history.is_empty());
    }

    #[allow(deprecated)]
    fn chunk(
        content: Option<&str>,
        finish_reason: Option<FinishReason>,
    ) -> CreateChatCompletionStreamResponse {
        CreateChatCompletionStreamResponse {
            id: "test".to_owned(),
            choices: vec![ChatChoiceStream {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    content: content.map(str::to_owned),
                    function_call: None,
                    tool_calls: None,
                    role: None,
                    refusal: None,
                },
                finish_reason,
                logprobs: None,
            }],
            created: 0,
            model: "test".to_owned(),
            service_tier: None,
            system_fingerprint: None,
            object: "chat.completion.chunk".to_owned(),
            usage: None,
            obfuscation: None,
            moderation: None,
        }
    }

    fn tool_chunk(
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        args: Option<&str>,
        finish: Option<FinishReason>,
    ) -> CreateChatCompletionStreamResponse {
        let mut chunk = chunk(None, finish);
        chunk.choices[0].delta.tool_calls = Some(vec![ChatCompletionMessageToolCallChunk {
            index,
            id: id.map(str::to_owned),
            r#type: None,
            function: Some(FunctionCallStream {
                name: name.map(str::to_owned),
                arguments: args.map(str::to_owned),
            }),
        }]);
        chunk
    }

    #[tokio::test]
    async fn fragmented_multiple_tool_calls_are_collected_by_index() {
        let events = stream::iter([
            Ok(tool_chunk(1, Some("id_1"), Some("get_"), Some("{"), None)),
            Ok(tool_chunk(
                0,
                Some("id_0"),
                Some("get_current_time"),
                Some("{}"),
                None,
            )),
            Ok(tool_chunk(
                1,
                None,
                Some("current_time"),
                Some("}"),
                Some(FinishReason::ToolCalls),
            )),
        ]);
        let step = collect_reply(
            events,
            &mut |_| Ok(()),
            Duration::from_secs(1),
            Instant::now() + Duration::from_secs(1),
        )
        .await
        .expect("完整工具调用应完成");
        assert_eq!(step.calls.len(), 2);
        assert_eq!(step.calls[0].id, "id_0");
        assert_eq!(step.calls[1].name, "get_current_time");
        assert_eq!(step.calls[1].args, "{}");
    }

    #[tokio::test]
    async fn idle_stream_times_out_without_text() {
        let stream = stream::pending::<Result<CreateChatCompletionStreamResponse, OpenAIError>>();
        let idle_timeout = Duration::from_millis(10);

        let error = collect_reply(
            stream,
            &mut |_| Ok(()),
            idle_timeout,
            Instant::now() + idle_timeout,
        )
        .await
        .expect_err("没有模型文本时应超时");

        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::TimedOut)
        );
    }

    #[tokio::test]
    async fn finished_stream_without_text_reports_error() {
        let stream = stream::iter([Ok(chunk(None, Some(FinishReason::Stop)))]);
        let idle_timeout = Duration::from_secs(1);

        let error = collect_reply(
            stream,
            &mut |_| Ok(()),
            idle_timeout,
            Instant::now() + idle_timeout,
        )
        .await
        .expect_err("空回复应报错");

        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::InvalidData)
        );
    }

    #[tokio::test]
    async fn finish_reason_ends_stream_without_waiting_for_connection_close() {
        let stream = stream::iter([Ok(chunk(Some("hello"), Some(FinishReason::Stop)))])
            .chain(stream::pending());
        let mut output = String::new();
        let idle_timeout = Duration::from_secs(1);

        let answer = collect_reply(
            stream,
            &mut |delta| {
                output.push_str(delta);
                Ok(())
            },
            idle_timeout,
            Instant::now() + idle_timeout,
        )
        .await
        .expect("完成标记应立即结束读取");

        assert_eq!(answer.text, "hello");
        assert_eq!(output, "hello");
    }

    #[tokio::test]
    async fn interrupted_stream_does_not_commit_partial_reply() {
        let stream = stream::iter([Ok(chunk(Some("partial"), None))]);
        let idle_timeout = Duration::from_secs(1);

        let error = collect_reply(
            stream,
            &mut |_| Ok(()),
            idle_timeout,
            Instant::now() + idle_timeout,
        )
        .await
        .expect_err("没有完成标记应报错");

        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::UnexpectedEof)
        );
    }
}
