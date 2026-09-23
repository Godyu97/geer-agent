use std::{error::Error, io, io::BufRead, io::Write};

use super::color::Color;
use crate::{
    config::Config,
    provider::{ChatProvider, openai::Provider},
};

pub(crate) async fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;
    let mut chat = Provider::new(&config);
    let color = Color::detect();
    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    println!("GeekAgent —— 最简单的 Agent");
    println!("输入 /help 查看命令。\n");

    loop {
        {
            let mut stdout = io::stdout().lock();
            color.write_user_prompt(&mut stdout)?;
            stdout.flush()?;
        }

        let line = match read_input_line(&mut stdin)? {
            InputLine::Line(line) => line,
            InputLine::InvalidUtf8 => {
                eprintln!("输入包含无效 UTF-8，请重新输入。");
                continue;
            }
            InputLine::Eof => {
                println!("bye");
                break;
            }
        };

        match parse_input(&line) {
            Input::Empty => {}
            Input::Help => print_help(),
            Input::Reset => {
                chat.reset();
                println!("（已清空对话记忆）");
            }
            Input::Exit => {
                println!("bye");
                break;
            }
            Input::Unknown(command) => {
                println!("未知命令：{command}（输入 /help 查看可用命令）");
            }
            Input::Message(message) => {
                {
                    let mut stdout = io::stdout().lock();
                    color.write_assistant_prompt(&mut stdout)?;
                    stdout.flush()?;
                }

                let result = chat
                    .stream_reply(&message, |delta| {
                        let mut stdout = io::stdout().lock();
                        color.write_assistant_delta(&mut stdout, delta)?;
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
    println!("可用命令：\n  /help   显示帮助\n  /reset  清空对话记忆\n  /exit   退出程序");
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
