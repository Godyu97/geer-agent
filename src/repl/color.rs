use std::{env, io, io::IsTerminal, io::Write};

const USER_COLOR: &str = "\x1b[1;36m";
const ASSISTANT_LABEL_COLOR: &str = "\x1b[1;32m";
const ASSISTANT_TEXT_COLOR: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

pub(crate) struct Color {
    enabled: bool,
}

impl Color {
    pub(crate) fn detect() -> Self {
        Self {
            enabled: io::stdout().is_terminal()
                && env::var_os("NO_COLOR").is_none()
                && !matches!(env::var("TERM").as_deref(), Ok("dumb")),
        }
    }

    pub(crate) fn write_user_prompt(&self, out: &mut impl Write) -> io::Result<()> {
        self.write_colored(out, USER_COLOR, "李火旺🔥 › ")
    }

    pub(crate) fn write_assistant_prompt(&self, out: &mut impl Write) -> io::Result<()> {
        self.write_colored(out, ASSISTANT_LABEL_COLOR, "Ai › ")
    }

    pub(crate) fn write_assistant_delta(
        &self,
        out: &mut impl Write,
        delta: &str,
    ) -> io::Result<()> {
        self.write_colored(out, ASSISTANT_TEXT_COLOR, delta)
    }

    fn write_colored(&self, out: &mut impl Write, color: &str, text: &str) -> io::Result<()> {
        if self.enabled {
            write!(out, "{color}{text}{RESET}")
        } else {
            out.write_all(text.as_bytes())
        }
    }
}
