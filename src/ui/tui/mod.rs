//! Ratatui 终端界面。只通过界面无关契约使用会话能力。

mod input;
mod memory;
mod sessions;

use std::{cell::RefCell, error::Error, io, rc::Rc};

use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Paragraph},
};

use super::commands::error_text;
use crate::interaction::{
    self, CommandOutcome, DiagnosticBuffer, Input as Command, Session, SessionScope, SessionStatus,
    Usage,
};
use input::Input;
use memory::{MemoryAction, MemoryPanel};
use sessions::{SessionAction, SessionPanel};

const PANEL_WIDTH: u16 = 28;
const MIN_PANEL_WIDTH: u16 = 60;

pub(super) struct Tui {
    screen: Rc<RefCell<Screen>>,
    diagnostics: DiagnosticBuffer,
    restored: bool,
}

struct Screen {
    terminal: DefaultTerminal,
    state: State,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    User,
    Assistant,
    Tool,
    System,
}

impl Kind {
    fn style(self) -> Style {
        Style::default().fg(match self {
            Self::User => Color::Cyan,
            Self::Assistant => Color::Green,
            Self::Tool => Color::Yellow,
            Self::System => Color::Gray,
        })
    }
}

struct Entry {
    kind: Kind,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Chat,
    Confirm,
    Workspace,
    Sessions,
    Memories,
}

struct State {
    entries: Vec<Entry>,
    input: Input,
    workspace_input: Input,
    mode: Mode,
    status: SessionStatus,
    estimating: bool,
    live_chars: u64,
    scroll_back: usize,
    exit_requested: bool,
    sessions: SessionPanel,
    memories: MemoryPanel,
}

enum Action {
    Submit(String),
    Exit,
    Session(SessionAction),
    Memory(MemoryAction),
}

impl Tui {
    pub(super) fn new() -> io::Result<Self> {
        let terminal = ratatui::try_init()?;
        if let Err(error) = execute!(io::stdout(), EnableBracketedPaste) {
            let _ = ratatui::try_restore();
            return Err(error);
        }
        let diagnostics = DiagnosticBuffer::start();
        Ok(Self {
            screen: Rc::new(RefCell::new(Screen {
                terminal,
                state: State {
                    entries: Vec::new(),
                    input: Input::default(),
                    workspace_input: Input::default(),
                    mode: Mode::Chat,
                    status: SessionStatus {
                        model: String::new(),
                        session_id: String::new(),
                        session_title: "新会话".to_owned(),
                        workspace: String::new(),
                        context_tokens: 0,
                        context_window_tokens: 1,
                        turn_tokens: 0,
                        total_tokens: 0,
                        usage_complete: true,
                        instructions_loaded: false,
                        memory: crate::memory::MemoryService::disabled().status(),
                    },
                    estimating: false,
                    live_chars: 0,
                    scroll_back: 0,
                    exit_requested: false,
                    sessions: SessionPanel::new(SessionScope::Current),
                    memories: MemoryPanel::default(),
                },
            })),
            diagnostics,
            restored: false,
        })
    }

