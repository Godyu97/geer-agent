use ratatui::text::Line;

#[derive(Default)]
pub(super) struct Input {
    chars: Vec<char>,
    cursor: usize,
}

impl Input {
    pub(super) fn set(&mut self, value: &str) {
        self.chars = value.chars().filter(|ch| !ch.is_control()).collect();
        self.cursor = self.chars.len();
    }

    pub(super) fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub(super) fn insert(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        self.chars.insert(self.cursor, ch);
        self.cursor += 1;
    }

    pub(super) fn paste(&mut self, value: &str) {
        for ch in value.chars() {
            self.insert(if ch.is_whitespace() { ' ' } else { ch });
        }
    }

    pub(super) fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub(super) fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub(super) fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    pub(super) fn home(&mut self) {
        self.cursor = 0;
    }

    pub(super) fn end(&mut self) {
        self.cursor = self.chars.len();
    }

    pub(super) fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    pub(super) fn take(&mut self) -> String {
        let value: String = self.chars.drain(..).collect();
        self.cursor = 0;
        value
    }

    pub(super) fn visible(&self, width: usize) -> (String, usize) {
        if width == 0 {
            return (String::new(), 0);
        }
        let mut start = 0;
        while start < self.cursor && self.width(start, self.cursor) >= width {
            start += 1;
        }
        let content = self.chars[start..].iter().collect();
        (content, self.width(start, self.cursor).min(width - 1))
    }

    fn width(&self, start: usize, end: usize) -> usize {
        Line::raw(self.chars[start..end].iter().collect::<String>()).width()
    }
}

#[cfg(test)]
mod tests {
    use super::Input;

    #[test]
    fn chinese_cursor_and_editing_follow_terminal_columns() {
        let mut input = Input::default();
        input.paste("你好abc");
        assert_eq!(input.visible(5), ("abc".into(), 3));
        input.left();
        input.left();
        input.backspace();
        assert_eq!(input.take(), "你好bc");
    }

    #[test]
    fn pasted_lines_do_not_submit_or_insert_controls() {
        let mut input = Input::default();
        input.paste("first\nsecond\tline\u{1b}");
        assert_eq!(input.take(), "first second line");
    }
}
