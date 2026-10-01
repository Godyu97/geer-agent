//! 在入口处组合会话能力与具体终端界面。

mod commands;
mod repl;
mod tui;

#[cfg(feature = "gui")]
mod gui;

#[cfg(any(feature = "gui", feature = "web", test))]
mod app;

#[cfg(feature = "web")]
mod web;

use std::{
    error::Error,
    io::{self, IsTerminal},
};

use crate::{
    agent,
    config::{self, UiMode},
};

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let terminal = io::stdin().is_terminal() && io::stdout().is_terminal();
    let startup = (|| -> Result<UiMode, Box<dyn Error>> {
        config::load_environment()?;
        let mode = UiMode::parse(std::env::var("GEER_AGENT_UI").ok().as_deref())
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        resolve_mode(mode, terminal, cfg!(feature = "desktop-gui"))
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message).into())
    })();
    let mode = match startup {
        Ok(mode) => mode,
        Err(error) => {
            #[cfg(feature = "desktop-gui")]
            return gui::run(Some(error.to_string()));
            #[cfg(not(feature = "desktop-gui"))]
            return Err(error);
        }
    };
    match mode {
        #[cfg(feature = "gui")]
        UiMode::Gui => gui::run(None),
        #[cfg(not(feature = "gui"))]
        UiMode::Gui => Err(io::Error::other(
            "当前构建未包含 GUI；请用 make build 构建通用程序，再设置 GEER_AGENT_UI=gui 启动。",
        )
        .into()),
        #[cfg(feature = "web")]
        UiMode::Web => web::run(),
        #[cfg(not(feature = "web"))]
        UiMode::Web => Err(io::Error::other(
            "当前构建未包含 Web UI；请用 make build 构建通用程序，再设置 GEER_AGENT_UI=web 启动。",
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
                    agent.set_confirm(repl::confirm);
                    repl::run(&mut agent).await
                }
            })
        }
        UiMode::Auto => unreachable!("auto 已解析"),
    }
}

fn resolve_mode(mode: UiMode, terminal: bool, desktop: bool) -> Result<UiMode, &'static str> {
    if desktop {
        return match mode {
            UiMode::Auto | UiMode::Gui => Ok(UiMode::Gui),
            UiMode::Tui | UiMode::Repl | UiMode::Web => Err(
                "桌面构建只支持 GUI；请将 GEER_AGENT_UI 设为 auto 或 gui。使用终端界面请运行不含 desktop-gui 的构建（make run）。",
            ),
        };
    }
    Ok(match mode {
        UiMode::Auto if terminal => UiMode::Tui,
        UiMode::Auto => UiMode::Repl,
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::{UiMode, resolve_mode};

    #[test]
    fn desktop_defaults_to_gui_with_or_without_terminal() {
        for terminal in [false, true] {
            for mode in [UiMode::Auto, UiMode::Gui] {
                assert_eq!(resolve_mode(mode, terminal, true), Ok(UiMode::Gui));
            }
            for mode in [UiMode::Tui, UiMode::Repl, UiMode::Web] {
                assert!(
                    resolve_mode(mode, terminal, true)
                        .unwrap_err()
                        .contains("make run")
                );
            }
        }
    }

    #[test]
    fn terminal_build_keeps_existing_selection() {
        assert_eq!(resolve_mode(UiMode::Auto, true, false), Ok(UiMode::Tui));
        assert_eq!(resolve_mode(UiMode::Auto, false, false), Ok(UiMode::Repl));
        for mode in [UiMode::Gui, UiMode::Tui, UiMode::Repl, UiMode::Web] {
            assert_eq!(resolve_mode(mode, false, false), Ok(mode));
        }
    }
}
