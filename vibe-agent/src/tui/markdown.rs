use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut span_style = Style::default().fg(Color::White);

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    span_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                    let prefix = "#".repeat(level as usize) + " ";
                    current_line.push(Span::styled(prefix, span_style));
                }
                Tag::CodeBlock(_) => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    in_code_block = true;
                    lines.push(Line::from(Span::styled("```", Style::default().fg(Color::DarkGray))));
                }
                Tag::BlockQuote(_) => {
                    current_line.push(Span::styled("| ", Style::default().fg(Color::DarkGray)));
                }
                Tag::Strong => { span_style = span_style.add_modifier(Modifier::BOLD); }
                Tag::Emphasis => { span_style = span_style.add_modifier(Modifier::ITALIC); }
                Tag::Item => { current_line.push(Span::styled("  . ", Style::default().fg(Color::Cyan))); }
                _ => {}
            },

            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    span_style = Style::default().fg(Color::White);
                    if matches!(tag, TagEnd::Paragraph) { lines.push(Line::default()); }
                }
                TagEnd::CodeBlock => { in_code_block = false; lines.push(Line::default()); }
                TagEnd::Strong => { span_style = span_style.remove_modifier(Modifier::BOLD); }
                TagEnd::Emphasis => { span_style = span_style.remove_modifier(Modifier::ITALIC); }
                _ => {}
            },

            Event::Text(text) => {
                if in_code_block {
                    for code_line in text.lines() {
                        lines.push(Line::from(Span::styled(format!("  {}", code_line), Style::default().fg(Color::Rgb(180, 210, 180)))));
                    }
                } else {
                    current_line.push(Span::styled(text.to_string(), span_style));
                }
            }
            Event::Code(code) => {
                current_line.push(Span::styled(format!("`{}`", code), Style::default().fg(Color::Rgb(200, 180, 120)).bg(Color::Rgb(30, 30, 30))));
            }
            Event::SoftBreak | Event::HardBreak => { current_line.push(Span::raw(" ")); }
            Event::Rule => {
                if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                lines.push(Line::from(Span::styled("─".repeat(60), Style::default().fg(Color::DarkGray))));
                lines.push(Line::default());
            }
            _ => {}
        }
    }
    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
    lines
}