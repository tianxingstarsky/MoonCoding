use pulldown_cmark::{Alignment, Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use super::syntax;

pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer = String::new();
    let mut span_style = Style::default().fg(Color::Rgb(224, 224, 224));
    // table state
    let mut in_table = false;
    let mut in_table_head = false;
    let mut table_row_cells: Vec<String> = Vec::new();
    let mut table_col_widths: Vec<usize> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    span_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                    current_line.push(Span::styled("#".repeat(level as usize) + " ", span_style));
                }
                Tag::CodeBlock(kind) => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    in_code_block = true;
                    code_lang = match kind { pulldown_cmark::CodeBlockKind::Fenced(f) => f.to_string(), _ => String::new() };
                    let label = if code_lang.is_empty() { "```" } else { &code_lang };
                    lines.push(Line::from(Span::styled(format!("```{}", label), Style::default().fg(Color::DarkGray))));
                }
                Tag::BlockQuote(_) => { current_line.push(Span::styled("| ", Style::default().fg(Color::DarkGray))); }
                Tag::Strong => { span_style = span_style.add_modifier(Modifier::BOLD); }
                Tag::Emphasis => { span_style = span_style.add_modifier(Modifier::ITALIC); }
                Tag::Item => { current_line.push(Span::styled("  · ", Style::default().fg(Color::Cyan))); }
                Tag::Table(_) => { in_table = true; table_rows.clear(); table_col_widths.clear(); lines.push(Line::default()); }
                Tag::TableHead => { in_table_head = true; }
                Tag::TableRow => { table_row_cells.clear(); }
                Tag::TableCell => { table_row_cells.push(String::new()); }
                _ => {}
            },

            Event::End(tag) => match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) => {
                    if !current_line.is_empty() { lines.push(Line::from(current_line.drain(..).collect::<Vec<_>>())); }
                    span_style = Style::default().fg(Color::Rgb(224, 224, 224));
                    if matches!(tag, TagEnd::Paragraph) { lines.push(Line::default()); }
                }
                TagEnd::CodeBlock => {
                    let highlighted = syntax::highlight(&code_buffer, &code_lang);
                    lines.extend(highlighted);
                    in_code_block = false; code_buffer.clear(); code_lang.clear();
                    lines.push(Line::default());
                }
                TagEnd::Strong => { span_style = span_style.remove_modifier(Modifier::BOLD); }
                TagEnd::Emphasis => { span_style = span_style.remove_modifier(Modifier::ITALIC); }
                TagEnd::Table => {
                    in_table = false;
                    if !table_rows.is_empty() {
                        // render table with box-drawing characters
                        let has_header = table_rows.len() >= 2 && in_table_head;
                        // compute column widths
                        let mut widths = vec![0usize; table_col_widths.len().max(1)];
                        // first pass: use the first row as header to size
                        for row in &table_rows {
                            for (i, cell) in row.iter().enumerate() {
                                let w = cell.chars().count().max(3);
                                if i < widths.len() && w > widths[i] { widths[i] = w; }
                            }
                        }
                        // render top border
                        let mut sep = String::from("┌");
                        for (i, w) in widths.iter().enumerate() {
                            sep.push_str(&"─".repeat(w + 2));
                            if i + 1 < widths.len() { sep.push('┬'); }
                        }
                        sep.push('┐');
                        lines.push(Line::from(Span::styled(sep, Style::default().fg(Color::DarkGray))));

                        for (ri, row) in table_rows.iter().enumerate() {
                            let mut line_str = String::from("│");
                            for (i, cell) in row.iter().enumerate() {
                                let w = widths.get(i).copied().unwrap_or(10);
                                let padded = format!(" {:<w$} ", cell, w=w);
                                line_str.push_str(&padded);
                                if i + 1 < row.len() { line_str.push('│'); }
                            }
                            line_str.push('│');
                            let style = if in_table_head && ri == 0 {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Rgb(224,224,224))
                            };
                            lines.push(Line::from(Span::styled(line_str, style)));

                            // separator after header
                            if ri == 0 && has_header {
                                let mut mid = String::from("├");
                                for (i, w) in widths.iter().enumerate() {
                                    mid.push_str(&"─".repeat(w + 2));
                                    if i + 1 < widths.len() { mid.push('┼'); }
                                }
                                mid.push('┤');
                                lines.push(Line::from(Span::styled(mid, Style::default().fg(Color::DarkGray))));
                            }
                        }
                        // bottom border
                        let mut bot = String::from("└");
                        for (i, w) in widths.iter().enumerate() {
                            bot.push_str(&"─".repeat(w + 2));
                            if i + 1 < widths.len() { bot.push('┴'); }
                        }
                        bot.push('┘');
                        lines.push(Line::from(Span::styled(bot, Style::default().fg(Color::DarkGray))));
                        lines.push(Line::default());
                    }
                    table_rows.clear();
                    table_col_widths.clear();
                }
                TagEnd::TableHead => { in_table_head = false; }
                TagEnd::TableRow => {
                    table_rows.push(table_row_cells.clone());
                    for (i, c) in table_row_cells.iter().enumerate() {
                        let w = c.chars().count();
                        while table_col_widths.len() <= i { table_col_widths.push(0); }
                        if w > table_col_widths[i] { table_col_widths[i] = w; }
                    }
                }
                _ => {}
            },

            Event::Text(text) => {
                if in_code_block {
                    code_buffer.push_str(&text);
                } else if in_table && !table_row_cells.is_empty() {
                    let last = table_row_cells.last_mut().unwrap();
                    last.push_str(&text);
                } else {
                    current_line.push(Span::styled(text.to_string(), span_style));
                }
            }
            Event::Code(code) => {
                current_line.push(Span::styled(format!("`{}`", code), Style::default().fg(Color::Rgb(200,180,120)).bg(Color::Rgb(30,30,30))));
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_table { /* ignore soft breaks in tables */ } else { current_line.push(Span::raw(" ")); }
            }
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