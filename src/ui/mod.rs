//! 在入口处组合会话能力与具体终端界面。

mod tui;

#[cfg(feature = "gui")]
mod gui;

#[cfg(any(feature = "gui", test))]
#[path = "gui/authorization.rs"]
mod gui_authorization;

#[cfg(any(feature = "gui", test))]
#[path = "gui/commands.rs"]
mod gui_commands;

#[cfg(any(feature = "gui", test))]
#[path = "gui/close.rs"]
mod gui_close;

use std::{
    error::Error,
    io::{self, IsTerminal},
};

use crate::{
    agent,
    config::{self, UiMode},
    repl,
};

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    config::load_environment()?;
    let mode = UiMode::parse(std::env::var("GEER_AGENT_UI").ok().as_deref())
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let terminal = io::stdin().is_terminal() && io::stdout().is_terminal();
    let mode = match mode {
        UiMode::Auto if terminal => UiMode::Tui,
        UiMode::Auto => UiMode::Repl,
        other => other,
    };
    match mode {
        #[cfg(feature = "gui")]
        UiMode::Gui => gui::run(),
        #[cfg(not(feature = "gui"))]
        UiMode::Gui => Err(io::Error::other(
            "当前构建未包含 GUI；请先构建前端，再用 cargo run --features gui 启动。",
        )
        .into()),
        UiMode::Tui if !terminal => {
            Err(io::Error::other("TUI 需要交互终端，请改用 GEER_AGENT_UI=repl。").into())
        }
        UiMode::Tui | UiMode::Repl => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let mut agent = agent::create().await?;
                if mode == UiMode::Tui {
                    let mut ui = tui::Tui::new()?;
                    agent.set_confirm(ui.confirmer());
                    ui.run(&mut agent).await
                } else {
                    repl::run(&mut agent).await
                }
            })
        }
        UiMode::Auto => unreachable!("auto 已解析"),
    }
}