    pub(super) fn confirmer(&self) -> impl FnMut(&str) -> io::Result<bool> + 'static {
        let screen = Rc::clone(&self.screen);
        move |prompt| screen.borrow_mut().confirm(prompt)
    }

    pub(super) async fn run(&mut self, session: &mut impl Session) -> Result<(), Box<dyn Error>> {
        {
            let mut screen = self.screen.borrow_mut();
            screen.state.sync_session(session.status());
            screen.state.append(Kind::System, "GeekAgent —— 轻量 TUI");
            screen.state.append(
                Kind::System,
                "输入 /help 查看命令；PageUp/PageDown 滚动消息。",
            );
            screen.draw()?;
        }

        loop {
            let action = self.screen.borrow_mut().read_action()?;
            match action {
                Action::Exit => break,
                Action::Session(action) => self.session_action(session, action).await?,
                Action::Memory(action) => self.memory_action(session, action).await?,
                Action::Submit(line) => {
                    if self.handle_line(session, &line).await? {
                        break;
                    }
                }
            }
            if self.screen.borrow().state.exit_requested {
                break;
            }
        }

        let report = interaction::save(session).await;
        self.restore()?;
        if !report.is_empty() {
            println!("{report}");
        }
        println!("bye");
        Ok(())
    }

    async fn handle_line(
        &mut self,
        session: &mut impl Session,
        line: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let command = interaction::parse_input(line);
        if matches!(command, Command::Empty) {
            return Ok(false);
        }
        {
            let mut screen = self.screen.borrow_mut();
            screen
                .state
                .append(Kind::User, format!("你 › {}", line.trim()));
            screen.draw()?;
        }

        let command = match command {
            Command::Memory(command) => {
                self.screen.borrow_mut().state.mode = Mode::Memories;
                self.memory_action(session, MemoryAction::Command(command))
                    .await?;
                return Ok(false);
            }
            Command::Sessions(scope) => {
                self.show_sessions(session, scope).await?;
                return Ok(false);
            }
            Command::Delete { ids, confirmed } => {
                if self.screen.borrow().state.mode != Mode::Sessions {
                    self.show_sessions(session, SessionScope::Current).await?;
                }
                self.session_action(
                    session,
                    if confirmed {
                        SessionAction::Delete(ids)
                    } else {
                        SessionAction::Preview(ids)
                    },
                )
                .await?;
                return Ok(false);
            }
            command => command,
        };
        let streaming = matches!(command, Command::Message(_));
        if streaming {
            let mut screen = self.screen.borrow_mut();
            screen.state.status.turn_tokens = 0;
            screen.state.estimating = true;
            screen.state.live_chars = 0;
            screen.draw()?;
        }
        let output = Rc::clone(&self.screen);
        let usage_output = Rc::clone(&self.screen);
        let result = interaction::execute(
            session,
            command,
            move |delta| output.borrow_mut().delta(delta),
            move |usage| usage_output.borrow_mut().usage(usage),
        )
        .await;
        let mut screen = self.screen.borrow_mut();
        if streaming {
            screen.state.estimating = false;
            screen.state.live_chars = 0;
        }
        let message = match result {
            Err(error) => Some(error_text(&error)),
            Ok(CommandOutcome::Empty | CommandOutcome::Message) => None,
            Ok(CommandOutcome::Memories { entries, query }) => {
                Some(crate::memory::list_text(&entries, query.is_some()))
            }
            Ok(CommandOutcome::MemoryChanged(message)) => Some(message),
            Ok(CommandOutcome::MemoryPreview(preview)) => Some(preview.text()),
            Ok(CommandOutcome::Exit) => return Ok(true),
            Ok(CommandOutcome::Help) => Some(interaction::help_text().to_owned()),
            Ok(CommandOutcome::NewSession { session_id, .. }) => {
                Some(format!("已开始新会话。\nSession ID: {session_id}"))
            }
            Ok(CommandOutcome::Saved(message) | CommandOutcome::Compacted(message)) => {
                Some(message)
            }
            Ok(CommandOutcome::Opened {
                message,
                session_id,
            }) => Some(format!("{message}\nSession ID: {session_id}")),
            Ok(CommandOutcome::Workspace(path)) => Some(format!("Workspace: {path}")),
            Ok(CommandOutcome::WorkspaceChanged(message)) => {
                screen.state.mode = Mode::Chat;
                screen.state.workspace_input.clear();
                Some(message)
            }
            Ok(CommandOutcome::Unknown(command)) => {
                Some(format!("未知命令：{command}（输入 /help 查看可用命令）"))
            }
            Ok(CommandOutcome::Sessions(items)) => Some(items.join("\n")),
            Ok(CommandOutcome::DeletePreview(preview)) => {
                screen.state.sessions.confirm(preview);
                None
            }
            Ok(CommandOutcome::Deleted(report)) => Some(report.text()),
        };
        screen.state.sync_session(session.status());
        if let Some(message) = message {
            screen.state.append(Kind::System, message);
        }
        screen.draw()?;
        Ok(false)
    }

    fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        let paste = execute!(io::stdout(), DisableBracketedPaste);
        let terminal = ratatui::try_restore();
        self.diagnostics.finish();
        paste.and(terminal)
    }

    async fn show_memories(&mut self, session: &impl Session) -> io::Result<()> {
        let entries = session.memories().await;
        let mut screen = self.screen.borrow_mut();
        screen.state.mode = Mode::Memories;
        screen.state.memories.ready =
            session.status().memory.state == crate::memory::MemoryState::Ready;
        match entries {
            Ok(entries) => screen.state.memories.refresh(entries),
            Err(error) => screen.state.memories.message = format!("记忆读取失败：{error}"),
        }
        screen.state.sync_session(session.status());
        screen.draw()
    }

    async fn memory_action(
        &mut self,
        session: &mut impl Session,
        action: MemoryAction,
    ) -> io::Result<()> {
        let command = match action {
            MemoryAction::Open => return self.show_memories(session).await,
            MemoryAction::Close => {
                let mut screen = self.screen.borrow_mut();
                screen.state.mode = Mode::Chat;
                screen.state.memories = MemoryPanel::default();
                return screen.draw();
            }
            MemoryAction::Command(command) => command,
        };
        let result =
            interaction::execute(session, Command::Memory(command), |_| Ok(()), |_| Ok(())).await;
        match result {
            Ok(CommandOutcome::MemoryPreview(preview)) => {
                self.screen.borrow_mut().state.memories.confirm(preview)
            }
            Ok(CommandOutcome::MemoryChanged(message)) => {
                self.screen.borrow_mut().state.memories.completed(message);
                return self.show_memories(session).await;
            }
            Ok(CommandOutcome::Memories { query, .. }) => {
                self.screen.borrow_mut().state.memories.query = query.unwrap_or_default();
                return self.show_memories(session).await;
            }
            Err(error) => self.screen.borrow_mut().state.memories.message = error_text(&error),
            Ok(_) => return Err(io::Error::other("记忆面板收到不支持的操作结果")),
        }
        let mut screen = self.screen.borrow_mut();
        screen.state.memories.ready =
            session.status().memory.state == crate::memory::MemoryState::Ready;
        screen.state.sync_session(session.status());
        screen.draw()
    }

    async fn show_sessions(
        &mut self,
        session: &impl Session,
        scope: SessionScope,
    ) -> io::Result<()> {
        let entries = session.session_entries(scope).await;
        let mut screen = self.screen.borrow_mut();
        screen.state.sessions = SessionPanel::new(scope);
        screen.state.mode = Mode::Sessions;
        match entries {
            Ok(entries) => screen.state.sessions.refresh(entries),
            Err(error) => screen.state.sessions.message = format!("会话列表读取失败：{error}"),
        }
        screen.draw()
    }

    async fn session_action(
        &mut self,
        session: &mut impl Session,
        action: SessionAction,
    ) -> io::Result<()> {
        let command = match action {
            SessionAction::Reload(scope) => return self.show_sessions(session, scope).await,
            SessionAction::Close => {
                let mut screen = self.screen.borrow_mut();
                screen.state.mode = Mode::Chat;
                screen.state.sessions = SessionPanel::new(SessionScope::Current);
                screen.state.sync_session(session.status());
                return screen.draw();
            }
            SessionAction::Preview(ids) => Command::Delete {
                ids,
                confirmed: false,
            },
            SessionAction::Delete(ids) => Command::Delete {
                ids,
                confirmed: true,
            },
            SessionAction::Open(id) => Command::Open(id),
        };
        match interaction::execute(session, command, |_| Ok(()), |_| Ok(())).await {
            Ok(CommandOutcome::DeletePreview(preview)) => {
                self.screen.borrow_mut().state.sessions.confirm(preview);
            }
            Ok(CommandOutcome::Deleted(report)) => {
                let scope = self.screen.borrow().state.sessions.scope;
                let entries = session.session_entries(scope).await;
                let mut screen = self.screen.borrow_mut();
                screen.state.sync_session(session.status());
                screen.state.append(Kind::System, report.text());
                screen.state.sessions.report(&report);
                match entries {
                    Ok(entries) => screen.state.sessions.refresh(entries),
                    Err(error) => screen
                        .state
                        .sessions
                        .message
                        .push_str(&format!("\n列表刷新失败：{error}")),
                }
            }
            Ok(CommandOutcome::Opened { message, .. }) => {
                let mut screen = self.screen.borrow_mut();
                screen.state.sync_session(session.status());
                screen.state.sessions = SessionPanel::new(SessionScope::Current);
                screen.state.mode = Mode::Chat;
                screen.state.append(Kind::System, message);
            }
            Err(error) => {
                self.screen.borrow_mut().state.sessions.message = error_text(&error);
            }
            Ok(_) => return Err(io::Error::other("会话面板收到不支持的操作结果")),
        }
        let mut screen = self.screen.borrow_mut();
        screen.state.sync_session(session.status());
        screen.draw()
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

impl Screen {
    fn draw(&mut self) -> io::Result<()> {
        let state = &self.state;
        self.terminal.draw(|frame| render(frame, state))?;
        Ok(())
    }

    fn delta(&mut self, delta: &str) -> io::Result<()> {
        self.state.append_delta(delta);
        self.draw()
    }

    fn usage(&mut self, usage: Option<Usage>) -> io::Result<()> {
        self.state.apply_usage(usage);
        self.draw()
    }

    fn confirm(&mut self, prompt: &str) -> io::Result<bool> {
        self.state.mode = Mode::Confirm;
        self.state.input.clear();
        self.state.append(Kind::Tool, prompt.trim());
        self.draw()?;
        let action = self.read_action();
        self.state.mode = Mode::Chat;
        let answer = match action? {
            Action::Submit(answer) => answer,
            Action::Exit => {
                self.state.exit_requested = true;
                String::new()
            }
            Action::Session(_) | Action::Memory(_) => String::new(),
        };
        let allowed = authorization_allowed(&answer);
        self.state.append(
            Kind::Tool,
            format!("授权选择：{}", if allowed { "y" } else { "n（拒绝）" }),
        );
        self.draw()?;
        Ok(allowed)
    }

    fn read_action(&mut self) -> io::Result<Action> {
        loop {
            let event = event::read()?;
            let action = self.state.handle_event(event);
            self.draw()?;
            if let Some(action) = action {
                return Ok(action);
            }
        }
    }
}

impl State {
    fn sync_session(&mut self, status: SessionStatus) {
        if self.status.session_id != status.session_id || self.status.workspace != status.workspace
        {
            self.entries.clear();
            self.input.clear();
            self.workspace_input.clear();
            self.scroll_back = 0;
            self.estimating = false;
            self.live_chars = 0;
        }
        self.status = status;
    }

    fn append(&mut self, kind: Kind, text: impl AsRef<str>) {
        self.entries.push(Entry {
            kind,
            text: clean_text(text.as_ref()),
        });
    }

    fn append_delta(&mut self, delta: &str) {
        if is_progress_delta(delta) {
            self.append(Kind::Tool, delta.trim_matches('\n'));
            return;
        }
        self.estimating = true;
        let clean = clean_text(delta);
        self.live_chars = self.live_chars.saturating_add(clean.chars().count() as u64);
        if let Some(last) = self.entries.last_mut()
            && last.kind == Kind::Assistant
        {
            last.text.push_str(&clean);
        } else {
            self.append(Kind::Assistant, format!("助手 › {clean}"));
        }
    }

    fn apply_usage(&mut self, usage: Option<Usage>) {
        self.estimating = false;
        self.live_chars = 0;
        if let Some(usage) = usage {
            let tokens = usage.input.saturating_add(usage.output);
            self.status.turn_tokens = self.status.turn_tokens.saturating_add(tokens);
            self.status.total_tokens = self.status.total_tokens.saturating_add(tokens);
        } else {
            self.status.usage_complete = false;
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'd'))
                {
                    return Some(Action::Exit);
                }
                if self.mode == Mode::Sessions {
                    return self.sessions.key(key).map(Action::Session);
                }
                if self.mode == Mode::Memories {
                    return self.memories.key(key).map(Action::Memory);
                }
                match key.code {
                    KeyCode::F(4) if self.mode == Mode::Chat => {
                        return Some(Action::Memory(MemoryAction::Open));
                    }
                    KeyCode::F(3) if self.mode == Mode::Chat => {
                        return Some(Action::Session(SessionAction::Reload(
                            SessionScope::Current,
                        )));
                    }
                    KeyCode::F(2) if self.mode == Mode::Chat => {
                        self.workspace_input.set(&self.status.workspace);
                        self.mode = Mode::Workspace;
                    }
                    KeyCode::Enter if self.mode == Mode::Workspace => {
                        let path = self.workspace_input.text();
                        return Some(Action::Submit(if path.trim().is_empty() {
                            "/workspace \"\"".to_owned()
                        } else {
                            format!("/workspace {path}")
                        }));
                    }
                    KeyCode::Enter => return Some(Action::Submit(self.input.take())),
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.active_input_mut().insert(ch);
                    }
                    KeyCode::Backspace => self.active_input_mut().backspace(),
                    KeyCode::Delete => self.active_input_mut().delete(),
                    KeyCode::Left => self.active_input_mut().left(),
                    KeyCode::Right => self.active_input_mut().right(),
                    KeyCode::Home => self.active_input_mut().home(),
                    KeyCode::End => self.active_input_mut().end(),
                    KeyCode::PageUp => self.scroll_back = self.scroll_back.saturating_add(10),
                    KeyCode::PageDown => self.scroll_back = self.scroll_back.saturating_sub(10),
                    KeyCode::Up => self.scroll_back = self.scroll_back.saturating_add(1),
                    KeyCode::Down => self.scroll_back = self.scroll_back.saturating_sub(1),
                    KeyCode::Esc if self.mode == Mode::Confirm => {
                        return Some(Action::Submit(String::new()));
                    }
                    KeyCode::Esc if self.mode == Mode::Workspace => {
                        self.workspace_input.clear();
                        self.mode = Mode::Chat;
                    }
                    KeyCode::Esc => {
                        self.input.clear();
                        self.scroll_back = 0;
                    }
                    _ => {}
                }
            }
            Event::Paste(value) if self.mode == Mode::Memories => self.memories.paste(&value),
            Event::Paste(value) if self.mode != Mode::Sessions => {
                self.active_input_mut().paste(&value)
            }
            _ => {}
        }
        None
    }

    fn active_input_mut(&mut self) -> &mut Input {
        if self.mode == Mode::Workspace {
            &mut self.workspace_input
        } else {
            &mut self.input
        }
    }
}

