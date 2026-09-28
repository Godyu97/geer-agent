//! 在入口处组合会话能力与具体终端界面。

mod tui;

use std::{
    error::Error,
    io::{self, IsTerminal},
};

use crate::{agent, repl};

pub(crate) async fn run() -> Result<(), Box<dyn Error>> {
    let mut agent = agent::create().await?;
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        let mut ui = tui::Tui::new()?;
        agent.set_confirm(ui.confirmer());
        ui.run(&mut agent).await
    } else {
        repl::run(&mut agent).await
    }
}
