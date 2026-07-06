use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const BG_BAR:    Color = Color::Rgb(14, 14, 16);
const TEXT:       Color = Color::Rgb(224, 224, 224);
const TEXT_MUTED: Color = Color::Rgb(80, 80, 80);
const KEY_COLOR:  Color = Color::Rgb(200, 200, 200);

pub struct StatusBar {
    left: String,
}

impl StatusBar {
    pub fn new(initial: &str) -> Self { Self { left: initial.to_string() } }
    pub fn set(&mut self, text: &str) { self.left = text.to_string(); }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            Span::styled(format!(" {} ", self.left), Style::default().fg(TEXT)),

            Span::styled("wheel", Style::default().fg(KEY_COLOR)),
            Span::styled(" ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("jk", Style::default().fg(KEY_COLOR)),
            Span::styled(" ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("/", Style::default().fg(KEY_COLOR)),
            Span::styled(" cmd ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("space", Style::default().fg(KEY_COLOR)),
            Span::styled(" expand ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("tab", Style::default().fg(KEY_COLOR)),
            Span::styled(" focus ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("y", Style::default().fg(KEY_COLOR)),
            Span::styled(" copy ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("ctrl+c", Style::default().fg(KEY_COLOR)),
            Span::styled(" stop ▌ ", Style::default().fg(TEXT_MUTED)),
            Span::styled("esc", Style::default().fg(KEY_COLOR)),
            Span::styled(" quit", Style::default().fg(TEXT_MUTED)),
        ]);
        let p = Paragraph::new(line).style(Style::default().bg(BG_BAR));
        f.render_widget(p, area);
    }
}