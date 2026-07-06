use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct InputPanel {
    pub buffer: String,
    pub cursor: usize,     // char position, not byte
    pub command_mode: bool,
}

impl InputPanel {
    pub fn new() -> Self { Self { buffer: String::new(), cursor: 0, command_mode: false } }

    pub fn toggle_command(&mut self) {
        self.command_mode = !self.command_mode;
        if self.command_mode { self.buffer = "/".into(); self.cursor = 1; }
        else { self.buffer.clear(); self.cursor = 0; }
    }

    /// char count
    fn len_chars(&self) -> usize { self.buffer.chars().count() }

    /// byte offset for char position `idx`
    fn byte_of(&self, idx: usize) -> usize {
        self.buffer.char_indices().nth(idx).map(|(i,_)| i).unwrap_or(self.buffer.len())
    }

    pub fn push_char(&mut self, c: char) {
        let byte_pos = self.byte_of(self.cursor);
        self.buffer.insert(byte_pos, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev_idx = self.cursor - 1;
            let byte_pos = self.byte_of(prev_idx);
            self.buffer.remove(byte_pos);
            self.cursor = prev_idx;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.len_chars() {
            let byte_pos = self.byte_of(self.cursor);
            self.buffer.remove(byte_pos);
        }
    }

    pub fn cursor_left(&mut self) { if self.cursor > 0 { self.cursor -= 1; } }
    pub fn cursor_right(&mut self) { if self.cursor < self.len_chars() { self.cursor += 1; } }
    pub fn cursor_home(&mut self) { self.cursor = 0; }
    pub fn cursor_end(&mut self) { self.cursor = self.len_chars(); }

    pub fn take(&mut self) -> String {
        let s = std::mem::take(&mut self.buffer);
        self.cursor = 0;
        s
    }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut s = String::with_capacity(chars.len() + 2);
        for (i, c) in chars.iter().enumerate() {
            if i == self.cursor { s.push('█'); }
            s.push(*c);
        }
        if self.cursor >= chars.len() { s.push('█'); }

        let title = if self.command_mode { " / cmd " } else { " moon> " };
        let title = if focused { format!("[{}]", title.trim()) } else { title.to_string() };
        let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default() };
        let input = Paragraph::new(s)
            .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
            .style(Style::default().fg(Color::White));
        f.render_widget(input, area);
    }
}