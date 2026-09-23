mod chat;
mod responses;

use std::{error::Error, io};

use async_openai::types::{
    chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject},
    responses::{FunctionTool, Tool},
};

use crate::{
    config::{Config, OpenAiApi},
    provider::{ChatProvider, Messages, ModelStep, ToolSpec},
};

use chat::Chat;
use responses::Responses;

fn chat_tools(specs: &[ToolSpec]) -> Vec<ChatCompletionTools> {
    specs
        .iter()
        .map(|spec| {
            ChatCompletionTools::Function(ChatCompletionTool {
                function: FunctionObject {
                    name: spec.name.clone(),
                    description: Some(spec.description.clone()),
                    parameters: Some(spec.parameters.clone()),
                    strict: None,
                },
            })
        })
        .collect()
}

fn response_tools(specs: &[ToolSpec]) -> Vec<Tool> {
    specs
        .iter()
        .map(|spec| {
            Tool::Function(FunctionTool {
                name: spec.name.clone(),
                description: Some(spec.description.clone()),
                parameters: Some(spec.parameters.clone()),
                ..Default::default()
            })
        })
        .collect()
}

pub(crate) struct Provider {
    api: Api,
}

enum Api {
    Responses(Responses),
    ChatCompletions(Chat),
}

impl Provider {
    pub(crate) fn new(config: &Config) -> Self {
        let api = match config.api {
            OpenAiApi::Responses => Api::Responses(Responses::new(config)),
            OpenAiApi::ChatCompletions => Api::ChatCompletions(Chat::new(config)),
        };
        Self { api }
    }
}

impl ChatProvider for Provider {
    async fn complete_step<F>(
        &mut self,
        messages: Messages,
        tools: &[ToolSpec],
        on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        match (&mut self.api, messages) {
            (
                Api::Responses(api),
                Messages::Responses {
                    instructions,
                    input,
                },
            ) => {
                api.complete_step(instructions, input, tools, on_delta)
                    .await
            }
            (Api::ChatCompletions(api), Messages::Chat(messages)) => {
                api.complete_step(messages, tools, on_delta).await
            }
            _ => Err(
                io::Error::new(io::ErrorKind::InvalidInput, "模型接口与消息类型不匹配。").into(),
            ),
        }
    }
}
