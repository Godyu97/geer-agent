use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{
    interaction::SessionScope,
    session::{DeletePreview, DeleteReport, SessionEntry},
};

pub(super) enum SessionAction {
    Reload(SessionScope),
    Preview(Vec<String>),
    Delete(Vec<String>),
    Open(String),
    Close,
}

pub(super) struct SessionPanel {
    pub(super) scope: SessionScope,
    pub(super) entries: Vec<SessionEntry>,
    pub(super) selected: HashSet<String>,
    pub(super) cursor: usize,
    pub(super) confirmation: Option<DeletePreview>,
    pub(super) message: String,
    report: Option<DeleteReport>,
    retry_ids: Vec<String>,
    confirmation_scroll: u16,
}

impl SessionPanel {
    pub(super) fn new(scope: SessionScope) -> Self {
        Self {
            scope,
            entries: Vec::new(),
            selected: HashSet::new(),
            cursor: 0,
            confirmation: None,
            message: String::new(),
            report: None,
            retry_ids: Vec::new(),
            confirmation_scroll: 0,
        }
    }

    pub(super) fn refresh(&mut self, entries: Vec<SessionEntry>) {
        let focused = self.entries.get(self.cursor).map(|entry| entry.id.clone());
        self.selected
            .retain(|id| entries.iter().any(|entry| &entry.id == id));
        self.cursor = focused
            .and_then(|id| entries.iter().position(|entry| entry.id == id))
            .unwrap_or(self.cursor)
            .min(entries.len().saturating_sub(1));
        self.entries = entries;
    }

    pub(super) fn confirm(&mut self, preview: DeletePreview) {
        self.report = None;
        self.confirmation = Some(preview);
        self.confirmation_scroll = 0;
    }

    pub(super) fn report(&mut self, report: &DeleteReport) {
        self.message = report.text();
        self.retry_ids = report.retry_ids();
        self.report = Some(report.clone());
        self.confirmation_scroll = 0;
    }

    pub(super) fn key(&mut self, key: KeyEvent) -> Option<SessionAction> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return None;
        }
        if self.confirmation.is_some() {
            match key.code {
                KeyCode::Char('y' | 'Y') => {
                    return self
                        .confirmation
                        .take()
                        .map(|preview| SessionAction::Delete(preview.ids()));
                }
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.confirmation = None;
                    self.message = "已取消删除。".to_owned();
                }
                KeyCode::Up => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_sub(1)
                }
                KeyCode::Down => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_add(1)
                }
                KeyCode::PageUp => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_sub(10)
                }
                KeyCode::PageDown => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_add(10)
                }
                KeyCode::Home => self.confirmation_scroll = 0,
                _ => {}
            }
            return None;
        }
        if self.report.is_some() {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.report = None,
                KeyCode::Char('r') if !self.retry_ids.is_empty() => {
                    return Some(SessionAction::Preview(self.retry_ids.clone()));
                }
                KeyCode::Up => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_sub(1)
                }
                KeyCode::Down => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_add(1)
                }
                KeyCode::PageUp => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_sub(10)
                }
                KeyCode::PageDown => {
                    self.confirmation_scroll = self.confirmation_scroll.saturating_add(10)
                }
                KeyCode::Home => self.confirmation_scroll = 0,
                _ => {}
            }
            return None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::F(3) => return Some(SessionAction::Close),
            KeyCode::Tab => {
                return Some(SessionAction::Reload(match self.scope {
                    SessionScope::Current => SessionScope::All,
                    SessionScope::All => SessionScope::Current,
                }));
            }
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(self.entries.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(10),
            KeyCode::PageDown => {
                self.cursor = (self.cursor + 10).min(self.entries.len().saturating_sub(1))
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.entries.len().saturating_sub(1),
            KeyCode::Char(' ') => {
                if let Some(entry) = self.entries.get(self.cursor)
                    && !self.selected.remove(&entry.id)
                {
                    self.selected.insert(entry.id.clone());
                }
            }
            KeyCode::Char('a') => {
                if self.selected.len() == self.entries.len() {
                    self.selected.clear();
                } else {
                    self.selected = self.entries.iter().map(|entry| entry.id.clone()).collect();
                }
            }
            KeyCode::Delete if !self.selected.is_empty() => {
                return Some(SessionAction::Preview(
                    self.entries
                        .iter()
                        .filter(|entry| self.selected.contains(&entry.id))
                        .map(|entry| entry.id.clone())
                        .collect(),
                ));
            }
            KeyCode::Delete => self.message = "请先用 Space 勾选会话。".to_owned(),
            KeyCode::Char('r') if !self.retry_ids.is_empty() => {
                return Some(SessionAction::Preview(self.retry_ids.clone()));
            }
            KeyCode::Enter => {
                return self
                    .entries
                    .get(self.cursor)
                    .map(|entry| SessionAction::Open(entry.id.clone()));
            }
            _ => {}
        }
        None
    }
}

