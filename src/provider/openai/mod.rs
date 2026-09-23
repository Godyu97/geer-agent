mod chat;
mod responses;

use std::{error::Error, io};

use crate::{
    config::{Config, OpenAiApi},
    provider::ChatProvider,
};

use chat::Chat;
use responses::Responses;

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
    async fn stream_reply<F>(&mut self, user_input: &str, on_delta: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>,
    {
        match &mut self.api {
            Api::Responses(api) => api.stream_reply(user_input, on_delta).await,
            Api::ChatCompletions(api) => api.stream_reply(user_input, on_delta).await,
        }
    }

    fn reset(&mut self) {
        match &mut self.api {
            Api::Responses(api) => api.reset(),
            Api::ChatCompletions(api) => api.reset(),
        }
    }
}
