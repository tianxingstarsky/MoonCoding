use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use similar::{ChangeTag, TextDiff};

/// 生成带颜色的 unified diff 视图，用于在 TUI 中展示代码改动
pub fn render_diff(old: &str, new: &str, old_label: &str, new_label: &str) -> Vec<Line<'static>> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("--- ", Style::default().fg(Color::DarkGray)),
        Span::styled(old_label.to_string(), Style::default().fg(Color::Red)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("+++ ", Style::default().fg(Color::DarkGray)),
        Span::styled(new_label.to_string(), Style::default().fg(Color::Green)),
    ]));

    for change in diff.iter_all_changes() {
        let (prefix, style) = match change.tag() {
            ChangeTag::Equal => (" ", Style::default().fg(Color::DarkGray)),
            ChangeTag::Delete => ("-", Style::default().fg(Color::Red)),
            ChangeTag::Insert => ("+", Style::default().fg(Color::Green)),
        };
        lines.push(Line::from(vec![
            Span::styled(prefix.to_string(), style.add_modifier(Modifier::BOLD)),
            Span::styled(change.value().to_string(), style),
        ]));
    }
    lines
}