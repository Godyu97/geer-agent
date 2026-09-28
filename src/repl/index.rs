use std::{error::Error, io, io::BufRead, io::Write};

use super::color::Color;
use crate::prompt::Prompt;

pub(crate) trait Session {
    fn session_id(&self) -> &str;

    async fn handle_message<F>(
        &mut self,
        prompt: &mut Prompt,
        on_delta: F,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(&str) -> io::Result<()>;

    fn reset(&mut self);

    async fn flush(&mut self, prompt: &mut Prompt);

    async fn compact(&mut self, prompt: &mut Prompt) -> Result<String, Box<dyn Error>>;

    async fn sessions(&self) -> Result<Vec<String>, Box<dyn Error>>;

    async fn resume(&mut self, prompt: &mut Prompt, id: &str) -> Result<String, Box<dyn Error>>;
}

pub(crate) async fn run(
    session: &mut impl Session,
    prompt: &mut Prompt,
) -> Result<(), Box<dyn Error>> {
    let color = Color::detect();
    let stdin = io::stdin();

    println!("GeekAgent —— 最简单的 Agent");
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
                session.flush(prompt).await;
                println!("bye");
                break;
            }
        };

        match parse_input(&line) {
            Input::Empty => {}
            Input::Help => print_help(),
            Input::Reset => {
                session.flush(prompt).await;
                prompt.reset();
                session.reset();
                println!("（已清空对话记忆）");
                println!("Session ID: {}", session.session_id());
            }
            Input::Exit => {
                session.flush(prompt).await;
                println!("bye");
                break;
            }
            Input::Compact => match session.compact(prompt).await {
                Ok(message) => println!("{message}"),
                Err(error) => eprintln!("上下文压缩失败：{error}"),
            },
            Input::Sessions => match session.sessions().await {
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
            Input::Resume(id) => {
                session.flush(prompt).await;
                match session.resume(prompt, &id).await {
                    Ok(message) => {
                        println!("{message}");
                        println!("Session ID: {}", session.session_id());
                    }
                    Err(error) => eprintln!("会话恢复失败：{error}"),
                }
            }
            Input::Unknown(command) => {
                println!("未知命令：{command}（输入 /help 查看可用命令）");
            }
            Input::Message(message) => {
                prompt.begin_turn(&message);
                {
                    let mut stdout = io::stdout().lock();
                    color.write_assistant_prompt(&mut stdout)?;
                    stdout.flush()?;
                }

                let result = session
                    .handle_message(prompt, |delta| {
                        let mut stdout = io::stdout().lock();
                        if delta.starts_with("\n[调用工具") || delta.starts_with("\n[工具调用轮次")
                        {
                            color.write_tool_delta(&mut stdout, delta)?;
                        } else {
                            color.write_assistant_delta(&mut stdout, delta)?;
                        }
                        stdout.flush()
                    })
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
    println!(
        "可用命令：\n  /help                 显示帮助\n  /compact              压缩旧对话\n  /sessions             列出当前目录的近期会话\n  /resume <session-id>  恢复会话\n  /reset                开始新会话\n  /exit                 退出程序"
    );
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

enum Input {
    Empty,
    Help,
    Reset,
    Compact,
    Sessions,
    Resume(String),
    Exit,
    Unknown(String),
    Message(String),
}

fn parse_input(line: &str) -> Input {
    let input = line.trim();
    match input {
        "" => Input::Empty,
        "/help" => Input::Help,
        "/reset" => Input::Reset,
        "/compact" => Input::Compact,
        "/sessions" => Input::Sessions,
        input if input.starts_with("/resume ") && !input[8..].trim().is_empty() => {
            Input::Resume(input[8..].trim().to_owned())
        }
        "/exit" => Input::Exit,
        input if input.starts_with('/') => Input::Unknown(input.to_owned()),
        input => Input::Message(input.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Input, InputLine, parse_input, read_input_line};

    #[test]
    fn parses_supported_commands() {
        assert!(matches!(parse_input("/help"), Input::Help));
        assert!(matches!(parse_input("/reset"), Input::Reset));
        assert!(matches!(parse_input("/compact"), Input::Compact));
        assert!(matches!(parse_input("/sessions"), Input::Sessions));
        assert!(matches!(parse_input("/resume abc"), Input::Resume(id) if id == "abc"));
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
