//! 图形界面 输入框沿用终端命令语义；流式回调交给窗口桥接层传输。

use std::io;

use crate::interaction::{self, CommandOutcome, Session, Usage};
use crate::session::{DeletePreview, DeleteReport};

#[derive(Debug, Default)]
pub(crate) struct CommandResult {
    pub(crate) notice: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) delete_confirmation: Option<DeletePreview>,
    pub(crate) delete_report: Option<DeleteReport>,
    pub(crate) memory_confirmation: Option<crate::memory::MemoryPreview>,
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
    result.notice =
        match interaction::execute(session, interaction::parse_input(line), on_delta, on_usage)
            .await
        {
            Err(error) => {
                result.error = Some(crate::ui::commands::error_text(&error));
                None
            }
            Ok(outcome) => match outcome {
                CommandOutcome::Empty | CommandOutcome::Message | CommandOutcome::Exit => None,
                CommandOutcome::Memories { entries, query } => {
                    Some(crate::memory::list_text(&entries, query.is_some()))
                }
                CommandOutcome::MemoryChanged(message) => Some(message),
                CommandOutcome::MemoryPreview(preview) => {
                    result.memory_confirmation = Some(preview);
                    None
                }
                CommandOutcome::Help => Some(interaction::help_text().to_owned()),
                CommandOutcome::NewSession { session_id, .. } => {
                    Some(format!("已开始新会话。Session ID: {session_id}"))
                }
                CommandOutcome::Saved(message)
                | CommandOutcome::Compacted(message)
                | CommandOutcome::WorkspaceChanged(message)
                | CommandOutcome::Opened { message, .. } => Some(message),
                CommandOutcome::Sessions(items) if items.is_empty() => {
                    Some("没有可列出的会话。".to_owned())
                }
                CommandOutcome::Sessions(items) => Some(items.join("\n")),
                CommandOutcome::Workspace(path) => Some(format!("Workspace: {path}")),
                CommandOutcome::DeletePreview(preview) => {
                    result.delete_confirmation = Some(preview);
                    None
                }
                CommandOutcome::Deleted(report) => {
                    let message = report.text();
                    result.delete_report = Some(report);
                    Some(message)
                }
                CommandOutcome::Unknown(command) => {
                    Some(format!("未知命令：{command}（输入 /help 查看可用命令）"))
                }
            },
        };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::test_support::MockSession;
    use std::{cell::RefCell, rc::Rc};

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
