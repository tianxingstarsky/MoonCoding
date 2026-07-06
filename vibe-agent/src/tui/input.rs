use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct InputPanel {
    pub buffer: String,
    pub cursor: usize,
    pub command_mode: bool,
}

impl InputPanel {
    pub fn new() -> Self { Self { buffer: String::new(), cursor: 0, command_mode: false } }

    pub fn toggle_command(&mut self) {
        self.command_mode = !self.command_mode;
        if self.command_mode { self.buffer = "/".into(); self.cursor = 1; }
        else { self.buffer.clear(); self.cursor = 0; }
    }

    pub fn push_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.buffer.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    pub fn cursor_left(&mut self) { if self.cursor > 0 { self.cursor -= 1; } }
    pub fn cursor_right(&mut self) { if self.cursor < self.buffer.len() { self.cursor += 1; } }
    pub fn cursor_home(&mut self) { self.cursor = 0; }
    pub fn cursor_end(&mut self) { self.cursor = self.buffer.len(); }
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    pub fn take(&mut self) -> String {
        let s = self.buffer.clone();
        self.buffer.clear();
        self.cursor = 0;
        s
    }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let display = if self.buffer.is_empty() {
            "█".to_string()
        } else {
            let mut s = self.buffer.clone();
            if self.cursor <= s.len() {
                s.insert(self.cursor.min(s.len()), '█');
            }
            s
        };
        let title = if self.command_mode { " / cmd " } else { " moon> " };
        let title = if focused { format!("[{}]", title.trim()) } else { title.to_string() };
        let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default() };
        let input = Paragraph::new(display.as_str())
            .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
            .style(Style::default().fg(Color::White));
        f.render_widget(input, area);
    }
}