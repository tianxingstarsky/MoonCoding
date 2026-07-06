use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::LazyLock;

// 所有语言语法定义 (懒加载)
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
// 暗色主题, 适合终端
static THEME: LazyLock<syntect::highlighting::Theme> = LazyLock::new(|| {
    let ts = ThemeSet::load_defaults();
    ts.themes["base16-ocean.dark"].clone()
});

/// 尝试按语言名对 code 做语法高亮, 失败回退为无色原始行
pub fn highlight(code: &str, lang: &str) -> Vec<Line<'static>> {
    let ss = &*SYNTAX_SET;
    if lang.is_empty() || lang == "code" {
        return plain_lines(code);
    }

    // 扩展名和别名映射 (syntect 使用 scope 名, 这里映射常用语言标签到 scope)
    let scope = resolve_lang(lang);
    let syntax = match ss.find_syntax_by_token(scope) {
        Some(s) => s,
        None => {
            // try by name
            if let Some(s) = ss.find_syntax_by_name(lang) { s }
            // try by extension
            else if let Some(s) = ss.find_syntax_by_extension(lang) { s }
            else { return plain_lines(code); }
        }
    };

    let mut h = HighlightLines::new(syntax, &THEME);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for line in LinesWithEndings::from(code) {
        let ranges = match h.highlight_line(line, ss) {
            Ok(r) => r,
            Err(_) => { lines.push(Line::from(Span::raw(line.to_string()))); continue; }
        };
        let spans: Vec<Span> = ranges.into_iter().map(|(style, text)| {
            let fg = style.foreground;
            let color = Color::Rgb(fg.r, fg.g, fg.b);
            let mut rat_style = Style::default().fg(color);
            if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                rat_style = rat_style.add_modifier(ratatui::style::Modifier::BOLD);
            }
            if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                rat_style = rat_style.add_modifier(ratatui::style::Modifier::ITALIC);
            }
            Span::styled(text.to_string(), rat_style)
        }).collect();
        lines.push(Line::from(spans));
    }
    lines
}

fn plain_lines(code: &str) -> Vec<Line<'static>> {
    code.lines().map(|l| Line::from(Span::raw(l.to_string()))).collect()
}

/// syntect 使用 scope token 查找语法, 这里把常用语言标签映射到 scope
fn resolve_lang(lang: &str) -> &str {
    match lang.to_lowercase().as_str() {
        "python" | "py" => "source.python",
        "rust" | "rs" => "source.rust",
        "c" => "source.c",
        "cpp" | "c++" | "cxx" => "source.c++",
        "csharp" | "c#" | "cs" => "source.cs",
        "java" => "source.java",
        "php" => "source.php",
        "html" => "text.html.basic",
        "css" => "source.css",
        "javascript" | "js" => "source.js",
        "typescript" | "ts" => "source.ts",
        "json" => "source.json",
        "yaml" | "yml" => "source.yaml",
        "toml" => "source.toml",
        "markdown" | "md" => "text.html.markdown",
        "go" => "source.go",
        "sql" => "source.sql",
        "bash" | "sh" | "shell" => "source.shell",
        "powershell" | "ps1" => "source.powershell",
        "lua" => "source.lua",
        "ruby" | "rb" => "source.ruby",
        "perl" | "pl" => "source.perl",
        "swift" => "source.swift",
        "kotlin" | "kt" => "source.kotlin",
        "scala" => "source.scala",
        "haskell" | "hs" => "source.haskell",
        "dart" => "source.dart",
        "elixir" | "ex" => "source.elixir",
        "clojure" | "clj" => "source.clojure",
        "r" => "source.r",
        "cmake" => "source.cmake",
        "makefile" | "make" => "source.makefile",
        "dockerfile" | "docker" => "source.dockerfile",
        "nginx" => "source.nginx",
        "xml" => "text.xml",
        "vue" => "text.html.vue",
        _ => lang, // pass through for syntect to try directly
    }
}