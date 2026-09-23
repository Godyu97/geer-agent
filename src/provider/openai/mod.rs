mod chat;
mod responses;

use std::{error::Error, io};

use async_openai::types::{
    chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject},
    responses::{FunctionTool, Tool},
};

use crate::{
    config::{Config, OpenAiApi},
    provider::{ChatProvider, ModelStep, ToolCall, ToolSpec},
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
    pub(crate) fn new(config: &Config, system_prompt: String) -> Self {
        let api = match config.api {
            OpenAiApi::Responses => Api::Responses(Responses::new(config, system_prompt)),
            OpenAiApi::ChatCompletions => Api::ChatCompletions(Chat::new(config, system_prompt)),
        };
        Self { api }
    }
}

impl ChatProvider for Provider {
    fn begin_turn(&mut self, user_input: &str) {
        match &mut self.api {
            Api::Responses(api) => api.begin_turn(user_input),
            Api::ChatCompletions(api) => api.begin_turn(user_input),
        }
    }

    async fn complete_step<F>(
        &mut self,
        tools: &[ToolSpec],
        on_delta: F,
    ) -> Result<ModelStep, Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        match &mut self.api {
            Api::Responses(api) => api.complete_step(tools, on_delta).await,
            Api::ChatCompletions(api) => api.complete_step(tools, on_delta).await,
        }
    }

    fn apply_tool_results(&mut self, text: String, results: &[(ToolCall, String)]) {
        match &mut self.api {
            Api::Responses(api) => api.apply_tool_results(text, results),
            Api::ChatCompletions(api) => api.apply_tool_results(text, results),
        }
    }

    fn finish_turn(&mut self, text: String) {
        match &mut self.api {
            Api::Responses(api) => api.finish_turn(text),
            Api::ChatCompletions(api) => api.finish_turn(text),
        }
    }

    fn commit_turn(&mut self) {
        match &mut self.api {
            Api::Responses(api) => api.commit_turn(),
            Api::ChatCompletions(api) => api.commit_turn(),
        }
    }

    fn rollback_turn(&mut self) {
        match &mut self.api {
            Api::Responses(api) => api.rollback_turn(),
            Api::ChatCompletions(api) => api.rollback_turn(),
        }
    }

    fn reset(&mut self) {
        match &mut self.api {
            Api::Responses(api) => api.reset(),
            Api::ChatCompletions(api) => api.reset(),
        }
    }
}