pub(super) fn render(frame: &mut Frame, panel: &SessionPanel) {
    let area = frame.area();
    let detail = panel
        .confirmation
        .as_ref()
        .map(|preview| {
            (
                preview.text(),
                "删除确认（默认取消）",
                "y 删除 · Enter/Esc 取消",
            )
        })
        .or_else(|| {
            panel.report.as_ref().map(|_| {
                (
                    panel.message.clone(),
                    "删除结果",
                    "Enter/Esc 返回列表 · r 重试",
                )
            })
        });
    if let Some((text, title, hint)) = detail {
        let regions = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(area);
        let line_count = text
            .lines()
            .map(|line| {
                Line::raw(line)
                    .width()
                    .div_ceil(regions[0].width.saturating_sub(2).max(1) as usize)
                    .max(1)
            })
            .sum::<usize>();
        let scroll = panel.confirmation_scroll.min(
            line_count
                .saturating_sub(regions[0].height.saturating_sub(2) as usize)
                .min(u16::MAX as usize) as u16,
        );
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .block(Block::bordered().title(title)),
            regions[0],
        );
        frame.render_widget(
            Paragraph::new(format!("{hint}\n↑↓/PgUp/PgDn 滚动")),
            regions[1],
        );
        return;
    }
    let footer_height = if area.height >= 14 {
        7
    } else if area.height >= 7 {
        4
    } else {
        2
    };
    let regions =
        Layout::vertical([Constraint::Min(1), Constraint::Length(footer_height)]).split(area);
    let scope = if panel.scope == SessionScope::Current {
        "当前"
    } else {
        "全部"
    };
    let items: Vec<_> = panel
        .entries
        .iter()
        .map(|entry| {
            let marker = if panel.selected.contains(&entry.id) {
                "[x]"
            } else {
                "[ ]"
            };
            let active = if entry.active { " *" } else { "" };
            ListItem::new(format!("{marker} {}{active}", entry.title))
        })
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title(format!(
            "会话 {scope} · 已选 {}/{}",
            panel.selected.len(),
            panel.entries.len()
        )))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");
    let mut state =
        ListState::default().with_selected((!panel.entries.is_empty()).then_some(panel.cursor));
    frame.render_stateful_widget(list, regions[0], &mut state);
    let mut footer = vec![
        Line::raw("Space选 a全选 Del删 Tab范围 Esc返回"),
        Line::raw("↑↓/PgUp/Dn移动 Enter打开 r重试"),
    ];
    if let Some(entry) = panel.entries.get(panel.cursor) {
        footer.push(Line::raw(entry.id.clone()));
        footer.push(Line::raw(format!("{} · {}", entry.status, entry.workspace)));
    }
    if !panel.message.is_empty() {
        footer.extend(panel.message.lines().map(|line| Line::raw(line.to_owned())));
    }
    frame.render_widget(
        Paragraph::new(footer).wrap(Wrap { trim: false }),
        regions[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{DeleteItem, DeleteState, DeleteTarget};
    use ratatui::{Terminal, backend::TestBackend};

    fn entry(id: &str, active: bool) -> SessionEntry {
        SessionEntry {
            id: id.into(),
            title: "相同的中文标题🙂".into(),
            updated_at_ms: 0,
            model: "test".into(),
            status: "已保存".into(),
            active,
            uncertain_tools: false,
            workspace: "/tmp/test".into(),
        }
    }

    fn key(panel: &mut SessionPanel, code: KeyCode) -> Option<SessionAction> {
        panel.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn selection_follows_ids_through_reorder_and_all_only_uses_loaded_rows() {
        let mut panel = SessionPanel::new(SessionScope::Current);
        panel.refresh(vec![entry("one", true), entry("two", false)]);
        key(&mut panel, KeyCode::Char(' '));
        panel.refresh(vec![entry("two", false), entry("one", true)]);
        assert_eq!(panel.cursor, 1);
        assert!(panel.selected.contains("one"));
        key(&mut panel, KeyCode::Char('a'));
        assert_eq!(panel.selected.len(), 2);
        assert!(
            matches!(key(&mut panel, KeyCode::Delete), Some(SessionAction::Preview(ids)) if ids == ["two", "one"])
        );
        panel.refresh(vec![entry("two", false)]);
        assert_eq!(panel.selected, HashSet::from(["two".to_owned()]));
        key(&mut panel, KeyCode::Char('a'));
        assert!(panel.selected.is_empty());
        assert!(matches!(
            key(&mut panel, KeyCode::Tab),
            Some(SessionAction::Reload(SessionScope::All))
        ));
        assert!(
            matches!(key(&mut panel, KeyCode::Enter), Some(SessionAction::Open(id)) if id == "two")
        );
        assert!(matches!(
            key(&mut panel, KeyCode::Esc),
            Some(SessionAction::Close)
        ));
    }

    #[test]
    fn confirmation_defaults_to_cancel_freezes_ids_and_reports_retryable_items() {
        let mut panel = SessionPanel::new(SessionScope::Current);
        let preview = DeletePreview {
            targets: vec![DeleteTarget {
                id: "target".into(),
                title: "首句".into(),
                active: true,
            }],
        };
        panel.confirm(preview.clone());
        key(&mut panel, KeyCode::Enter);
        assert!(panel.confirmation.is_none());
        panel.confirm(preview);
        panel.refresh(vec![entry("another", true)]);
        assert!(
            matches!(key(&mut panel, KeyCode::Char('y')), Some(SessionAction::Delete(ids)) if ids == ["target"])
        );
        let report = DeleteReport {
            items: vec![
                DeleteItem {
                    id: "gone".into(),
                    state: DeleteState::Deleted,
                    error: None,
                },
                DeleteItem {
                    id: "retry".into(),
                    state: DeleteState::CleanupPending,
                    error: Some("清理失败".into()),
                },
            ],
            new_session_id: None,
        };
        panel.report(&report);
        assert!(
            matches!(key(&mut panel, KeyCode::Char('r')), Some(SessionAction::Preview(ids)) if ids == ["retry"])
        );
        key(&mut panel, KeyCode::Esc);
        assert!(panel.report.is_none());
    }

    #[test]
    fn manager_confirmation_and_results_render_in_narrow_terminal() {
        let id = "11111111-1111-4111-8111-111111111111";
        let mut panel = SessionPanel::new(SessionScope::Current);
        panel.refresh(vec![entry(id, true)]);
        for size in [(80, 24), (40, 5), (10, 2)] {
            let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
            terminal.draw(|frame| render(frame, &panel)).unwrap();
            panel.confirm(DeletePreview {
                targets: vec![DeleteTarget {
                    id: id.into(),
                    title: "首句".into(),
                    active: true,
                }],
            });
            terminal.draw(|frame| render(frame, &panel)).unwrap();
            key(&mut panel, KeyCode::PageDown);
            terminal.draw(|frame| render(frame, &panel)).unwrap();
            key(&mut panel, KeyCode::Esc);
            panel.report(&DeleteReport {
                items: vec![DeleteItem {
                    id: id.into(),
                    state: DeleteState::Failed,
                    error: Some("存储故障".into()),
                }],
                new_session_id: None,
            });
            terminal.draw(|frame| render(frame, &panel)).unwrap();
            key(&mut panel, KeyCode::Esc);
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| render(frame, &panel)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains(id));
        assert!(rendered.contains("Space"));
    }
}
