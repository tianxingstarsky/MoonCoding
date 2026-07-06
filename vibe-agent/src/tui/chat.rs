use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::markdown;

pub struct ChatPanel {
    lines: Vec<ChatLine>,
    scroll: u16,
    current_assistant: Option<String>,
}
#[derive(Clone)]
pub struct ChatLine {
    pub text: Line<'static>,
    pub tool_call: Option<ToolCallInfo>,
    pub expanded: bool,
}
#[derive(Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub exit_code: i32,
    pub full_output: String,
}

impl ChatPanel {
    pub fn new() -> Self { Self { lines: Vec::new(), scroll: 0, current_assistant: None } }
    pub fn scroll_up(&mut self) { self.scroll = self.scroll.saturating_add(1); }
    pub fn scroll_down(&mut self) { self.scroll = self.scroll.saturating_sub(1); }
    pub fn scroll_to_bottom(&mut self) { self.scroll = 0; }
    pub fn scroll_pos(&self) -> usize { self.scroll as usize }
    pub fn line_count(&self) -> usize { self.lines.len() }
    pub fn toggle_line(&mut self, idx: usize) -> bool {
        if let Some(line) = self.lines.get_mut(idx) {
            if line.tool_call.is_some() { line.expanded = !line.expanded; return true; }
        }
        false
    }
    pub fn push(&mut self, line: Line<'static>) {
        self.lines.push(ChatLine { text: line, tool_call: None, expanded: false });
    }
    pub fn push_user(&mut self, text: &str) {
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled("moon> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(text.to_string()),
            ]),
            tool_call: None, expanded: false,
        });
        self.current_assistant = None;
    }
    pub fn append_delta(&mut self, text: &str) {
        if let Some(ref mut cur) = self.current_assistant {
            cur.push_str(text);
            let content = cur.clone();
            if let Some(last) = self.lines.last_mut() { last.text = Line::from(Span::raw(content)); }
        } else {
            let content = text.to_string();
            self.current_assistant = Some(content.clone());
            self.lines.push(ChatLine { text: Line::from(Span::raw(content)), tool_call: None, expanded: false });
        }
    }
    pub fn push_tool_start(&mut self, name: &str, args: &str) {
        self.current_assistant = None;
        let preview: String = args.chars().take(80).collect();
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled("[", Style::default().fg(Color::DarkGray)),
                Span::styled(name.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("] ", Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::DarkGray)),
            ]),
            tool_call: Some(ToolCallInfo { name: name.to_string(), exit_code: 0, full_output: String::new() }),
            expanded: false,
        });
    }
    pub fn push_tool_result(&mut self, name: &str, exit_code: i32, output: &str) {
        let code_color = if exit_code == 0 { Color::Green } else { Color::Red };
        let first_line = output.lines().next().unwrap_or("");
        if let Some(line) = self.lines.iter_mut().rev().find(|l| l.tool_call.as_ref().map(|tc| tc.name == name).unwrap_or(false)) {
            line.tool_call.as_mut().unwrap().exit_code = exit_code;
            line.tool_call.as_mut().unwrap().full_output = output.to_string();
        }
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled(format!("  {} ", name), Style::default().fg(Color::Cyan)),
                Span::styled(format!("exit {}", exit_code), Style::default().fg(code_color)),
                Span::raw("  "),
                Span::styled(first_line.to_string(), Style::default().fg(Color::DarkGray)),
            ]),
            tool_call: None, expanded: false,
        });
    }
    pub fn push_error(&mut self, text: &str) {
        self.current_assistant = None;
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled("x ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(text.to_string(), Style::default().fg(Color::Red)),
            ]),
            tool_call: None, expanded: false,
        });
    }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool) {
        let mut display_lines: Vec<Line> = Vec::new();
        for line in &self.lines {
            let raw_text: String = line.text.spans.iter().map(|s| s.content.as_ref()).collect::<Vec<&str>>().join("");
            let is_tool = line.tool_call.is_some();
            let mark = if is_tool { if line.expanded { "[-]" } else { "[+]" } } else { "" };

            let md_lines = markdown::render_markdown(&raw_text);
            for md_line in md_lines {
                let mut spans: Vec<Span> = Vec::new();
                if is_tool {
                    spans.push(Span::styled(mark, Style::default().fg(Color::Yellow)));
                    spans.push(Span::raw(" "));
                }
                spans.extend(md_line.into_iter());
                display_lines.push(Line::from(spans));
            }
            if line.expanded {
                if let Some(tc) = &line.tool_call {
                    for md_line in markdown::render_markdown(&tc.full_output) {
                        let mut spans = vec![Span::styled("  | ".to_string(), Style::default().fg(Color::DarkGray))];
                        spans.extend(md_line.into_iter());
                        display_lines.push(Line::from(spans));
                    }
                }
            }
        }
        let title = if focused { " [chat] " } else { " chat " };
        let border_style = if focused { Style::default().fg(Color::Cyan) } else { Style::default() };
        let chat = Paragraph::new(display_lines)
            .block(Block::default().borders(Borders::ALL).title(title).border_style(border_style))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        f.render_widget(chat, area);
    }
}