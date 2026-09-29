//! GUI 输入框沿用终端命令语义；流式回调交给窗口桥接层传输。

use std::io;

use crate::interaction::{self, Input, Session, Usage};

pub(crate) async fn handle_line<S, F, U>(
    session: &mut S,
    line: &str,
    on_delta: F,
    on_usage: U,
) -> (Option<String>, Option<String>)
where
    S: Session,
    F: FnMut(&str) -> io::Result<()>,
    U: FnMut(Option<Usage>) -> io::Result<()>,
{
    let mut error = None;
    let notice = match interaction::parse_input(line) {
        Input::Empty => None,
        Input::Help => Some(interaction::help_text().to_owned()),
        Input::New | Input::Reset => Some(format!(
            "已开始新会话。Session ID: {}",
            session.new_session().await
        )),
        Input::Save => Some(session.flush().await),
        Input::Compact => match session.compact().await {
            Ok(message) => Some(message),
            Err(failure) => {
                error = Some(format!("上下文压缩失败：{failure}"));
                None
            }
        },
        Input::Sessions => match session.sessions().await {
            Ok(items) if items.is_empty() => Some("没有可列出的会话。".to_owned()),
            Ok(items) => Some(items.join("\n")),
            Err(failure) => {
                error = Some(format!("会话列表读取失败：{failure}"));
                None
            }
        },
        Input::Open(id) => match session.open(&id).await {
            Ok(message) => Some(message),
            Err(failure) => {
                error = Some(format!("会话恢复失败：{failure}"));
                None
            }
        },
        Input::Exit => None,
        Input::Unknown(command) => Some(format!("未知命令：{command}（输入 /help 查看可用命令）")),
        Input::Message(message) => {
            if let Err(failure) = session.handle_message(&message, on_delta, on_usage).await {
                error = Some(format!("模型请求失败：{failure}"));
            }
            None
        }
    };
    (notice, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::SessionStatus;
    use std::{cell::RefCell, error::Error, rc::Rc};

    #[derive(Default)]
    struct MockSession {
        seen: Vec<String>,
        fail: bool,
    }

    impl Session for MockSession {
        fn session_id(&self) -> &str {
            "mock-session"
        }

        fn status(&self) -> SessionStatus {
            SessionStatus {
                model: "mock".into(),
                session_id: self.session_id().into(),
                context_tokens: 0,
                context_window_tokens: 100,
                turn_tokens: 0,
                total_tokens: 0,
                usage_complete: true,
            }
        }

        async fn handle_message<F, U>(
            &mut self,
            input: &str,
            mut on_delta: F,
            mut on_usage: U,
        ) -> Result<(), Box<dyn Error>>
        where
            F: FnMut(&str) -> io::Result<()>,
            U: FnMut(Option<Usage>) -> io::Result<()>,
        {
            self.seen.push(input.to_owned());
            on_delta("你")?;
            on_usage(Some(Usage {
                input: 3,
                output: 4,
            }))?;
            on_delta("好")?;
            if self.fail {
                return Err(io::Error::other("mock failure").into());
            }
            Ok(())
        }

        async fn new_session(&mut self) -> String {
            "new-session".into()
        }

        async fn flush(&mut self) -> String {
            "已保存".into()
        }

        async fn compact(&mut self) -> Result<String, Box<dyn Error>> {
            Ok("已压缩".into())
        }

        async fn sessions(&self) -> Result<Vec<String>, Box<dyn Error>> {
            Ok(vec!["mock-session".into()])
        }

        async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>> {
            Ok(format!("已打开 {id}"))
        }
    }

    #[tokio::test]
    async fn streams_message_callbacks_in_order_and_keeps_command_semantics() {
        let mut session = MockSession::default();
        let events = Rc::new(RefCell::new(Vec::new()));
        let delta_events = Rc::clone(&events);
        let usage_events = Rc::clone(&events);
        assert_eq!(
            handle_line(
                &mut session,
                "你好",
                move |text| {
                    delta_events.borrow_mut().push(format!("delta:{text}"));
                    Ok(())
                },
                move |usage| {
                    usage_events
                        .borrow_mut()
                        .push(format!("usage:{}", usage.unwrap().output));
                    Ok(())
                },
            )
            .await,
            (None, None)
        );
        assert_eq!(session.seen, ["你好"]);
        assert_eq!(*events.borrow(), ["delta:你", "usage:4", "delta:好"]);

        assert_eq!(
            handle_line(&mut session, "/open saved", |_| Ok(()), |_| Ok(())).await,
            (Some("已打开 saved".into()), None)
        );
        assert_eq!(
            handle_line(&mut session, "/save", |_| Ok(()), |_| Ok(())).await,
            (Some("已保存".into()), None)
        );
        assert_eq!(session.seen, ["你好"]);
    }

    #[tokio::test]
    async fn reports_failed_message_without_losing_emitted_deltas() {
        let mut session = MockSession {
            fail: true,
            ..MockSession::default()
        };
        let events = Rc::new(RefCell::new(Vec::new()));
        let delta_events = Rc::clone(&events);
        let result = handle_line(
            &mut session,
            "hello",
            move |text| {
                delta_events.borrow_mut().push(text.to_owned());
                Ok(())
            },
            |_| Ok(()),
        )
        .await;
        assert_eq!(result, (None, Some("模型请求失败：mock failure".into())));
        assert_eq!(*events.borrow(), ["你", "好"]);
    }
}
