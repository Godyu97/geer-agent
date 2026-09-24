use std::{error::Error, io, time::Duration};

use async_openai::{
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
    middleware::ReqwestService,
    types::responses::{
        CreateResponseArgs, InputItem, InputParam, OutputItem, Response, ResponseStreamEvent,
    },
};
use futures_util::{Stream, StreamExt};
use tokio::time::{Instant, sleep, timeout, timeout_at};

use crate::{
    config::Config,
    provider::{ModelStep, TokenUsage, ToolCall, ToolSpec},
    trace::TraceCapture,
};

const REPLY_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_RETRIES: usize = 5;
const RETRY_DELAY: Duration = Duration::from_millis(200);

pub(super) struct Responses {
    client: Client<OpenAIConfig>,
    model: String,
}

impl Responses {
    pub(super) fn new(config: &Config) -> Self {
        let client_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone())
            .with_api_base(config.base_url.clone());

        Self {
            // SDK 默认会静默重试；这里由外层循环控制次数并显示进度。
            client: Client::with_config(client_config).with_http_service(ReqwestService::default()),
            model: config.model.clone(),
        }
    }

    pub(super) async fn complete_step<F>(
        &mut self,
        instructions: String,
        input: Vec<InputItem>,
        tools: &[ToolSpec],
        capture: &mut TraceCapture,
        mut on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        let response = self
            .request_reply(instructions, input, tools, capture, &mut on_delta)
            .await?;
        let calls: Vec<ToolCall> = response
            .output
            .iter()
            .filter_map(|item| match item {
                OutputItem::FunctionCall(call) => Some(ToolCall {
                    id: call.call_id.clone(),
                    name: call.name.clone(),
                    args: call.arguments.clone(),
                }),
                _ => None,
            })
            .collect();
        if calls
            .iter()
            .any(|call| call.id.is_empty() || call.name.is_empty())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "模型工具调用缺少名称或调用标识。",
            )
            .into());
        }
        Ok(ModelStep {
            text: String::new(),
            calls,
            usage: response.usage.as_ref().map(|usage| TokenUsage {
                input: u64::from(usage.input_tokens),
                output: u64::from(usage.output_tokens),
            }),
            output: response.output,
        })
    }

    async fn request_reply<F>(
        &self,
        instructions: String,
        input: Vec<InputItem>,
        tools: &[ToolSpec],
        capture: &mut TraceCapture,
        on_delta: &mut F,
    ) -> Result<Response, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        let mut builder = CreateResponseArgs::default();
        builder
            .model(self.model.clone())
            .input(InputParam::Items(input))
            .instructions(instructions);
        if !tools.is_empty() {
            builder.tools(super::response_tools(tools));
        }
        let request = builder.build()?;
        capture.set_request(&request)?;
        capture.request["stream"] = serde_json::Value::Bool(true);
        let mut retries = 0;
        loop {
            capture.clear_partial();
            capture.attempts = i32::try_from(retries + 1).unwrap_or(i32::MAX);
            let mut emitted_text = false;
            let result = match timeout(
                REPLY_IDLE_TIMEOUT,
                self.client.responses().create_stream(request.clone()),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    collect_reply(
                        stream,
                        capture,
                        &mut |delta| {
                            // 已交给终端的正文不能在下一次尝试中重复显示。
                            emitted_text = true;
                            on_delta(delta)
                        },
                        REPLY_IDLE_TIMEOUT,
                    )
                    .await
                }
                Ok(Err(error)) => Err(error.into()),
                Err(_) => Err(timeout_error().into()),
            };

            match result {
                Ok(response) => return Ok(response),
                Err(error) if emitted_text || retries == MAX_RETRIES => return Err(error),
                Err(_) => {
                    retries += 1;
                    on_delta(&format!("retry {retries}/{MAX_RETRIES}...\n"))?;
                    sleep(RETRY_DELAY).await;
                }
            }
        }
    }
}

