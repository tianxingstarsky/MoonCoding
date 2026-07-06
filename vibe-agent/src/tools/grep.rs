use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolContext, ToolResult};

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }
    fn description(&self) -> &str {
        "Fast content search tool. Searches file contents using regular expressions. \
         Supports full regex syntax. Returns file paths and line numbers with matching lines. \
         Use when you need to find files containing specific patterns."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Regex pattern to search for"},
                "path": {"type": "string", "description": "Directory to search in (defaults to workspace)", "default": ""},
                "include": {"type": "string", "description": "File glob to filter, e.g. *.rs or *.{ts,tsx}", "default": "*"}
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        let dir = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let include = args.get("include").and_then(|v| v.as_str()).unwrap_or("*");

        let search_dir = if dir.is_empty() { ctx.workspace.clone() } else { PathBuf::from(dir) };
        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return ToolResult { output: format!("regex err: {}", e), exit_code: 1, duration_ms: 0, truncated: false },
        };

        let start = std::time::Instant::now();
        let mut out = String::new();
        let mut matched = 0usize;
        let max_lines = 200usize;

        let mut walker = walkdir::WalkDir::new(&search_dir)
            .max_depth(10)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden(e) && !is_vibe_dir(e));

        for entry in walker.flatten() {
            if matched >= max_lines { break; }
            if !entry.file_type().is_file() { continue; }
            if !simple_glob_match(include, entry.file_name().to_string_lossy().as_ref()) { continue; }
            let content = match fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            for (line_no, line) in content.lines().enumerate() {
                if matched >= max_lines { break; }
                if re.is_match(line) {
                    let rel = entry.path().strip_prefix(&search_dir).unwrap_or(entry.path());
                    out.push_str(&format!("{}:{} {}\n", rel.display(), line_no + 1, line));
                    matched += 1;
                }
            }
        }
        if matched >= max_lines {
            out.push_str(&format!("(truncated at {} matches)", max_lines));
        }
        let ms = start.elapsed().as_millis() as u64;
        ToolResult { output: out, exit_code: 0, duration_ms: ms, truncated: matched >= max_lines }
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false)
}

fn is_vibe_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name() == ".vibe"
}

fn simple_glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" { return true; }
    let re_str = regex::escape(pattern).replace("\\*", ".*");
    Regex::new(&format!("^{}$", re_str)).map(|r| r.is_match(name)).unwrap_or(true)
}