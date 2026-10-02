use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::Line,
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

use super::{clean_text, input::Input};
use crate::{
    interaction::MemoryCommand,
    memory::{MemoryEntry, MemoryPreview, MemoryStatus, search_entries},
};

pub(super) enum MemoryAction {
    Open,
    Close,
    Command(MemoryCommand),
}

enum Mode {
    List,
    Search,
    Add,
    Edit(String),
    Detail(MemoryEntry),
    Confirm(MemoryPreview),
}

pub(super) struct MemoryPanel {
    entries: Vec<MemoryEntry>,
    cursor: usize,
    pub(super) query: String,
    draft: Input,
    mode: Mode,
    scroll: u16,
    pub(super) message: String,
    pub(super) ready: bool,
}

impl Default for MemoryPanel {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            query: String::new(),
            draft: Input::default(),
            mode: Mode::List,
            scroll: 0,
            message: String::new(),
            ready: false,
        }
    }
}

impl MemoryPanel {
    pub(super) fn refresh(&mut self, entries: Vec<MemoryEntry>) {
        let focused = self
            .visible()
            .get(self.cursor)
            .map(|entry| entry.id.clone());
        self.entries = entries;
        let visible = self.visible();
        self.cursor = focused
            .and_then(|id| visible.iter().position(|entry| entry.id == id))
            .unwrap_or(self.cursor)
            .min(visible.len().saturating_sub(1));
    }

    fn visible(&self) -> Vec<MemoryEntry> {
        if self.query.trim().is_empty() {
            self.entries.clone()
        } else {
            search_entries(&self.entries, &self.query)
        }
    }

    pub(super) fn confirm(&mut self, preview: MemoryPreview) {
        self.mode = Mode::Confirm(preview);
        self.scroll = 0;
        self.message.clear();
    }

    pub(super) fn completed(&mut self, message: String) {
        self.mode = Mode::List;
        self.draft.clear();
        self.message = message;
    }

    pub(super) fn paste(&mut self, text: &str) {
        match self.mode {
            Mode::Search => self.draft.paste(text),
            Mode::Add | Mode::Edit(_) => self.draft.paste_multiline(text),
            _ => {}
        }
    }

    pub(super) fn key(&mut self, key: KeyEvent) -> Option<MemoryAction> {
        if matches!(self.mode, Mode::Search | Mode::Add | Mode::Edit(_)) {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.draft.clear();
                    self.message.clear();
                }
                KeyCode::Enter
                    if key.modifiers.contains(KeyModifiers::SHIFT)
                        && !matches!(self.mode, Mode::Search) =>
                {
                    self.draft.newline()
                }
                KeyCode::Enter => match &self.mode {
                    Mode::Search => {
                        self.query = self.draft.text().trim().to_owned();
                        self.cursor = 0;
                        self.mode = Mode::List;
                    }
                    Mode::Add => {
                        return Some(MemoryAction::Command(MemoryCommand::Add(self.draft.text())));
                    }
                    Mode::Edit(id) => {
                        return Some(MemoryAction::Command(MemoryCommand::Edit {
                            id: id.clone(),
                            content: self.draft.text(),
                        }));
                    }
                    _ => {}
                },
                KeyCode::Char(ch)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.draft.insert(ch)
                }
                KeyCode::Backspace => self.draft.backspace(),
                KeyCode::Delete => self.draft.delete(),
                KeyCode::Left => self.draft.left(),
                KeyCode::Right => self.draft.right(),
                KeyCode::Home => self.draft.home(),
                KeyCode::End => self.draft.end(),
                _ => {}
            }
            return None;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return None;
        }
        if let Mode::Confirm(preview) = &self.mode {
            match key.code {
                KeyCode::Char('y' | 'Y') => {
                    return Some(MemoryAction::Command(MemoryCommand::confirm(
                        &preview.action,
                    )));
                }
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.mode = Mode::List;
                    self.message = "已取消记忆操作。".into();
                }
                code => self.scroll_key(code),
            }
            return None;
        }
        if matches!(self.mode, Mode::Detail(_)) {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.mode = Mode::List,
                code => self.scroll_key(code),
            }
            return None;
        }
        let visible = self.visible();
        match key.code {
            KeyCode::Esc | KeyCode::F(4) => return Some(MemoryAction::Close),
            KeyCode::Char('r') => return Some(MemoryAction::Open),
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => self.cursor = (self.cursor + 1).min(visible.len().saturating_sub(1)),
            KeyCode::PageUp => self.cursor = self.cursor.saturating_sub(10),
            KeyCode::PageDown => {
                self.cursor = (self.cursor + 10).min(visible.len().saturating_sub(1))
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = visible.len().saturating_sub(1),
            KeyCode::Enter => {
                if let Some(entry) = visible.get(self.cursor) {
                    self.mode = Mode::Detail(entry.clone());
                    self.scroll = 0;
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.draft.set(&self.query);
            }
            KeyCode::Char('a' | 'e' | 'c') | KeyCode::Delete if !self.ready => {
                self.message = "长期记忆已关闭或不可用；r 重新读取。".into();
            }
            KeyCode::Char('a') => {
                self.mode = Mode::Add;
                self.draft.clear();
                self.message.clear();
            }
            KeyCode::Char('e') => {
                if let Some(entry) = visible.get(self.cursor) {
                    self.mode = Mode::Edit(entry.id.clone());
                    self.draft.set_multiline(&entry.content);
                    self.message.clear();
                }
            }
            KeyCode::Delete => {
                if let Some(entry) = visible.get(self.cursor) {
                    return Some(MemoryAction::Command(MemoryCommand::Delete {
                        id: entry.id.clone(),
                        confirmed: false,
                    }));
                }
            }
            KeyCode::Char('c') => {
                return Some(MemoryAction::Command(MemoryCommand::Clear {
                    confirmed: false,
                }));
            }
            _ => {}
        }
        None
    }

    fn scroll_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
            KeyCode::Home => self.scroll = 0,
            _ => {}
        }
    }
}

