use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Fast file pattern matching. Supports glob patterns like \"**/*.rs\" or \"src/**/*.ts\". \
         Returns matching file paths. Use when you need to find files by name patterns."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern like **/*.py"}
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");
        let re_str = glob_to_regex(pattern);
        let re = match Regex::new(&re_str) {
            Ok(r) => r,
            Err(e) => {
                return ToolResult {
                    output: format!("glob regex err: {}", e),
                    exit_code: 1,
                    duration_ms: 0,
                    truncated: false,
                }
            }
        };

        let start = std::time::Instant::now();
        let mut out = String::new();
        let mut matched = 0usize;
        let max_matches = 200usize;
        let workspace = match ctx.workspace.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                return ToolResult {
                    output: format!("workspace error: {error}"),
                    exit_code: 1,
                    duration_ms: 0,
                    truncated: false,
                }
            }
        };

        let walker = walkdir::WalkDir::new(&workspace)
            .max_depth(10)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden(e) && !is_vibe_dir(e));

        for entry in walker.flatten() {
            if matched >= max_matches {
                break;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&workspace)
                .unwrap_or(entry.path());
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if re.is_match(&rel_str) {
                out.push_str(&format!("{}\n", rel_str));
                matched += 1;
            }
        }
        if matched >= max_matches {
            out.push_str(&format!("(truncated at {} matches)", max_matches));
        }
        let ms = start.elapsed().as_millis() as u64;
        ToolResult {
            output: out,
            exit_code: 0,
            duration_ms: ms,
            truncated: matched >= max_matches,
        }
    }
}

fn glob_to_regex(pattern: &str) -> String {
    let mut out = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    out.push_str(".*");
                    i += 2;
                    // skip slash after ** if present
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                    }
                } else {
                    out.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                i += 1;
            }
            '.' => {
                out.push_str("\\.");
                i += 1;
            }
            c => {
                if c.is_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '\\' {
                    out.push(c);
                } else {
                    out.push_str(&regex::escape(&c.to_string()));
                }
                i += 1;
            }
        }
    }
    out.push('$');
    out
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
fn is_vibe_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name() == ".vibe"
}
