use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const BG_BAR:    Color = Color::Rgb(14, 14, 16);
const TEXT:       Color = Color::Rgb(224, 224, 224);
const TEXT_MUTED: Color = Color::Rgb(80, 80, 80);

pub struct StatusBar {
    left: String,
}

impl StatusBar {
    pub fn new(initial: &str) -> Self { Self { left: initial.to_string() } }
    pub fn set(&mut self, text: &str) { self.left = text.to_string(); }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(&self.left, Style::default().fg(TEXT)),
            Span::raw("  "),
            Span::styled("wheel ↑↓  ·  jk ↑↓  ·  / cmd  ·  space expand  ·  tab focus  ·  y copy  ·  ctrl+c stop  ·  esc quit",
                Style::default().fg(TEXT_MUTED)),
        ]);
        let p = Paragraph::new(line).style(Style::default().bg(BG_BAR));
        f.render_widget(p, area);
    }
}