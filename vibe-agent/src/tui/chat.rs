use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::markdown;

const BG:        Color = Color::Rgb(10, 10, 10);
const BORDER:    Color = Color::Rgb(50, 50, 50);
const BORDER_ACT:Color = Color::Rgb(96, 96, 96);
const TEXT:       Color = Color::Rgb(224, 224, 224);
const TEXT_MUTED: Color = Color::Rgb(96, 96, 96);
const ACCENT:     Color = Color::Rgb(92, 156, 245);
const SUCCESS:    Color = Color::Rgb(126, 207, 126);
const WARN:       Color = Color::Rgb(224, 180, 100);
const ERROR:      Color = Color::Rgb(224, 80, 80);

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
    fn auto_scroll(&mut self) { self.scroll = 0; }
    pub fn last_message_text(&self) -> String {
        let mut s = String::new();
        for line in &self.lines {
            let txt: String = line.text.spans.iter().map(|sp| sp.content.as_ref()).collect();
            s.push_str(&txt); s.push('\n');
        }
        s
    }
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
                Span::styled("> ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(text.to_string(), Style::default().fg(TEXT)),
            ]),
            tool_call: None, expanded: false,
        });
        self.current_assistant = None;
        self.auto_scroll();
    }
    pub fn append_delta(&mut self, text: &str) {
        if let Some(ref mut cur) = self.current_assistant {
            cur.push_str(text);
            let content = cur.clone();
            if let Some(last) = self.lines.last_mut() { last.text = Line::from(Span::styled(content, Style::default().fg(TEXT))); }
        } else {
            let content = text.to_string();
            self.current_assistant = Some(content.clone());
            self.lines.push(ChatLine { text: Line::from(Span::styled(content, Style::default().fg(TEXT))), tool_call: None, expanded: false });
        }
        self.auto_scroll();
    }
    pub fn push_tool_start(&mut self, name: &str, args: &str) {
        self.current_assistant = None;
        let preview: String = args.chars().take(80).collect();
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled(format!(" {} ", name), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(preview, Style::default().fg(TEXT_MUTED)),
            ]),
            tool_call: Some(ToolCallInfo { name: name.to_string(), exit_code: 0, full_output: String::new() }),
            expanded: false,
        });
        self.auto_scroll();
    }
    pub fn push_tool_result(&mut self, name: &str, exit_code: i32, output: &str) {
        let code_color = if exit_code == 0 { SUCCESS } else { ERROR };
        let first_line = output.lines().next().unwrap_or("");
        if let Some(line) = self.lines.iter_mut().rev().find(|l| l.tool_call.as_ref().map(|tc| tc.name == name).unwrap_or(false)) {
            line.tool_call.as_mut().unwrap().exit_code = exit_code;
            line.tool_call.as_mut().unwrap().full_output = output.to_string();
        }
        self.lines.push(ChatLine {
            text: Line::from(vec![
                Span::styled(format!("  {} ", name), Style::default().fg(TEXT_MUTED)),
                Span::styled(format!("exit {}", exit_code), Style::default().fg(code_color)),
                Span::raw("  "),
                Span::styled(first_line.to_string(), Style::default().fg(TEXT_MUTED)),
            ]),
            tool_call: None, expanded: false,
        });
        self.auto_scroll();
    }
    pub fn push_error(&mut self, text: &str) {
        self.current_assistant = None;
        self.lines.push(ChatLine {
            text: Line::from(Span::styled(text.to_string(), Style::default().fg(ERROR))),
            tool_call: None, expanded: false,
        });
        self.auto_scroll();
    }

    pub fn render(&self, f: &mut Frame, area: Rect, focused: bool, spinner: &str, thinking: bool) {
        let mut display_lines: Vec<Line> = Vec::new();
        for line in &self.lines {
            let raw: String = line.text.spans.iter().map(|s| s.content.as_ref()).collect::<Vec<&str>>().join("");
            let is_tool = line.tool_call.is_some();
            let mark = if is_tool { if line.expanded { "[-]" } else { "[+]" } } else { "" };

            let md_lines = markdown::render_markdown(&raw);
            for md_line in md_lines {
                let mut spans: Vec<Span> = Vec::new();
                if is_tool {
                    spans.push(Span::styled(mark, Style::default().fg(WARN)));
                    spans.push(Span::raw(" "));
                }
                spans.extend(md_line.into_iter());
                display_lines.push(Line::from(spans));
            }
            if line.expanded {
                if let Some(tc) = &line.tool_call {
                    for md_line in markdown::render_markdown(&tc.full_output) {
                        let mut spans = vec![Span::styled("  | ".to_string(), Style::default().fg(TEXT_MUTED))];
                        spans.extend(md_line.into_iter());
                        display_lines.push(Line::from(spans));
                    }
                }
            }
        }
        // spinner line when thinking
        if thinking && current_assistant_is_active(&self.lines) == false {
            display_lines.push(Line::from(Span::styled(
                format!(" {} thinking...", spinner),
                Style::default().fg(TEXT_MUTED),
            )));
        }

        let title = if focused { " chat " } else { " chat " };
        let border_color = if focused { BORDER_ACT } else { BORDER };
        let chat = Paragraph::new(display_lines)
            .block(Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(border_color)).style(Style::default().bg(BG)))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        f.render_widget(chat, area);
    }
}

fn current_assistant_is_active(lines: &[ChatLine]) -> bool {
    lines.last().map(|l| l.tool_call.is_none() && l.text.spans.iter().any(|s| !s.content.trim().is_empty())).unwrap_or(false)
}