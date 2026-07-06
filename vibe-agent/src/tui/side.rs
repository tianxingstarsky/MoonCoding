use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct SidePanel {
    pub title: String,
    pub entries: Vec<(String, String)>,
}

impl SidePanel {
    pub fn new(title: &str) -> Self { Self { title: title.into(), entries: Vec::new() } }

    pub fn set_entries(&mut self, entries: Vec<(String, String)>) { self.entries = entries; }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let lines: Vec<Line> = self.entries.iter().map(|(k, v)| {
            Line::from(vec![
                Span::styled(k, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(v, Style::default().fg(Color::DarkGray)),
            ])
        }).collect();

        let title = if focused { " [info] " } else { " info " };
        let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default() };
        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(format!(" {} ", title)).border_style(border_style))
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }
}