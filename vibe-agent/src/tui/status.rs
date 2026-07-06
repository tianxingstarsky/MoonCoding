use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct StatusBar {
    pub left: String,
    pub right: String,
    pub shortcut: String,
}

impl StatusBar {
    pub fn new(left: &str) -> Self {
        Self { left: left.to_string(), right: String::new(), shortcut: "Ctrl+C stop · Tab focus · jk scroll · / cmd · Esc quit".into() }
    }

    pub fn set(&mut self, text: &str) { self.left = text.to_string(); }
    pub fn set_right(&mut self, text: &str) { self.right = text.to_string(); }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(&self.left, Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(&self.right, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(&self.shortcut, Style::default().fg(Color::Rgb(80, 80, 80))),
        ]);
        let p = Paragraph::new(line)
            .style(Style::default().bg(Color::Rgb(20, 20, 24)));
        f.render_widget(p, area);
    }
}