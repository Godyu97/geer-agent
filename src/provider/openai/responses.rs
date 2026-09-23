use std::{error::Error, io, time::Duration};

use async_openai::{
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
    types::responses::{
        CreateResponseArgs, EasyInputContent, EasyInputMessage, InputItem, InputParam, Response,
        ResponseStreamEvent, Role,
    },
};
use futures_util::{Stream, StreamExt};
use tokio::time::{Instant, timeout, timeout_at};

use crate::config::Config;

const REPLY_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub(super) struct Responses {
    client: Client<OpenAIConfig>,
    model: String,
    history: Vec<InputItem>,
}

impl Responses {
    pub(super) fn new(config: &Config) -> Self {
        let client_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone())
            .with_api_base(config.base_url.clone());

        Self {
            client: Client::with_config(client_config),
            model: config.model.clone(),
            history: Vec::new(),
        }
    }

    pub(super) async fn stream_reply<F>(
        &mut self,
        user_input: &str,
        mut on_delta: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        let user_item = InputItem::EasyMessage(EasyInputMessage {
            role: Role::User,
            content: EasyInputContent::Text(user_input.to_owned()),
            ..Default::default()
        });
        let mut input = self.history.clone();
        input.push(user_item.clone());

        let request = CreateResponseArgs::default()
            .model(self.model.clone())
            .input(InputParam::Items(input))
            .build()?;
        let stream = timeout(
            REPLY_IDLE_TIMEOUT,
            self.client.responses().create_stream(request),
        )
        .await
        .map_err(|_| timeout_error())??;
        let response = collect_reply(stream, &mut on_delta, REPLY_IDLE_TIMEOUT).await?;

        // 只在完整成功后提交 user 和全部模型输出，避免失败轮次污染上下文。
        self.commit_turn(user_item, response);
        Ok(())
    }

    fn commit_turn(&mut self, user_item: InputItem, response: Response) {
        self.history.push(user_item);
        self.history
            .extend(response.output.into_iter().map(Into::into));
    }

    pub(super) fn reset(&mut self) {
        self.history.clear();
    }
}

async fn collect_reply<S, F>(
    mut stream: S,
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
            ResponseStreamEvent::ResponseOutputTextDelta(event) => {
                if !event.delta.is_empty() {
                    on_delta(&event.delta)?;
                    answer.push_str(&event.delta);
                    if !event.delta.trim().is_empty() {
                        deadline = Instant::now() + idle_timeout;
                    }
                }
            }
            ResponseStreamEvent::ResponseCompleted(event) => {
                if answer.trim().is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "模型已结束，但没有返回可显示的文本。",
                    )
                    .into());
                }
                return Ok(event.response);
            }
            ResponseStreamEvent::ResponseFailed(event) => {
                return Err(
                    io::Error::other(format!("模型响应失败：{:?}", event.response.error)).into(),
                );
            }
            ResponseStreamEvent::ResponseIncomplete(event) => {
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

    use async_openai::{
        error::OpenAIError,
        types::responses::{
            AssistantRole, EasyInputContent, EasyInputMessage, InputItem, OutputItem,
            OutputMessage, OutputMessageContent, OutputStatus, OutputTextContent, Response,
            ResponseCompletedEvent, ResponseFailedEvent, ResponseIncompleteEvent,
            ResponseStreamEvent, ResponseTextDeltaEvent, Role, Status,
        },
    };
    use futures_util::{StreamExt, stream};

    use super::{Responses, collect_reply};
    use crate::config::{Config, OpenAiApi};

    fn test_provider() -> Responses {
        Responses::new(&Config {
            api_key: "test-key".to_owned(),
            model: "test-model".to_owned(),
            base_url: "https://example.invalid/v1".to_owned(),
            api: OpenAiApi::Responses,
        })
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

    #[test]
    fn completed_history_is_committed_and_reset_clears_it() {
        let mut provider = test_provider();
        let user = InputItem::EasyMessage(EasyInputMessage {
            role: Role::User,
            content: EasyInputContent::Text("hello".to_owned()),
            ..Default::default()
        });
        provider.commit_turn(user, response("hi"));
        assert_eq!(provider.history.len(), 2);
        provider.reset();
        assert!(provider.history.is_empty());
    }
}
