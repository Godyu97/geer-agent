//! 文本 UI：输入、确认与渲染；会话执行交给 interaction。

mod authorization;
mod color;

pub(super) use authorization::confirm;

use std::{error::Error, io, io::BufRead, io::IsTerminal, io::Write};

use super::commands::error_text;
use crate::interaction::{self, CommandOutcome, Input, Operation, Session};
use color::Color;

pub(super) async fn run(session: &mut impl Session) -> Result<(), Box<dyn Error>> {
    let color = Color::detect();
    let stdin = io::stdin();

    println!("GeekAgent —— 最简单的 Agent");
    println!("Workspace: {}", session.workspace());
    println!("Session ID: {}", session.session_id());
    println!("输入 /help 查看命令。\n");

    loop {
        {
            let mut stdout = io::stdout().lock();
            color.write_user_prompt(&mut stdout)?;
            stdout.flush()?;
        }

        let line = match read_input_line(&mut stdin.lock())? {
            InputLine::Line(line) => line,
            InputLine::InvalidUtf8 => {
                eprintln!("输入包含无效 UTF-8，请重新输入。");
                continue;
            }
            InputLine::Eof => break,
        };
        let command = interaction::parse_input(&line);
        if matches!(
            &command,
            Input::Delete {
                confirmed: false,
                ..
            }
        ) && (!stdin.is_terminal() || !io::stdout().is_terminal())
        {
            eprintln!(
                "非交互输入删除会话需要 --yes。{}",
                crate::session::DELETE_USAGE
            );
            println!("已取消删除。");
            continue;
        }
        let streaming = matches!(&command, Input::Message(_));
        if streaming {
            let mut stdout = io::stdout().lock();
            color.write_assistant_prompt(&mut stdout)?;
            stdout.flush()?;
        }

        let mut result = interaction::execute(
            session,
            command,
            |delta| {
                let mut stdout = io::stdout().lock();
                if delta.starts_with("\n[调用工具") || delta.starts_with("\n[工具调用轮次")
                {
                    color.write_tool_delta(&mut stdout, delta)?;
                } else {
                    color.write_assistant_delta(&mut stdout, delta)?;
                }
                stdout.flush()
            },
            |_| Ok(()),
        )
        .await;
        if streaming {
            println!();
        }

        match &result {
            Ok(CommandOutcome::DeletePreview(preview)) => {
                println!("{}", preview.text());
                print!("确认删除？[y/N] ");
                io::stdout().flush()?;
                if !read_delete_confirmation(&mut stdin.lock())? {
                    println!("已取消删除。");
                    continue;
                }
                result = interaction::execute(
                    session,
                    Input::Delete {
                        ids: preview.ids(),
                        confirmed: true,
                    },
                    |_| Ok(()),
                    |_| Ok(()),
                )
                .await;
            }
            Err(error) if error.operation == Operation::DeletePreview => {
                eprintln!("{}", error_text(error));
                println!("已取消删除。");
                continue;
            }
            _ => {}
        }

        match result {
            Ok(outcome) => {
                if render(outcome) {
                    break;
                }
            }
            Err(error) => eprintln!("{}", error_text(&error)),
        }
    }

    let report = interaction::save(session).await;
    if !report.is_empty() {
        println!("{report}");
    }
    println!("bye");
    Ok(())
}

fn render(outcome: CommandOutcome) -> bool {
    match outcome {
        CommandOutcome::Empty | CommandOutcome::Message => {}
        CommandOutcome::Exit => return true,
        CommandOutcome::Help => println!("{}", interaction::help_text()),
        CommandOutcome::NewSession { session_id, reset } => {
            if reset {
                println!("（已清空对话记忆，开始新会话）");
            } else {
                println!("（已开始新会话）");
            }
            println!("Session ID: {session_id}");
        }
        CommandOutcome::Saved(message)
        | CommandOutcome::Compacted(message)
        | CommandOutcome::WorkspaceChanged(message) => println!("{message}"),
        CommandOutcome::Sessions(items) if items.is_empty() => {
            println!("没有可列出的会话（或未配置会话数据库）。")
        }
        CommandOutcome::Sessions(items) => {
            for item in items {
                println!("{item}");
            }
        }
        CommandOutcome::DeletePreview(preview) => println!("{}", preview.text()),
        CommandOutcome::Deleted(report) => println!("{}", report.text()),
        CommandOutcome::Workspace(path) => println!("Workspace: {path}"),
        CommandOutcome::Opened {
            message,
            session_id,
        } => {
            println!("{message}");
            println!("Session ID: {session_id}");
        }
        CommandOutcome::Unknown(command) => {
            println!("未知命令：{command}（输入 /help 查看可用命令）");
        }
    }
    false
}

fn read_delete_confirmation(reader: &mut impl BufRead) -> io::Result<bool> {
    Ok(match read_input_line(reader)? {
        InputLine::Line(line) => matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
        InputLine::InvalidUtf8 | InputLine::Eof => false,
    })
}

enum InputLine {
    Line(String),
    InvalidUtf8,
    Eof,
}

fn read_input_line(reader: &mut impl BufRead) -> io::Result<InputLine> {
    let mut bytes = Vec::new();
    if reader.read_until(b'\n', &mut bytes)? == 0 {
        return Ok(InputLine::Eof);
    }
    match String::from_utf8(bytes) {
        Ok(line) => Ok(InputLine::Line(line)),
        Err(_) => Ok(InputLine::InvalidUtf8),
    }
}

#[cfg(test)]
mod tests {
    use super::{InputLine, read_input_line};

    #[test]
    fn confirmation_requires_explicit_yes_and_rejects_eof_or_invalid_utf8() {
        for answer in ["\n", "n\n", "maybe\n", "y then delete\n", ""] {
            assert!(!super::read_delete_confirmation(&mut answer.as_bytes()).unwrap());
        }
        assert!(!super::read_delete_confirmation(&mut &b"\xff\n"[..]).unwrap());
        assert!(super::read_delete_confirmation(&mut "YES\n".as_bytes()).unwrap());
    }

    #[test]
    fn invalid_utf8_only_discards_its_own_line() {
        let mut input = &b"hello\n\xff\n/exit\n"[..];
        assert!(
            matches!(read_input_line(&mut input).unwrap(), InputLine::Line(line) if line == "hello\n")
        );
        assert!(matches!(
            read_input_line(&mut input).unwrap(),
            InputLine::InvalidUtf8
        ));
        assert!(
            matches!(read_input_line(&mut input).unwrap(), InputLine::Line(line) if line == "/exit\n")
        );
        assert!(matches!(
            read_input_line(&mut input).unwrap(),
            InputLine::Eof
        ));
    }
}
