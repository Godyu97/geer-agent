pub(crate) mod openai;

use std::{error::Error, io};

pub(crate) trait ChatProvider {
    async fn stream_reply<F>(
        &mut self,
        user_input: &str,
        on_delta: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>;

    fn reset(&mut self);
}
