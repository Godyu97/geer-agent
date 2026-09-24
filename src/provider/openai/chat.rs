use std::{
    collections::BTreeMap,
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

use async_openai::{
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
    middleware::{HttpRequestFactory, ReqwestService, retry::OpenAIRetryLayer},
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionStreamOptions, CreateChatCompletionRequestArgs,
        CreateChatCompletionStreamResponse, FinishReason,
    },
};
use futures_util::{Stream, StreamExt};
use tokio::time::{Instant, timeout_at};
use tower::{ServiceBuilder, ServiceExt, service_fn};

use crate::{
    config::Config,
    provider::{ModelStep, TokenUsage, ToolCall, ToolSpec},
    trace::TraceCapture,
};

const REPLY_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub(crate) struct Chat {
    client: Client<OpenAIConfig>,
    model: String,
    attempt_counter: Arc<AtomicI32>,
}

impl Chat {
    pub(crate) fn new(config: &Config) -> Self {
        let client_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone())
            .with_api_base(config.base_url.clone());

        let attempt_counter = Arc::new(AtomicI32::new(0));
        let count = Arc::clone(&attempt_counter);
        let service = ServiceBuilder::new()
            .layer(OpenAIRetryLayer::default())
            .service(service_fn(move |factory: HttpRequestFactory| {
                count.fetch_add(1, Ordering::Relaxed);
                ReqwestService::default().oneshot(factory)
            }));
        Self {
            client: Client::with_config(client_config).with_http_service(service),
            model: config.model.clone(),
            attempt_counter,
        }
    }

    pub(crate) async fn complete_step<F>(
        &mut self,
        messages: Vec<ChatCompletionRequestMessage>,
        tools: &[ToolSpec],
        capture: &mut TraceCapture,
        mut on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        self.attempt_counter.store(0, Ordering::Relaxed);
        capture.count_attempts_with(Arc::clone(&self.attempt_counter));
        self.request_reply(messages, tools, capture, &mut on_delta)
            .await
    }

    async fn request_reply<F>(
        &self,
        messages: Vec<ChatCompletionRequestMessage>,
        tools: &[ToolSpec],
        capture: &mut TraceCapture,
        on_delta: &mut F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        let mut request = CreateChatCompletionRequestArgs::default();
        request
            .model(self.model.clone())
            .messages(messages)
            .stream_options(ChatCompletionStreamOptions {
                include_usage: Some(true),
                include_obfuscation: None,
            });
        if !tools.is_empty() {
            request.tools(super::chat_tools(tools));
        }
        let request = request.build()?;
        capture.set_request(&request)?;
        capture.request["stream"] = serde_json::Value::Bool(true);
        let deadline = Instant::now() + REPLY_IDLE_TIMEOUT;
        let stream = timeout_at(deadline, self.client.chat().create_stream(request))
            .await
            .map_err(|_| timeout_error())??;

        let step = collect_reply(stream, capture, on_delta, REPLY_IDLE_TIMEOUT, deadline).await?;
        capture.set_response(&serde_json::json!({
            "text": step.text,
            "tool_calls": step.calls.iter().map(|call| serde_json::json!({
                "id": call.id, "name": call.name, "arguments": call.args,
            })).collect::<Vec<_>>(),
        }))?;
        Ok(step)
    }
}

async fn collect_reply<S, F>(
    mut stream: S,
    capture: &mut TraceCapture,
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

        capture.provider_response_id = Some(chunk.id.clone());
        let chunk_usage = chunk.usage.map(|usage| TokenUsage {
            input: u64::from(usage.prompt_tokens),
            output: u64::from(usage.completion_tokens),
        });
        for choice in chunk.choices {
            if choice.index != 0 {
                continue;
            }

            if let Some(content) = choice.delta.content.or(choice.delta.refusal)
                && !content.is_empty()
            {
                capture.append_text(&content);
                on_delta(&content)?;
                answer.push_str(&content);
                if !content.trim().is_empty() {
                    deadline = Instant::now() + idle_timeout;
                }
            }

            if let Some(deltas) = choice.delta.tool_calls {
                for delta in deltas {
                    let call = calls.entry(delta.index).or_default();
                    capture.append_call_delta(
                        delta.index,
                        delta.id.as_deref(),
                        delta.function.as_ref().and_then(|f| f.name.as_deref()),
                        delta.function.as_ref().and_then(|f| f.arguments.as_deref()),
                    );
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
                        output: Vec::new(),
                        usage: match chunk_usage {
                            Some(usage) => Some(usage),
                            None => tail_usage(&mut stream).await,
                        },
                    });
                }
                return match reason {
                    FinishReason::Stop | FinishReason::Length if !answer.trim().is_empty() => {
                        Ok(ModelStep {
                            text: answer,
                            calls: Vec::new(),
                            output: Vec::new(),
                            usage: match chunk_usage {
                                Some(usage) => Some(usage),
                                None => tail_usage(&mut stream).await,
                            },
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

async fn tail_usage<S>(stream: &mut S) -> Option<TokenUsage>
where
    S: Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Unpin,
{
    // 兼容接口可能不发尾包；等待有界，避免正文结束后卡住 REPL。
    let deadline = Instant::now() + Duration::from_millis(200);
    loop {
        let next = timeout_at(deadline, stream.next()).await.ok()??;
        let chunk = next.ok()?;
        if let Some(usage) = chunk.usage {
            return Some(TokenUsage {
                input: u64::from(usage.prompt_tokens),
                output: u64::from(usage.completion_tokens),
            });
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
            ChatCompletionStreamResponseDelta, CompletionUsage, CreateChatCompletionStreamResponse,
            FinishReason, FunctionCallStream,
        },
    };
    use futures_util::{StreamExt, stream};
    use tokio::time::Instant;

    use crate::{provider::ModelStep, trace::TraceCapture};

    async fn collect_reply<S, F>(
        stream: S,
        on_delta: &mut F,
        idle_timeout: Duration,
        deadline: Instant,
    ) -> Result<ModelStep, Box<dyn std::error::Error>>
    where
        S: futures_util::Stream<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>>
            + Unpin,
        F: FnMut(&str) -> io::Result<()>,
    {
        super::collect_reply(
            stream,
            &mut TraceCapture::new(),
            on_delta,
            idle_timeout,
            deadline,
        )
        .await
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
        assert_eq!(answer.usage, None);
    }

    #[tokio::test]
    async fn reads_usage_after_finish_reason() {
        let mut usage_chunk = chunk(None, None);
        usage_chunk.choices.clear();
        usage_chunk.usage = Some(CompletionUsage {
            prompt_tokens: 21,
            completion_tokens: 8,
            total_tokens: 29,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        });
        let events = stream::iter([
            Ok(chunk(Some("done"), Some(FinishReason::Stop))),
            Ok(usage_chunk),
        ]);
        let step = collect_reply(
            events,
            &mut |_| Ok(()),
            Duration::from_secs(1),
            Instant::now() + Duration::from_secs(1),
        )
        .await
        .expect("读取用量尾包");
        assert_eq!(
            step.usage,
            Some(crate::provider::TokenUsage {
                input: 21,
                output: 8
            })
        );
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
