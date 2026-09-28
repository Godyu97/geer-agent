//! Ratatui 终端界面。只通过界面无关契约使用会话能力。

mod input;

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

use crate::interaction::{self, DiagnosticBuffer, Input as Command, Session, SessionStatus, Usage};
use input::Input;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Chat,
    Confirm,
}

struct State {
    entries: Vec<Entry>,
    input: Input,
    mode: Mode,
    status: SessionStatus,
    estimating: bool,
    live_chars: u64,
    scroll_back: usize,
    exit_requested: bool,
}

enum Action {
    Submit(String),
    Exit,
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
                    mode: Mode::Chat,
                    status: SessionStatus {
                        model: String::new(),
                        session_id: String::new(),
                        context_tokens: 0,
                        context_window_tokens: 1,
                        turn_tokens: 0,
                        total_tokens: 0,
                        usage_complete: true,
                    },
                    estimating: false,
                    live_chars: 0,
                    scroll_back: 0,
                    exit_requested: false,
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
            screen.state.status = session.status();
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

        let report = session.flush().await;
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

        let message = match command {
            Command::Empty => return Ok(false),
            Command::Exit => return Ok(true),
            Command::Help => Some(interaction::help_text().to_owned()),
            Command::Reset | Command::New => {
                session.new_session().await;
                Some(format!(
                    "已开始新会话。\nSession ID: {}",
                    session.session_id()
                ))
            }
            Command::Save => Some(session.flush().await),
            Command::Compact => match session.compact().await {
                Ok(result) => Some(result),
                Err(error) => Some(format!("上下文压缩失败：{error}")),
            },
            Command::Sessions => match session.sessions().await {
                Ok(items) if items.is_empty() => {
                    Some("没有可列出的会话（或未配置会话数据库）。".to_owned())
                }
                Ok(items) => Some(items.join("\n")),
                Err(error) => Some(format!("会话列表读取失败：{error}")),
            },
            Command::Open(id) => match session.open(&id).await {
                Ok(message) => Some(format!("{message}\nSession ID: {}", session.session_id())),
                Err(error) => Some(format!("会话恢复失败：{error}")),
            },
            Command::Unknown(command) => {
                Some(format!("未知命令：{command}（输入 /help 查看可用命令）"))
            }
            Command::Message(message) => {
                {
                    let mut screen = self.screen.borrow_mut();
                    screen.state.status.turn_tokens = 0;
                    screen.state.estimating = true;
                    screen.state.live_chars = 0;
                    screen.draw()?;
                }
                let output = Rc::clone(&self.screen);
                let usage_output = Rc::clone(&self.screen);
                let result = session
                    .handle_message(
                        &message,
                        move |delta| output.borrow_mut().delta(delta),
                        move |usage| usage_output.borrow_mut().usage(usage),
                    )
                    .await;
                let mut screen = self.screen.borrow_mut();
                screen.state.estimating = false;
                screen.state.live_chars = 0;
                if let Err(error) = result {
                    screen
                        .state
                        .append(Kind::System, format!("模型请求失败：{error}"));
                }
                screen.state.status = session.status();
                screen.draw()?;
                return Ok(false);
            }
        };
        let mut screen = self.screen.borrow_mut();
        if let Some(message) = message {
            screen.state.append(Kind::System, message);
        }
        screen.state.status = session.status();
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
                match key.code {
                    KeyCode::Enter => return Some(Action::Submit(self.input.take())),
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.input.insert(ch);
                    }
                    KeyCode::Backspace => self.input.backspace(),
                    KeyCode::Delete => self.input.delete(),
                    KeyCode::Left => self.input.left(),
                    KeyCode::Right => self.input.right(),
                    KeyCode::Home => self.input.home(),
                    KeyCode::End => self.input.end(),
                    KeyCode::PageUp => self.scroll_back = self.scroll_back.saturating_add(10),
                    KeyCode::PageDown => self.scroll_back = self.scroll_back.saturating_sub(10),
                    KeyCode::Up => self.scroll_back = self.scroll_back.saturating_add(1),
                    KeyCode::Down => self.scroll_back = self.scroll_back.saturating_sub(1),
                    KeyCode::Esc if self.mode == Mode::Confirm => {
                        return Some(Action::Submit(String::new()));
                    }
                    KeyCode::Esc => {
                        self.input.clear();
                        self.scroll_back = 0;
                    }
                    _ => {}
                }
            }
            Event::Paste(value) => self.input.paste(&value),
            _ => {}
        }
        None
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
        context.saturating_add(state.live_chars.div_ceil(2))
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
        Line::raw(""),
        Line::raw("── 上下文（估算）──"),
        Line::raw(format!("{context} / {window}")),
        Line::raw(format!("{percent}% used")),
        Line::raw(""),
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
    let title = if state.mode == Mode::Confirm {
        "授权确认"
    } else {
        "输入"
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
    let full_prompt = if state.mode == Mode::Confirm {
        "[y/N] "
    } else {
        "你 › "
    };
    let prompt = if Line::raw(full_prompt).width() < inner.width as usize {
        full_prompt
    } else {
        "› "
    };
    let prompt_width = Line::raw(prompt).width().min(inner.width as usize);
    let available = (inner.width as usize).saturating_sub(prompt_width);
    let (visible, cursor) = state.input.visible(available);
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
            mode: Mode::Chat,
            status: SessionStatus {
                model: "test-model".into(),
                session_id: "test-session".into(),
                context_tokens: 50,
                context_window_tokens: 100,
                turn_tokens: 3,
                total_tokens: 7,
                usage_complete: true,
            },
            estimating: false,
            live_chars: 0,
            scroll_back: 0,
            exit_requested: false,
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
}