async fn collect_reply<S, F>(
    mut stream: S,
    capture: &mut TraceCapture,
    on_delta: &mut F,
    idle_timeout: Duration,
) -> Result<Response, Box<dyn Error>>
where
    S: Stream<Item = Result<ResponseStreamEvent, OpenAIError>> + Unpin,
    F: FnMut(&str) -> io::Result<()>,
{
    let mut answer = String::new();
    let mut deadline = Instant::now() + idle_timeout;

    loop {
        let event = timeout_at(deadline, stream.next())
            .await
            .map_err(|_| timeout_error())?
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "模型响应未完成便断开。")
            })??;

        match event {
            ResponseStreamEvent::ResponseCreated(event) => {
                capture.provider_response_id = Some(event.response.id);
            }
            ResponseStreamEvent::ResponseInProgress(event) => {
                capture.provider_response_id = Some(event.response.id);
            }
            ResponseStreamEvent::ResponseOutputItemAdded(event) => {
                if let OutputItem::FunctionCall(call) = event.item {
                    capture.replace_call(
                        event.output_index,
                        &call.call_id,
                        &call.name,
                        &call.arguments,
                    );
                }
            }
            ResponseStreamEvent::ResponseOutputItemDone(event) => {
                if let OutputItem::FunctionCall(call) = event.item {
                    capture.replace_call(
                        event.output_index,
                        &call.call_id,
                        &call.name,
                        &call.arguments,
                    );
                }
            }
            ResponseStreamEvent::ResponseOutputTextDelta(event) => {
                if !event.delta.is_empty() {
                    capture.append_text(&event.delta);
                    on_delta(&event.delta)?;
                    answer.push_str(&event.delta);
                    if !event.delta.trim().is_empty() {
                        deadline = Instant::now() + idle_timeout;
                    }
                }
            }
            ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(event) => {
                capture.append_call_delta(event.output_index, None, None, Some(&event.delta));
                deadline = Instant::now() + idle_timeout;
            }
            ResponseStreamEvent::ResponseRefusalDelta(event) => {
                capture.append_text(&event.delta);
            }
            ResponseStreamEvent::ResponseCompleted(event) => {
                capture.provider_response_id = Some(event.response.id.clone());
                if answer.trim().is_empty()
                    && !event
                        .response
                        .output
                        .iter()
                        .any(|item| matches!(item, OutputItem::FunctionCall(_)))
                {
                    capture.set_failure_response(&event.response)?;
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "模型已结束，但没有返回可显示的文本。",
                    )
                    .into());
                }
                capture.set_response(&event.response)?;
                return Ok(event.response);
            }
            ResponseStreamEvent::ResponseFailed(event) => {
                capture.provider_response_id = Some(event.response.id.clone());
                capture.set_failure_response(&event.response)?;
                return Err(
                    io::Error::other(format!("模型响应失败：{:?}", event.response.error)).into(),
                );
            }
            ResponseStreamEvent::ResponseIncomplete(event) => {
                capture.provider_response_id = Some(event.response.id.clone());
                capture.set_failure_response(&event.response)?;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("模型响应未完成：{:?}", event.response.incomplete_details),
                )
                .into());
            }
            _ => {}
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

    use async_openai::types::responses::{InputTokenDetails, OutputTokenDetails, ResponseUsage};
    use async_openai::{
        error::OpenAIError,
        types::responses::{
            AssistantRole, FunctionToolCall, OutputItem, OutputMessage, OutputMessageContent,
            OutputStatus, OutputTextContent, Response, ResponseCompletedEvent, ResponseFailedEvent,
            ResponseIncompleteEvent, ResponseStreamEvent, ResponseTextDeltaEvent, Status,
        },
    };
    use futures_util::{StreamExt, stream};

    use crate::trace::TraceCapture;

    async fn collect_reply<S, F>(
        stream: S,
        on_delta: &mut F,
        idle_timeout: Duration,
    ) -> Result<Response, Box<dyn std::error::Error>>
    where
        S: futures_util::Stream<Item = Result<ResponseStreamEvent, OpenAIError>> + Unpin,
        F: FnMut(&str) -> io::Result<()>,
    {
        super::collect_reply(stream, &mut TraceCapture::new(), on_delta, idle_timeout).await
    }

    fn delta(text: &str) -> ResponseStreamEvent {
        ResponseStreamEvent::ResponseOutputTextDelta(ResponseTextDeltaEvent {
            sequence_number: 1,
            item_id: "msg_1".to_owned(),
            output_index: 0,
            content_index: 0,
            delta: text.to_owned(),
            logprobs: None,
        })
    }

    fn completed() -> ResponseStreamEvent {
        ResponseStreamEvent::ResponseCompleted(ResponseCompletedEvent {
            sequence_number: 2,
            response: response("hello"),
        })
    }

    #[tokio::test]
    async fn completed_function_call_without_text_is_valid() {
        let mut result = response("");
        result.output = vec![OutputItem::FunctionCall(FunctionToolCall {
            arguments: "{}".to_owned(),
            call_id: "call_1".to_owned(),
            namespace: None,
            name: "get_current_time".to_owned(),
            id: None,
            status: None,
            caller: None,
            r#async: None,
        })];
        let events = stream::iter([Ok(ResponseStreamEvent::ResponseCompleted(
            ResponseCompletedEvent {
                sequence_number: 2,
                response: result,
            },
        ))]);
        let collected = collect_reply(events, &mut |_| Ok(()), Duration::from_secs(1))
            .await
            .expect("函数调用不要求正文");
        assert!(
            matches!(&collected.output[0], OutputItem::FunctionCall(call) if call.call_id == "call_1")
        );
    }

    #[allow(deprecated)]
    fn response(text: &str) -> Response {
        Response {
            background: None,
            billing: None,
            conversation: None,
            created_at: 0,
            completed_at: Some(0),
            error: None,
            id: "resp_1".to_owned(),
            incomplete_details: None,
            instructions: None,
            max_output_tokens: None,
            metadata: None,
            model: "test-model".to_owned(),
            object: "response".to_owned(),
            output: vec![OutputItem::Message(OutputMessage {
                content: vec![OutputMessageContent::OutputText(OutputTextContent {
                    annotations: Vec::new(),
                    logprobs: None,
                    text: text.to_owned(),
                })],
                id: "msg_1".to_owned(),
                role: AssistantRole::Assistant,
                phase: None,
                status: OutputStatus::Completed,
            })],
            parallel_tool_calls: None,
            previous_response_id: None,
            prompt: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            reasoning: None,
            safety_identifier: None,
            service_tier: None,
            status: Status::Completed,
            temperature: None,
            text: None,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: None,
            truncation: None,
            usage: None,
            prompt_cache_options: None,
            prompt_cache_diagnostics: None,
            moderation: None,
        }
    }

    #[tokio::test]
    async fn completed_stream_emits_text_and_returns_without_waiting_for_eof() {
        let events = stream::iter([Ok(delta("hel")), Ok(delta("lo")), Ok(completed())])
            .chain(stream::pending::<Result<ResponseStreamEvent, OpenAIError>>());
        let mut printed = String::new();

        collect_reply(
            events,
            &mut |text| {
                printed.push_str(text);
                Ok(())
            },
            Duration::from_secs(1),
        )
        .await
        .expect("完成事件应结束流");

        assert_eq!(printed, "hello");
    }

    #[tokio::test]
    async fn completed_response_carries_token_usage() {
        let mut completed = response("done");
        completed.usage = Some(ResponseUsage {
            input_tokens: 31,
            input_tokens_details: InputTokenDetails {
                cached_tokens: 0,
                cache_write_tokens: None,
            },
            output_tokens: 9,
            output_tokens_details: OutputTokenDetails {
                reasoning_tokens: 0,
            },
            total_tokens: 40,
        });
        let event = ResponseStreamEvent::ResponseCompleted(ResponseCompletedEvent {
            sequence_number: 2,
            response: completed,
        });
        let result = collect_reply(
            stream::iter([Ok(delta("done")), Ok(event)]),
            &mut |_| Ok(()),
            Duration::from_secs(1),
        )
        .await
        .expect("完成响应");
        let usage = result.usage.expect("应有用量");
        assert_eq!((usage.input_tokens, usage.output_tokens), (31, 9));
    }

    #[tokio::test]
    async fn incomplete_or_failed_stream_is_an_error() {
        let cases = [
            ResponseStreamEvent::ResponseFailed(ResponseFailedEvent {
                sequence_number: 1,
                response: response(""),
            }),
            ResponseStreamEvent::ResponseIncomplete(ResponseIncompleteEvent {
                sequence_number: 1,
                response: response(""),
            }),
        ];

        for event in cases {
            let result = collect_reply(
                stream::iter([Ok(event)]),
                &mut |_| Ok(()),
                Duration::from_secs(1),
            )
            .await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn failed_event_keeps_earlier_stream_text_in_trace() {
        let events = stream::iter([
            Ok(delta("part")),
            Ok(ResponseStreamEvent::ResponseFailed(ResponseFailedEvent {
                sequence_number: 2,
                response: response(""),
            })),
        ]);
        let mut capture = TraceCapture::new();
        let result = super::collect_reply(
            events,
            &mut capture,
            &mut |_| Ok(()),
            Duration::from_secs(1),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            capture.response().unwrap()["received_before_failure"]["text"],
            "part"
        );
    }

    #[tokio::test]
    async fn empty_completed_stream_and_early_eof_fail() {
        for events in [vec![completed()], vec![delta("partial")]] {
            let result = collect_reply(
                stream::iter(events.into_iter().map(Ok)),
                &mut |_| Ok(()),
                Duration::from_secs(1),
            )
            .await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn idle_stream_times_out() {
        let events = stream::pending::<Result<ResponseStreamEvent, OpenAIError>>();
        let error = collect_reply(events, &mut |_| Ok(()), Duration::from_millis(10))
            .await
            .expect_err("没有正文时应超时");
        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::TimedOut)
        );
    }
}
