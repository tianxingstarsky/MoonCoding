use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub struct StatusBar {
    text: String,
}

impl StatusBar {
    pub fn new(initial: &str) -> Self { Self { text: initial.to_string() } }

    pub fn set(&mut self, text: &str) { self.text = text.to_string(); }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(&self.text, Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled("/ cmd | jk scroll | space expand | Esc quit", Style::default().fg(Color::DarkGray)),
        ]);
        let p = Paragraph::new(line)
            .style(Style::default().bg(Color::Rgb(30, 30, 30)));
        f.render_widget(p, area);
    }
}