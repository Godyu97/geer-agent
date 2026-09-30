use std::{error::Error, io, io::BufRead, io::IsTerminal, io::Write};

use super::color::Color;
use crate::interaction::{Input, Session, help_text, parse_input};

pub(crate) async fn run(session: &mut impl Session) -> Result<(), Box<dyn Error>> {
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
            InputLine::Eof => {
                let report = session.flush().await;
                if !report.is_empty() {
                    println!("{report}");
                }
                println!("bye");
                break;
            }
        };

        match parse_input(&line) {
            Input::Empty => {}
            Input::Invalid(message) => eprintln!("{message}"),
            Input::Help => print_help(),
            Input::Reset => {
                session.new_session().await;
                println!("（已清空对话记忆，开始新会话）");
                println!("Session ID: {}", session.session_id());
            }
            Input::New => {
                session.new_session().await;
                println!("（已开始新会话）");
                println!("Session ID: {}", session.session_id());
            }
            Input::Exit => {
                let report = session.flush().await;
                if !report.is_empty() {
                    println!("{report}");
                }
                println!("bye");
                break;
            }
            Input::Save => println!("{}", session.flush().await),
            Input::Compact => match session.compact().await {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("上下文压缩失败：{error}"),
            },
            Input::Sessions(scope) => match session.sessions(scope).await {
                Ok(items) if items.is_empty() => {
                    println!("没有可列出的会话（或未配置会话数据库）。")
                }
                Ok(items) => {
                    for item in items {
                        println!("{item}");
                    }
                }
                Err(error) => eprintln!("会话列表读取失败：{error}"),
            },
            Input::Delete { ids, confirmed } => {
                let allowed = if confirmed {
                    true
                } else if !stdin.is_terminal() || !io::stdout().is_terminal() {
                    eprintln!(
                        "非交互输入删除会话需要 --yes。{}",
                        crate::session::DELETE_USAGE
                    );
                    false
                } else {
                    match session.preview_delete(&ids).await {
                        Ok(preview) => {
                            println!("{}", preview.text());
                            print!("确认删除？[y/N] ");
                            io::stdout().flush()?;
                            read_delete_confirmation(&mut stdin.lock())?
                        }
                        Err(error) => {
                            eprintln!("删除预览失败：{error}");
                            false
                        }
                    }
                };
                if allowed {
                    match session.delete_sessions(&ids).await {
                        Ok(report) => println!("{}", report.text()),
                        Err(error) => eprintln!("会话删除失败：{error}"),
                    }
                } else {
                    println!("已取消删除。");
                }
            }
            Input::Workspace(None) => println!("Workspace: {}", session.workspace()),
            Input::Workspace(Some(path)) => match session.set_workspace(&path).await {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("Workspace 切换失败：{error}"),
            },
            Input::Open(id) => match session.open(&id).await {
                Ok(message) => {
                    println!("{message}");
                    println!("Session ID: {}", session.session_id());
                }
                Err(error) => eprintln!("会话恢复失败：{error}"),
            },
            Input::Unknown(command) => {
                println!("未知命令：{command}（输入 /help 查看可用命令）");
            }
            Input::Message(message) => {
                {
                    let mut stdout = io::stdout().lock();
                    color.write_assistant_prompt(&mut stdout)?;
                    stdout.flush()?;
                }

                let result = session
                    .handle_message(
                        &message,
                        |delta| {
                            let mut stdout = io::stdout().lock();
                            if delta.starts_with("\n[调用工具")
                                || delta.starts_with("\n[工具调用轮次")
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

                println!();
                match result {
                    Ok(()) => {}
                    Err(error) => eprintln!("模型请求失败：{error}"),
                }
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("{}", help_text());
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
    use crate::interaction::{Input, parse_input};

    #[test]
    fn delete_requires_exact_ids_and_explicit_confirmation() {
        let id = "11111111-1111-4111-8111-111111111111";
        assert!(
            matches!(parse_input(&format!("/delete {id} {id}")), Input::Delete { ids, confirmed: false } if ids == vec![id])
        );
        assert!(matches!(
            parse_input(&format!("/delete --yes {id}")),
            Input::Delete {
                confirmed: true,
                ..
            }
        ));
        for line in [
            "/delete",
            "/delete --yes",
            "/delete 11111111",
            "/delete *",
            "/delete --all",
            "/delete title",
        ] {
            assert!(matches!(parse_input(line), Input::Invalid(_)), "{line}");
        }
        for answer in ["\n", "n\n", "maybe\n", "y then delete\n", ""] {
            assert!(!super::read_delete_confirmation(&mut answer.as_bytes()).unwrap());
        }
        assert!(super::read_delete_confirmation(&mut "YES\n".as_bytes()).unwrap());
    }

    #[test]
    fn parses_supported_commands() {
        assert!(matches!(parse_input("/help"), Input::Help));
        assert!(matches!(parse_input("/reset"), Input::Reset));
        assert!(matches!(parse_input("/new"), Input::New));
        assert!(matches!(parse_input("/save"), Input::Save));
        assert!(matches!(parse_input("/compact"), Input::Compact));
        assert!(matches!(
            parse_input("/sessions"),
            Input::Sessions(crate::interaction::SessionScope::Current)
        ));
        assert!(matches!(
            parse_input("/sessions --all"),
            Input::Sessions(crate::interaction::SessionScope::All)
        ));
        assert!(matches!(parse_input("/workspace"), Input::Workspace(None)));
        assert!(
            matches!(parse_input("/workspace ../other"), Input::Workspace(Some(path)) if path == "../other")
        );
        assert!(
            matches!(parse_input("/workspace \"/tmp/foo bar\""), Input::Workspace(Some(path)) if path == "\"/tmp/foo bar\"")
        );
        assert!(matches!(parse_input("/resume abc"), Input::Open(id) if id == "abc"));
        assert!(matches!(parse_input("/open abc"), Input::Open(id) if id == "abc"));
        assert!(matches!(parse_input("/exit"), Input::Exit));
    }

    #[test]
    fn skips_blank_lines_and_reports_unknown_commands() {
        assert!(matches!(parse_input("  \n"), Input::Empty));
        assert!(matches!(parse_input("/other"), Input::Unknown(command) if command == "/other"));
    }

    #[test]
    fn preserves_ordinary_user_input_without_outer_whitespace() {
        assert!(
            matches!(parse_input("  hello model  \n"), Input::Message(message) if message == "hello model")
        );
    }

    #[test]
    fn invalid_utf8_only_discards_its_own_line() {
        let mut input = &b"hello\n\xff\n/exit\n"[..];

        assert!(matches!(
            read_input_line(&mut input).expect("有效输入可读取"),
            InputLine::Line(line) if line == "hello\n"
        ));
        assert!(matches!(
            read_input_line(&mut input).expect("无效输入可读取"),
            InputLine::InvalidUtf8
        ));
        assert!(matches!(
            read_input_line(&mut input).expect("后续输入可读取"),
            InputLine::Line(line) if line == "/exit\n"
        ));
        assert!(matches!(
            read_input_line(&mut input).expect("EOF 可读取"),
            InputLine::Eof
        ));
    }
}
