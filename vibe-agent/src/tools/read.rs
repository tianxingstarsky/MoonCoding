use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolContext, ToolResult};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }
    fn description(&self) -> &str {
        "Read a file from the local filesystem. Returns up to 2000 lines from the given offset. \
         Use the grep tool to find specific content in large files. \
         Call this tool in parallel when you know there are multiple files you want to read."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {"type": "string", "description": "Absolute path to the file to read"},
                "offset": {"type": "integer", "description": "Line number to start from (1-indexed)", "default": 1},
                "limit": {"type": "integer", "description": "Max lines to read (default 2000)", "default": 2000}
            },
            "required": ["filePath"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let path_str = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("");
        let offset: usize = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
        let limit: usize = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000).max(1) as usize;

        let path = PathBuf::from(path_str);
        let start = std::time::Instant::now();
        match fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let from = offset.saturating_sub(1);
                let to = (from + limit).min(total);
                let mut out = String::new();
                if from > 0 {
                    out.push_str(&format!("(skipped {} lines)\n", from));
                }
                for (i, line) in lines[from..to].iter().enumerate() {
                    out.push_str(&format!("{:>4}: {}\n", offset + i, line));
                }
                if to < total {
                    out.push_str(&format!("({} more lines)", total - to));
                }
                let ms = start.elapsed().as_millis() as u64;
                ToolResult { output: out, exit_code: 0, duration_ms: ms, truncated: false }
            }
            Err(e) => {
                let ms = start.elapsed().as_millis() as u64;
                ToolResult { output: format!("err: {}", e), exit_code: 1, duration_ms: ms, truncated: false }
            }
        }
    }
}