pub(super) fn render(frame: &mut Frame, panel: &MemoryPanel, status: &MemoryStatus) {
    let area = frame.area();
    let regions = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(if area.height >= 8 { 4 } else { 1 }),
    ])
    .split(area);
    let (title, detail, hint) = match &panel.mode {
        Mode::Detail(entry) => (
            "记忆全文".to_owned(),
            Some(format!(
                "{}\n创建：{} · 更新：{}\n\n{}",
                entry.id,
                entry.created_at_ms,
                entry.updated_at_ms,
                clean_text(&entry.content)
            )),
            "Enter/Esc 返回 · ↑↓/PgUp/PgDn 滚动",
        ),
        Mode::Confirm(preview) => (
            "记忆操作确认（默认取消）".to_owned(),
            Some(clean_text(&preview.text())),
            "y 确认 · Enter/Esc 取消 · ↑↓ 滚动",
        ),
        Mode::Search => (
            "搜索记忆（多个关键词任意匹配，最多 10 条）".into(),
            None,
            "Enter 搜索 · Esc 取消 · 空输入恢复全部",
        ),
        Mode::Add => (
            "新增记忆".into(),
            None,
            "Enter 保存 · Shift+Enter 换行 · Esc 取消",
        ),
        Mode::Edit(id) => (
            format!("编辑记忆 {id}"),
            None,
            "Enter 保存 · Shift+Enter 换行 · Esc 取消",
        ),
        Mode::List => (
            format!("全局记忆 · {} · {}", status.label(), panel.query),
            None,
            "↑↓ 选择 Enter 全文 / 搜索 a 新增 e 编辑\nDel 删除 c 清空 r 刷新 Esc/F4 返回",
        ),
    };
    let block = Block::bordered().title(title);
    let inner = block.inner(regions[0]);
    if let Some(text) = detail {
        let rows = text
            .lines()
            .map(|line| {
                Line::raw(line)
                    .width()
                    .div_ceil(inner.width.max(1) as usize)
                    .max(1)
            })
            .sum::<usize>();
        let scroll = panel.scroll.min(
            rows.saturating_sub(inner.height as usize)
                .min(u16::MAX as usize) as u16,
        );
        frame.render_widget(
            Paragraph::new(text)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            regions[0],
        );
    } else if matches!(panel.mode, Mode::Search | Mode::Add | Mode::Edit(_)) {
        let (row, column) = panel.draft.position();
        let top = row.saturating_sub(inner.height.saturating_sub(1) as usize);
        let left = column.saturating_sub(inner.width.saturating_sub(1) as usize);
        frame.render_widget(
            Paragraph::new(panel.draft.text()).block(block).scroll((
                top.min(u16::MAX as usize) as u16,
                left.min(u16::MAX as usize) as u16,
            )),
            regions[0],
        );
        if inner.width > 0 && inner.height > 0 {
            frame.set_cursor_position(Position::new(
                inner.x + (column - left) as u16,
                inner.y + (row - top) as u16,
            ));
        }
    } else {
        let visible = panel.visible();
        let items = visible
            .iter()
            .map(|entry| {
                ListItem::new(format!(
                    "{}\n{}",
                    entry.id,
                    clean_text(&entry.content)
                        .lines()
                        .next()
                        .unwrap_or_default()
                ))
            })
            .collect::<Vec<_>>();
        let mut selection =
            ListState::default().with_selected((!visible.is_empty()).then_some(panel.cursor));
        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new(if panel.ready {
                    "没有匹配的记忆。"
                } else {
                    "长期记忆已关闭或不可用。"
                })
                .block(block),
                regions[0],
            );
        } else {
            frame.render_stateful_widget(
                List::new(items)
                    .block(block)
                    .highlight_style(Style::default().bg(Color::DarkGray))
                    .highlight_symbol("> "),
                regions[0],
                &mut selection,
            );
        }
    }
    let error = status.error.as_deref().unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!("{hint}\n{}\n{error}", panel.message)).wrap(Wrap { trim: false }),
        regions[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryAction as PreviewAction;
    use ratatui::{Terminal, backend::TestBackend};

    fn entry() -> MemoryEntry {
        MemoryEntry {
            id: "11111111-1111-4111-8111-111111111111".into(),
            content: "中文 preference\n完整第二行".into(),
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }

    fn key(panel: &mut MemoryPanel, code: KeyCode) -> Option<MemoryAction> {
        panel.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn memory_panel_search_editor_and_default_cancel_preserve_full_content() {
        let mut panel = MemoryPanel {
            ready: true,
            ..MemoryPanel::default()
        };
        panel.refresh(vec![entry()]);
        key(&mut panel, KeyCode::Char('/'));
        panel.paste("PREFERENCE absent");
        key(&mut panel, KeyCode::Enter);
        assert_eq!(panel.visible().len(), 1);
        key(&mut panel, KeyCode::Char('e'));
        assert_eq!(panel.draft.text(), entry().content);
        panel.paste("\n增加");
        assert!(
            matches!(key(&mut panel, KeyCode::Enter), Some(MemoryAction::Command(MemoryCommand::Edit { content, .. })) if content.ends_with("\n增加"))
        );
        panel.message = "数据库失败".into();
        assert!(panel.draft.text().ends_with("\n增加"));
        key(&mut panel, KeyCode::Esc);
        panel.confirm(MemoryPreview {
            action: PreviewAction::Clear,
            entries: vec![],
            count: 1,
        });
        assert!(key(&mut panel, KeyCode::Enter).is_none());
        assert!(matches!(panel.mode, Mode::List));
        panel.confirm(MemoryPreview {
            action: PreviewAction::Delete { id: entry().id },
            entries: vec![entry()],
            count: 1,
        });
        assert!(
            matches!(key(&mut panel, KeyCode::Char('y')), Some(MemoryAction::Command(MemoryCommand::Delete { confirmed: true, id })) if id == entry().id)
        );
    }

    #[test]
    fn memory_panel_detail_and_confirm_render_full_ids_and_narrow_sizes() {
        let mut panel = MemoryPanel {
            ready: true,
            ..MemoryPanel::default()
        };
        panel.refresh(vec![entry()]);
        key(&mut panel, KeyCode::Enter);
        let status = MemoryStatus {
            state: crate::memory::MemoryState::Ready,
            count: Some(1),
            error: None,
        };
        let mut full = Terminal::new(TestBackend::new(80, 20)).unwrap();
        full.draw(|frame| render(frame, &panel, &status)).unwrap();
        let text: String = full
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .filter(|symbol| *symbol != " ")
            .collect();
        assert!(text.contains(&entry().id));
        assert!(text.contains("完整第二行"));
        for (width, height) in [(40, 5), (10, 2), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render(frame, &panel, &status))
                .unwrap();
            panel.confirm(MemoryPreview {
                action: PreviewAction::Clear,
                entries: vec![],
                count: 1,
            });
            terminal
                .draw(|frame| render(frame, &panel, &status))
                .unwrap();
            key(&mut panel, KeyCode::Esc);
            key(&mut panel, KeyCode::Char('e'));
            terminal
                .draw(|frame| render(frame, &panel, &status))
                .unwrap();
            key(&mut panel, KeyCode::Esc);
        }
    }
}