fn clean_text(text: &str) -> String {
    text.chars()
        .filter_map(|ch| match ch {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect()
}

fn is_progress_delta(delta: &str) -> bool {
    [
        "\n[调用工具",
        "\n[工具调用轮次",
        "\n[上下文压缩",
        "\n[上下文窗口溢出后压缩",
        "\n[工具执行预算已耗尽",
    ]
    .iter()
    .any(|prefix| delta.starts_with(prefix))
}

fn authorization_allowed(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn render(frame: &mut Frame, state: &State) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    if state.mode == Mode::Sessions {
        sessions::render(frame, &state.sessions);
        return;
    }
    if state.mode == Mode::Memories {
        memory::render(frame, &state.memories, &state.status.memory);
        return;
    }
    let input_height = if area.height >= 4 { 3 } else { 1 };
    let sections =
        Layout::vertical([Constraint::Min(0), Constraint::Length(input_height)]).split(area);
    let body = sections[0];
    let input_area = sections[1];
    if body.height > 0 {
        if area.width >= MIN_PANEL_WIDTH {
            let columns = Layout::horizontal([Constraint::Min(1), Constraint::Length(PANEL_WIDTH)])
                .split(body);
            render_messages(frame, state, columns[0]);
            render_panel(frame, state, columns[1]);
        } else {
            render_messages(frame, state, body);
        }
    }
    render_input(frame, state, input_area);
}

fn render_messages(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::bordered().title("对话");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let rows = visual_lines(&state.entries, inner.width as usize);
    let visible = inner.height as usize;
    let bottom = rows.len().saturating_sub(visible);
    let first = bottom.saturating_sub(state.scroll_back);
    let text = Text::from(
        rows.into_iter()
            .skip(first)
            .take(visible)
            .collect::<Vec<_>>(),
    );
    frame.render_widget(Paragraph::new(text), inner);
}

fn visual_lines(entries: &[Entry], width: usize) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    let width = width.max(1);
    for entry in entries {
        for raw in entry.text.split('\n') {
            let line = Line::raw(raw);
            let mut segment = String::new();
            let mut used = 0;
            for grapheme in line.styled_graphemes(Style::default()) {
                let columns = Line::raw(grapheme.symbol).width();
                if used + columns > width && !segment.is_empty() {
                    output.push(Line::styled(
                        std::mem::take(&mut segment),
                        entry.kind.style(),
                    ));
                    used = 0;
                }
                if columns > width {
                    segment.push('?');
                    used = 1;
                } else {
                    segment.push_str(grapheme.symbol);
                    used += columns;
                }
            }
            output.push(Line::styled(segment, entry.kind.style()));
        }
        output.push(Line::raw(""));
    }
    output
}

fn render_panel(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::bordered().title("用量");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let status = &state.status;
    let context = status.context_tokens;
    let window = status.context_window_tokens.max(1);
    let percent = ((u128::from(context) * 100).div_ceil(u128::from(window))).min(9999);
    let live = if state.estimating {
        state.live_chars.div_ceil(2)
    } else {
        0
    };
    let turn = if state.estimating {
        format!("~{}", status.turn_tokens.saturating_add(live))
    } else {
        status.turn_tokens.to_string()
    };
    let total = if state.estimating {
        format!("~{}", status.total_tokens.saturating_add(live))
    } else {
        status.total_tokens.to_string()
    };
    let lines = vec![
        Line::raw(format!("模型  {}", status.model)),
        Line::raw(format!("会话  {}", status.session_id)),
        Line::raw(format!("标题  {}", status.session_title)),
        Line::raw(format!("目录  {}（F2）", status.workspace)),
        Line::raw(format!(
            "指令  {}",
            if status.instructions_loaded {
                "AGENTS.md 已加载"
            } else {
                "无项目指令"
            }
        )),
        Line::raw(format!("记忆  {}（F4）", status.memory.label())),
        Line::raw("── 上下文（估算）──"),
        Line::raw(format!("{context} / {window}")),
        Line::raw(format!("{percent}% used")),
        Line::raw("── 本轮 ──"),
        Line::raw(format!("{turn} tokens")),
        Line::raw("── 累计 ──"),
        Line::raw(format!("{total} tokens")),
        Line::raw(if status.usage_complete {
            "已报告用量"
        } else {
            "用量部分缺失"
        }),
        Line::raw(if state.estimating {
            "~ 表示流式估算"
        } else {
            ""
        }),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_input(frame: &mut Frame, state: &State, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let title = match state.mode {
        Mode::Confirm => "授权确认",
        Mode::Workspace => "Workspace · Enter 切换 · Esc 取消",
        Mode::Chat => "输入 · F2 Workspace · F3 会话 · F4 记忆",
        Mode::Sessions => "会话管理",
        Mode::Memories => "记忆管理",
    };
    let block = Block::bordered().title(title);
    let inner = if area.height >= 3 {
        block.inner(area)
    } else {
        area
    };
    if area.height >= 3 {
        frame.render_widget(block, area);
    }
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let full_prompt = match state.mode {
        Mode::Confirm => "[y/N] ",
        Mode::Workspace => "路径 › ",
        Mode::Chat => "你 › ",
        Mode::Sessions => "",
        Mode::Memories => "",
    };
    let prompt = if Line::raw(full_prompt).width() < inner.width as usize {
        full_prompt
    } else {
        "› "
    };
    let prompt_width = Line::raw(prompt).width().min(inner.width as usize);
    let available = (inner.width as usize).saturating_sub(prompt_width);
    let input = if state.mode == Mode::Workspace {
        &state.workspace_input
    } else {
        &state.input
    };
    let (visible, cursor) = input.visible(available);
    frame.render_widget(Paragraph::new(format!("{prompt}{visible}")), inner);
    let x = inner.x + (prompt_width + cursor).min((inner.width - 1) as usize) as u16;
    frame.set_cursor_position(Position::new(x, inner.y));
}

#[cfg(test)]
mod tests {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::{
        Action, Kind, Mode, SessionStatus, State, Usage, authorization_allowed, input::Input,
        render,
    };

    fn state() -> State {
        State {
            entries: Vec::new(),
            input: Input::default(),
            workspace_input: Input::default(),
            mode: Mode::Chat,
            status: SessionStatus {
                model: "test-model".into(),
                session_id: "test-session".into(),
                session_title: "新会话".into(),
                workspace: "/tmp/test-workspace".into(),
                context_tokens: 50,
                context_window_tokens: 100,
                turn_tokens: 3,
                total_tokens: 7,
                usage_complete: true,
                instructions_loaded: false,
                memory: crate::memory::MemoryService::disabled().status(),
            },
            estimating: false,
            live_chars: 0,
            scroll_back: 0,
            exit_requested: false,
            sessions: super::SessionPanel::new(crate::interaction::SessionScope::Current),
            memories: super::MemoryPanel::default(),
        }
    }

    fn rendered_text(buffer: &Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .filter(|symbol| *symbol != " ")
            .collect()
    }

    #[test]
    fn wide_panel_survives_messages_and_narrow_view_keeps_input() {
        let mut state = state();
        state.append(Kind::Assistant, "你好，模型回答。".repeat(20));
        let mut wide = Terminal::new(TestBackend::new(80, 20)).unwrap();
        wide.draw(|frame| render(frame, &state)).unwrap();
        let wide_text = rendered_text(wide.backend().buffer());
        assert!(wide_text.contains("上下文"));
        assert!(wide_text.contains("test-model"));
        assert!(wide_text.contains("50/100"));

        let mut narrow = Terminal::new(TestBackend::new(40, 10)).unwrap();
        narrow.draw(|frame| render(frame, &state)).unwrap();
        let narrow_text = rendered_text(narrow.backend().buffer());
        assert!(!narrow_text.contains("上下文"));
        assert!(narrow_text.contains("你›"));
        assert!(narrow_text.contains("模型回答"));
    }

    #[test]
    fn missing_usage_and_stream_estimate_are_explicit() {
        let mut state = state();
        state.status.usage_complete = false;
        state.estimating = true;
        state.append_delta("你好");
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("用量部分缺失"));
        assert!(text.contains("~表示流式估算"));
    }

    #[test]
    fn stream_estimate_counts_only_new_output_with_existing_context() {
        let mut state = state();
        state.status.context_tokens = 1000;
        state.status.context_window_tokens = 2000;
        state.status.turn_tokens = 10;
        state.status.total_tokens = 100;
        state.append_delta("你好abc");
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("1000/2000"));
        assert!(text.contains("~13tokens"));
        assert!(text.contains("~103tokens"));
        assert!(!text.contains("~1013tokens"));

        state.apply_usage(Some(Usage {
            input: 20,
            output: 4,
        }));
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("34tokens"));
        assert!(text.contains("124tokens"));
        assert!(!text.contains('~'));
    }

    #[test]
    fn session_or_workspace_change_clears_transient_chat_state() {
        for (new_id, new_workspace, mode) in [
            ("new-session", "/tmp/test-workspace", Mode::Chat),
            ("opened-session", "/tmp/test-workspace", Mode::Sessions),
            ("test-session", "/tmp/other-workspace", Mode::Workspace),
        ] {
            let mut state = state();
            state.mode = mode;
            state.append(Kind::User, "旧对话");
            state.append_delta("旧回答");
            state.input.set("旧草稿");
            state.workspace_input.set("旧路径");
            state.scroll_back = 12;
            let mut next = self::state().status;
            next.session_id = new_id.to_owned();
            next.workspace = new_workspace.to_owned();
            next.turn_tokens = 0;
            state.sync_session(next);

            assert!(state.entries.is_empty());
            assert!(state.input.text().is_empty());
            assert!(state.workspace_input.text().is_empty());
            assert_eq!(state.scroll_back, 0);
            assert!(!state.estimating);
            assert_eq!(state.live_chars, 0);
            assert_eq!(state.status.session_id, new_id);
            assert_eq!(state.mode, mode);

            state.append(Kind::System, "切换成功");
            let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
            terminal.draw(|frame| render(frame, &state)).unwrap();
            let text = rendered_text(terminal.backend().buffer());
            assert!(!text.contains("旧对话"));
            assert!(!text.contains("旧回答"));
        }
    }

    #[test]
    fn unchanged_session_preserves_messages_and_drafts() {
        let mut state = state();
        state.append(Kind::User, "原对话");
        state.input.set("尚未发送");
        state.workspace_input.set("/missing-path");
        state.scroll_back = 12;
        state.sync_session(self::state().status);
        assert_eq!(state.entries[0].text, "原对话");
        assert_eq!(state.input.text(), "尚未发送");
        assert_eq!(state.workspace_input.text(), "/missing-path");
        assert_eq!(state.scroll_back, 12);
    }

    #[test]
    fn reported_usage_replaces_estimate_between_model_steps() {
        let mut state = state();
        state.status.turn_tokens = 0;
        state.estimating = true;
        state.append_delta("partial");
        state.apply_usage(Some(Usage {
            input: 10,
            output: 2,
        }));
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("12tokens"));
        assert!(text.contains("19tokens"));
        assert!(!text.contains("~"));

        state.append_delta("next step");
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("~"));
        state.apply_usage(None);
        terminal.draw(|frame| render(frame, &state)).unwrap();
        let text = rendered_text(terminal.backend().buffer());
        assert!(text.contains("用量部分缺失"));
        assert!(text.contains("12tokens"));
        assert!(!text.contains("~"));
    }

    #[test]
    fn web_authorization_wraps_full_urls_and_retains_the_current_scope() {
        let mut state = state();
        state.mode = Mode::Confirm;
        let url = format!("http://127.0.0.1:8123/{}?query=full", "path/".repeat(20));
        state.append(
            Kind::Tool,
            format!(
                "工具 web_fetch 请求本次访问授权：\n访问 URL：{url}\n跨来源重定向再次确认。[y/N]"
            ),
        );
        for width in [40, 80] {
            let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
            terminal.draw(|frame| render(frame, &state)).unwrap();
            let text = rendered_text(terminal.backend().buffer());
            let buffer = terminal.backend().buffer();
            let mut chat = String::new();
            for y in 1..26 {
                for x in 1..width {
                    let symbol = buffer[(x, y)].symbol();
                    if symbol == "│" {
                        break;
                    }
                    chat.push_str(symbol);
                }
            }
            let compact: String = chat.chars().filter(|ch| !ch.is_whitespace()).collect();
            assert!(compact.contains(&url), "{text}");
            assert!(compact.contains("本次访问授权"));
            assert!(compact.contains("跨来源重定向再次确认"));
        }
    }

    #[test]
    fn confirmation_defaults_to_denial_and_control_d_exits() {
        assert!(authorization_allowed(" y "));
        assert!(authorization_allowed("YES"));
        for answer in ["", "n", "maybe", "y then run"] {
            assert!(!authorization_allowed(answer));
        }
        let mut state = state();
        state.mode = Mode::Confirm;
        let exit = state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(matches!(exit, Some(Action::Exit)));
    }

    #[test]
    fn memory_shortcut_and_panel_preserve_the_chat_draft() {
        let mut state = state();
        state.input.set("未发送草稿");
        let f4 = || Event::Key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
        assert!(matches!(
            state.handle_event(f4()),
            Some(Action::Memory(super::MemoryAction::Open))
        ));
        state.mode = Mode::Memories;
        state.handle_event(Event::Paste("列表不接收聊天粘贴".into()));
        assert_eq!(state.input.text(), "未发送草稿");
        assert!(matches!(
            state.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))),
            Some(Action::Memory(super::MemoryAction::Close))
        ));
        state.mode = Mode::Confirm;
        assert!(state.handle_event(f4()).is_none());
        assert_eq!(state.mode, Mode::Confirm);
    }

    #[test]
    fn session_shortcut_preserves_draft_and_does_not_interrupt_tool_confirmation() {
        let mut state = state();
        state.input.set("未发送草稿");
        let f3 = || Event::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
        assert!(matches!(
            state.handle_event(f3()),
            Some(Action::Session(super::SessionAction::Reload(
                crate::interaction::SessionScope::Current
            )))
        ));
        state.mode = Mode::Sessions;
        state.handle_event(Event::Paste("管理面板不接收聊天粘贴".to_owned()));
        state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        )));
        assert_eq!(state.input.text(), "未发送草稿");
        assert!(matches!(
            state.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))),
            Some(Action::Session(super::SessionAction::Close))
        ));
        state.mode = Mode::Confirm;
        assert!(state.handle_event(f3()).is_none());
        assert_eq!(state.mode, Mode::Confirm);
        state.mode = Mode::Workspace;
        assert!(state.handle_event(f3()).is_none());
        assert_eq!(state.mode, Mode::Workspace);
    }

    #[test]
    fn workspace_editor_preserves_chat_draft_and_keeps_path_until_cancelled() {
        let mut state = state();
        state.input.set("尚未发送的草稿");
        assert!(
            state
                .handle_event(Event::Key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)))
                .is_none()
        );
        assert_eq!(state.mode, Mode::Workspace);
        assert_eq!(state.workspace_input.text(), "/tmp/test-workspace");
        state.workspace_input.set("/missing path");
        let submit = state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(matches!(submit, Some(Action::Submit(line)) if line == "/workspace /missing path"));
        assert_eq!(state.workspace_input.text(), "/missing path");
        assert_eq!(state.input.text(), "尚未发送的草稿");
        state.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert_eq!(state.mode, Mode::Chat);
        assert_eq!(state.workspace_input.text(), "");
        assert_eq!(state.input.text(), "尚未发送的草稿");

        assert!(
            state
                .handle_event(Event::Key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)))
                .is_none()
        );
        state.workspace_input.set("   ");
        let empty = state.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(matches!(empty, Some(Action::Submit(line)) if line == "/workspace \"\""));
        assert_eq!(state.mode, Mode::Workspace);
        state.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));

        let mut narrow = Terminal::new(TestBackend::new(40, 5)).unwrap();
        narrow.draw(|frame| render(frame, &state)).unwrap();
        assert!(rendered_text(narrow.backend().buffer()).contains("F2Workspace"));
    }
}
