use async_openai::types::{
    chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent, FunctionCall,
    },
    responses::{
        EasyInputContent, EasyInputMessage, FunctionCallOutput, FunctionCallOutputItemParam,
        InputItem, Item, Role,
    },
};

use crate::{
    config::OpenAiApi,
    provider::{Messages, ModelStep, ToolCall},
};

pub(crate) struct Prompt {
    system: String,
    state: State,
}

enum State {
    Chat {
        history: Vec<ChatCompletionRequestMessage>,
    },
    Responses {
        history: Vec<InputItem>,
        pending: Vec<InputItem>,
    },
}

impl Prompt {
    pub(crate) fn new(api: OpenAiApi, system: String) -> Self {
        let state = match api {
            OpenAiApi::ChatCompletions => State::Chat {
                history: Vec::new(),
            },
            OpenAiApi::Responses => State::Responses {
                history: Vec::new(),
                pending: Vec::new(),
            },
        };
        Self { system, state }
    }

    pub(crate) fn begin_turn(&mut self, input: &str) {
        match &mut self.state {
            State::Chat { history } => history.push(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(input.to_owned()),
                    name: None,
                },
            )),
            State::Responses { pending, .. } => {
                pending.clear();
                pending.push(InputItem::EasyMessage(EasyInputMessage {
                    role: Role::User,
                    content: EasyInputContent::Text(input.to_owned()),
                    ..Default::default()
                }));
            }
        }
    }

    pub(crate) fn messages(&self) -> Messages {
        self.messages_with_runtime_instruction(None)
    }

    pub(crate) fn messages_with_runtime_instruction(
        &self,
        runtime_instruction: Option<&str>,
    ) -> Messages {
        let system = match runtime_instruction {
            Some(instruction) => format!("{}\n\n{instruction}", self.system),
            None => self.system.clone(),
        };
        match &self.state {
            State::Chat { history } => {
                let mut messages = vec![ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: system.into(),
                        name: None,
                    },
                )];
                messages.extend(history.iter().cloned());
                Messages::Chat(messages)
            }
            State::Responses { history, pending } => {
                let mut input = history.clone();
                input.extend(pending.iter().cloned());
                Messages::Responses {
                    instructions: system,
                    input,
                }
            }
        }
    }

    pub(crate) fn apply_tool_results(&mut self, step: ModelStep, results: &[(ToolCall, String)]) {
        match &mut self.state {
            State::Chat { history } => {
                let calls: Vec<ToolCall> = results
                    .iter()
                    .enumerate()
                    .map(|(index, (call, _))| {
                        let mut call = call.clone();
                        if call.id.is_empty() {
                            call.id = format!("call_{}_{index}", history.len());
                        }
                        call
                    })
                    .collect();
                history.push(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: (!step.text.is_empty()).then_some(
                            ChatCompletionRequestAssistantMessageContent::Text(step.text),
                        ),
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
                    history.push(ChatCompletionRequestMessage::Tool(
                        ChatCompletionRequestToolMessage {
                            content: ChatCompletionRequestToolMessageContent::Text(result.clone()),
                            tool_call_id: call.id.clone(),
                        },
                    ));
                }
            }
            State::Responses { pending, .. } => {
                pending.extend(step.output.into_iter().map(Into::into));
                for (call, output) in results {
                    pending.push(InputItem::Item(Item::FunctionCallOutput(
                        FunctionCallOutputItemParam {
                            call_id: Some(call.id.clone()),
                            output: FunctionCallOutput::Text(output.clone()),
                            id: None,
                            status: None,
                            name: None,
                            namespace: None,
                            caller: None,
                        },
                    )));
                }
            }
        }
    }

    pub(crate) fn finish_turn(&mut self, step: ModelStep) {
        match &mut self.state {
            State::Chat { history } => {
                history.push(ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessage {
                        content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                            step.text,
                        )),
                        ..Default::default()
                    },
                ));
            }
            State::Responses { pending, .. } => {
                pending.extend(step.output.into_iter().map(Into::into));
                self.commit_turn();
            }
        }
    }

    pub(crate) fn commit_turn(&mut self) {
        if let State::Responses { history, pending } = &mut self.state {
            history.extend(std::mem::take(pending));
        }
    }

    pub(crate) fn rollback_turn(&mut self) {
        match &mut self.state {
            State::Chat { history } => {
                history.pop();
            }
            State::Responses { pending, .. } => pending.clear(),
        }
    }

    pub(crate) fn reset(&mut self) {
        match &mut self.state {
            State::Chat { history } => history.clear(),
            State::Responses { history, pending } => {
                history.clear();
                pending.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use async_openai::types::responses::{FunctionToolCall, OutputItem};
    use serde_json::{Value, json};

    use super::Prompt;
    use crate::{
        config::OpenAiApi,
        provider::{Messages, ModelStep, ToolCall},
    };

    fn text_step(text: &str) -> ModelStep {
        ModelStep {
            text: text.to_owned(),
            calls: Vec::new(),
            output: Vec::new(),
            usage: None,
        }
    }

    fn body(prompt: &Prompt) -> Value {
        body_with_instruction(prompt, None)
    }

    fn body_with_instruction(prompt: &Prompt, runtime_instruction: Option<&str>) -> Value {
        match prompt.messages_with_runtime_instruction(runtime_instruction) {
            Messages::Chat(messages) => serde_json::to_value(messages).expect("Chat 消息可序列化"),
            Messages::Responses {
                instructions,
                input,
            } => {
                json!({"instructions": instructions, "input": input})
            }
        }
    }

    #[test]
    fn chat_system_history_and_reset() {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".to_owned());
        prompt.begin_turn("first");
        prompt.finish_turn(text_step("answer"));
        prompt.begin_turn("second");
        let messages = body(&prompt);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "system");
        assert_eq!(messages[1]["content"], "first");
        assert_eq!(messages[2]["content"], "answer");
        assert_eq!(messages[3]["content"], "second");
        prompt.reset();
        assert_eq!(body(&prompt).as_array().expect("Chat 消息").len(), 1);
        assert_eq!(body(&prompt)[0]["content"], "system");
    }

    #[test]
    fn runtime_instruction_is_request_scoped_for_both_apis() {
        for api in [OpenAiApi::ChatCompletions, OpenAiApi::Responses] {
            let mut prompt = Prompt::new(api, "system".to_owned());
            prompt.begin_turn("hello");

            let with_runtime = body_with_instruction(&prompt, Some("runtime"));
            let without_runtime = body(&prompt);
            match api {
                OpenAiApi::ChatCompletions => {
                    assert_eq!(with_runtime[0]["content"], "system\n\nruntime");
                    assert_eq!(without_runtime[0]["content"], "system");
                }
                OpenAiApi::Responses => {
                    assert_eq!(with_runtime["instructions"], "system\n\nruntime");
                    assert_eq!(without_runtime["instructions"], "system");
                }
            }
        }
    }

    #[test]
    fn chat_missing_call_id_and_failed_turn() {
        let mut prompt = Prompt::new(OpenAiApi::ChatCompletions, "system".to_owned());
        prompt.begin_turn("first");
        prompt.rollback_turn();
        assert_eq!(body(&prompt).as_array().expect("Chat 消息").len(), 1);
        prompt.begin_turn("second");
        let call = ToolCall {
            id: String::new(),
            name: "clock".to_owned(),
            args: "{}".to_owned(),
        };
        prompt.apply_tool_results(
            ModelStep {
                text: String::new(),
                calls: vec![call.clone()],
                output: Vec::new(),
                usage: None,
            },
            &[(call, "now".to_owned())],
        );
        prompt.commit_turn();
        let messages = body(&prompt);
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1_0");
        assert_eq!(messages[3]["tool_call_id"], "call_1_0");
    }

    #[test]
    fn responses_keeps_full_output_and_commits_pairs() {
        let mut prompt = Prompt::new(OpenAiApi::Responses, "system".to_owned());
        prompt.begin_turn("first");
        prompt.rollback_turn();
        assert!(
            body(&prompt)["input"]
                .as_array()
                .expect("Responses 输入")
                .is_empty()
        );
        prompt.begin_turn("second");
        let call = ToolCall {
            id: "call_1".to_owned(),
            name: "clock".to_owned(),
            args: "{}".to_owned(),
        };
        let output = OutputItem::FunctionCall(FunctionToolCall {
            arguments: "{}".to_owned(),
            call_id: "call_1".to_owned(),
            namespace: None,
            name: "clock".to_owned(),
            id: Some("raw_1".to_owned()),
            status: None,
            caller: None,
            r#async: None,
        });
        prompt.apply_tool_results(
            ModelStep {
                text: String::new(),
                calls: vec![call.clone()],
                output: vec![output],
                usage: None,
            },
            &[(call, "now".to_owned())],
        );
        let pending = body(&prompt);
        assert_eq!(pending["instructions"], "system");
        assert_eq!(pending["input"][1]["id"], "raw_1");
        assert_eq!(pending["input"][2]["call_id"], "call_1");
        prompt.commit_turn();
        prompt.begin_turn("third");
        assert_eq!(
            body(&prompt)["input"]
                .as_array()
                .expect("Responses 输入")
                .len(),
            4
        );
        prompt.reset();
        assert_eq!(body(&prompt)["instructions"], "system");
        assert!(
            body(&prompt)["input"]
                .as_array()
                .expect("Responses 输入")
                .is_empty()
        );
    }
}
