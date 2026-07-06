use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct ChatPanel {
    lines: Vec<Line<'static>>,
    scroll: u16,
    current_assistant: Option<String>,
}

impl ChatPanel {
    pub fn new() -> Self { Self { lines: Vec::new(), scroll: 0, current_assistant: None } }

    pub fn scroll_up(&mut self) { self.scroll = self.scroll.saturating_add(1); }
    pub fn scroll_down(&mut self) { self.scroll = self.scroll.saturating_sub(1); }
    pub fn scroll_to_bottom(&mut self) { self.scroll = 0; }

    pub fn push(&mut self, line: Line<'static>) {
        self.lines.push(line);
    }

    pub fn push_user(&mut self, text: &str) {
        self.lines.push(Line::from(vec![
            Span::styled("moon> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(text.to_string()),
        ]));
        self.current_assistant = None;
    }

    pub fn append_delta(&mut self, text: &str) {
        if let Some(ref mut cur) = self.current_assistant {
            cur.push_str(text);
            // update the last line
            let last = self.lines.last_mut().unwrap();
            *last = Line::from(Span::raw(cur.clone()));
        } else {
            let content = text.to_string();
            self.current_assistant = Some(content.clone());
            self.lines.push(Line::from(Span::raw(content)));
        }
    }

    pub fn push_tool_start(&mut self, name: &str, args: &str) {
        self.current_assistant = None;
        let preview: String = args.chars().take(80).collect();
        self.lines.push(Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(name.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled(preview, Style::default().fg(Color::DarkGray)),
        ]));
    }

    pub fn push_tool_result(&mut self, name: &str, exit_code: i32, output: &str) {
        let code_color = if exit_code == 0 { Color::Green } else { Color::Red };
        let first_line = output.lines().next().unwrap_or("");
        self.lines.push(Line::from(vec![
            Span::styled(format!("  {} ", name), Style::default().fg(Color::Cyan)),
            Span::styled(format!("exit {}", exit_code), Style::default().fg(code_color)),
            Span::raw("  "),
            Span::styled(first_line.to_string(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    pub fn push_error(&mut self, text: &str) {
        self.current_assistant = None;
        self.lines.push(Line::from(vec![
            Span::styled("✗ ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(text.to_string(), Style::default().fg(Color::Red)),
        ]));
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chat = Paragraph::new(self.lines.clone())
            .block(Block::default().borders(Borders::ALL).title(" chat "))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        f.render_widget(chat, area);
    }
}