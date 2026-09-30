//! GUI 输入框沿用终端命令语义；流式回调交给窗口桥接层传输。

use std::io;

use crate::interaction::{self, Input, Session, Usage};
use crate::session::{DeletePreview, DeleteReport};

#[derive(Debug, Default)]
pub(crate) struct CommandResult {
    pub(crate) notice: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) delete_confirmation: Option<DeletePreview>,
    pub(crate) delete_report: Option<DeleteReport>,
}

pub(crate) fn tool_progress(delta: &str) -> Option<&str> {
    // 核心一次回调发送完整进度标记；不从模型正文内部查找或删去相似文字。
    let name = delta.strip_prefix("\n[调用工具 ")?.strip_suffix("]\n")?;
    (!name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
    .then_some(name)
}

pub(crate) async fn handle_line<S, F, U>(
    session: &mut S,
    line: &str,
    on_delta: F,
    on_usage: U,
) -> CommandResult
where
    S: Session,
    F: FnMut(&str) -> io::Result<()>,
    U: FnMut(Option<Usage>) -> io::Result<()>,
{
    let mut result = CommandResult::default();
    let mut error = None;
    let notice = match interaction::parse_input(line) {
        Input::Empty => None,
        Input::Invalid(message) => {
            error = Some(message);
            None
        }
        Input::Delete { ids, confirmed } => {
            if confirmed {
                match session.delete_sessions(&ids).await {
                    Ok(report) => {
                        let notice = report.text();
                        result.delete_report = Some(report);
                        result.notice = Some(notice);
                        return result;
                    }
                    Err(failure) => error = Some(format!("会话删除失败：{failure}")),
                }
            } else {
                match session.preview_delete(&ids).await {
                    Ok(preview) => result.delete_confirmation = Some(preview),
                    Err(failure) => error = Some(format!("删除预览失败：{failure}")),
                }
            }
            None
        }
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
        Input::Sessions(scope) => match session.sessions(scope).await {
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
        Input::Workspace(None) => Some(format!("Workspace: {}", session.workspace())),
        Input::Workspace(Some(path)) => match session.set_workspace(&path).await {
            Ok(message) => Some(message),
            Err(failure) => {
                error = Some(format!("Workspace 切换失败：{failure}"));
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
    result.notice = notice;
    result.error = error;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::SessionStatus;
    use std::{cell::RefCell, error::Error, rc::Rc};

    #[test]
    fn identifies_only_complete_tool_progress_callbacks() {
        assert_eq!(tool_progress("\n[调用工具 read]\n"), Some("read"));
        assert_eq!(
            tool_progress("\n[调用工具 get_current_time]\n"),
            Some("get_current_time")
        );
        for text in [
            "正文",
            "[调用工具 read]",
            "说明\n[调用工具 read]\n",
            "\n[调用工具 ]\n",
            "\n[调用工具 read]\n正文\n[调用工具 ls]\n",
        ] {
            assert_eq!(tool_progress(text), None);
        }
    }

    #[derive(Default)]
    struct MockSession {
        seen: Vec<String>,
        fail: bool,
    }

    impl Session for MockSession {
        fn session_id(&self) -> &str {
            "mock-session"
        }

        fn workspace(&self) -> String {
            "/tmp/mock-workspace".into()
        }

        fn status(&self) -> SessionStatus {
            SessionStatus {
                model: "mock".into(),
                session_id: self.session_id().into(),
                session_title: "新会话".into(),
                workspace: self.workspace(),
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

        async fn set_workspace(&mut self, path: &str) -> Result<String, Box<dyn Error>> {
            Ok(format!("已切换 Workspace: {path}"))
        }

        async fn sessions(
            &self,
            _scope: crate::interaction::SessionScope,
        ) -> Result<Vec<String>, Box<dyn Error>> {
            Ok(vec!["mock-session".into()])
        }

        async fn open(&mut self, id: &str) -> Result<String, Box<dyn Error>> {
            Ok(format!("已打开 {id}"))
        }

        async fn session_entries(
            &self,
            _: crate::interaction::SessionScope,
        ) -> Result<Vec<crate::session::SessionEntry>, Box<dyn Error>> {
            Ok(Vec::new())
        }

        async fn preview_delete(&self, ids: &[String]) -> Result<DeletePreview, Box<dyn Error>> {
            Ok(DeletePreview {
                targets: ids
                    .iter()
                    .map(|id| crate::session::DeleteTarget {
                        id: id.clone(),
                        title: "测试会话".into(),
                        active: false,
                    })
                    .collect(),
            })
        }

        async fn delete_sessions(
            &mut self,
            ids: &[String],
        ) -> Result<DeleteReport, Box<dyn Error>> {
            self.seen.extend(ids.iter().cloned());
            Ok(DeleteReport {
                items: ids
                    .iter()
                    .map(|id| crate::session::DeleteItem {
                        id: id.clone(),
                        state: crate::session::DeleteState::Deleted,
                        error: None,
                    })
                    .collect(),
                new_session_id: None,
            })
        }
    }

    #[tokio::test]
    async fn streams_message_callbacks_in_order_and_keeps_command_semantics() {
        let mut session = MockSession::default();
        let events = Rc::new(RefCell::new(Vec::new()));
        let delta_events = Rc::clone(&events);
        let usage_events = Rc::clone(&events);
        let result = handle_line(
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
        .await;
        assert!(result.notice.is_none() && result.error.is_none());
        assert_eq!(session.seen, ["你好"]);
        assert_eq!(*events.borrow(), ["delta:你", "usage:4", "delta:好"]);

        for (command, notice) in [
            ("/open saved", "已打开 saved"),
            ("/save", "已保存"),
            ("/workspace", "Workspace: /tmp/mock-workspace"),
            (
                "/workspace /tmp/other space",
                "已切换 Workspace: /tmp/other space",
            ),
            ("/sessions --all", "mock-session"),
        ] {
            let result = handle_line(&mut session, command, |_| Ok(()), |_| Ok(())).await;
            assert_eq!(result.notice.as_deref(), Some(notice));
            assert!(result.error.is_none());
            assert!(result.delete_confirmation.is_none() && result.delete_report.is_none());
        }
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
        assert!(result.notice.is_none());
        assert_eq!(result.error.as_deref(), Some("模型请求失败：mock failure"));
        assert_eq!(*events.borrow(), ["你", "好"]);
    }

    #[tokio::test]
    async fn delete_preview_never_mutates_and_confirmation_deletes_exact_unique_ids() {
        let id = "11111111-1111-4111-8111-111111111111";
        let mut session = MockSession::default();
        let result = handle_line(
            &mut session,
            &format!("/delete {id} {id}"),
            |_| Ok(()),
            |_| Ok(()),
        )
        .await;
        assert!(session.seen.is_empty());
        assert_eq!(result.delete_confirmation.unwrap().ids(), vec![id]);
        assert!(result.delete_report.is_none());
        let result = handle_line(
            &mut session,
            &format!("/delete --yes {id} {id}"),
            |_| Ok(()),
            |_| Ok(()),
        )
        .await;
        assert_eq!(session.seen, vec![id]);
        assert!(result.delete_confirmation.is_none());
        assert_eq!(result.delete_report.unwrap().items[0].id, id);
        let result = handle_line(&mut session, "/delete --yes wrong", |_| Ok(()), |_| Ok(())).await;
        assert!(result.error.unwrap().contains("完整 UUID"));
        assert_eq!(session.seen, vec![id]);
    }
}
