use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

const BG:        Color = Color::Rgb(10, 10, 10);
const BORDER:    Color = Color::Rgb(50, 50, 50);
const BORDER_ACT:Color = Color::Rgb(96, 96, 96);
const TEXT_MUTED: Color = Color::Rgb(96, 96, 96);
const ACCENT:     Color = Color::Rgb(92, 156, 245);

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
                Span::styled(k, Style::default().fg(ACCENT)),
                Span::raw("  "),
                Span::styled(v, Style::default().fg(TEXT_MUTED)),
            ])
        }).collect();

        let border_color = if focused { BORDER_ACT } else { BORDER };
        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(format!(" {} ", self.title)).border_style(Style::default().fg(border_color)).style(Style::default().bg(BG)))
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }
}