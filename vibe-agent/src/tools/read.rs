use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::blockgate::{
    find_blockset, format_blockset_skeleton, is_code_surface, is_plain_readable,
    is_vibe_forbidden_path, projection_len, refuse_code_read, to_posix_rel,
};
use super::{Tool, ToolContext, ToolResult};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read a file in the CURRENT project workspace only. Prefer index.html / css / js / py. \
         Never read paths outside this workspace or from another project."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {"type": "string", "description": "Workspace-relative path to a non-code file"},
                "offset": {"type": "integer", "description": "Line number to start from (1-indexed)", "default": 1},
                "limit": {"type": "integer", "description": "Max lines to read (default 2000)", "default": 2000}
            },
            "required": ["filePath"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let path_str = args.get("filePath").and_then(|v| v.as_str()).unwrap_or("");
        let offset: usize = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1) as usize;
        let limit: usize = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000)
            .max(1) as usize;

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

        // Resolve relative path for blockset lookup even if file missing on disk.
        let rel = if PathBuf::from(path_str).is_absolute() {
            PathBuf::from(path_str)
                .strip_prefix(&workspace)
                .map(|p| to_posix_rel(&p.to_string_lossy()))
                .unwrap_or_else(|_| to_posix_rel(path_str))
        } else {
            to_posix_rel(path_str)
        };

        // Hard gate: code surfaces never go through plain read.
        if is_code_surface(&rel) || !is_plain_readable(&rel) {
            let meta = find_blockset(&ctx.workspace, &rel);
            // If blocks have code but disk projection is empty/stale, say so clearly.
            let mut output = refuse_code_read(&rel, meta.as_ref());
            if let Some(m) = &meta {
                let disk = projection_len(&ctx.workspace, &rel);
                if m.code_bytes > 0 && disk == 0 {
                    output.push_str(
                        "\n【映射缺失】区块里有代码，但磁盘投影是空的。\
                         先调用 vibe action=assemble path=该文件，再 vibe read(seq) 看块——\
                         不要用 plain read，也不要以为『没写过』。\n",
                    );
                } else if m.code_bytes > 0 && disk > 0 && disk != m.code_bytes {
                    output.push_str(
                        "\n【映射可能过期】磁盘投影字节数与区块不一致。\
                         先 vibe action=assemble，再以区块为准。\n",
                    );
                }
            }
            return ToolResult {
                output,
                exit_code: 126,
                duration_ms: 0,
                truncated: false,
            };
        }

        // ui.json etc. with a mistaken empty blockset: prefer reading the real JSON file,
        // and warn that vibe should not own this path.
        if let Some(meta) = find_blockset(&ctx.workspace, &rel) {
            if is_vibe_forbidden_path(&rel) {
                // Fall through to plain read of the JSON, with a warning prefix later.
            } else {
                return ToolResult {
                    output: format!(
                        "refused: path has a vibe blockset — use vibe tools.\n\n{}",
                        format_blockset_skeleton(&meta)
                    ),
                    exit_code: 126,
                    duration_ms: 0,
                    truncated: false,
                };
            }
        }

        let requested = PathBuf::from(path_str);
        let path = match if requested.is_absolute() {
            requested.canonicalize()
        } else {
            workspace.join(&requested).canonicalize()
        } {
            Ok(path) if path.starts_with(&workspace) => path,
            Ok(path) => {
                return ToolResult {
                    output: format!("refused: {} is outside workspace", path.display()),
                    exit_code: 126,
                    duration_ms: 0,
                    truncated: false,
                }
            }
            Err(error) => {
                return ToolResult {
                    output: format!("path error: {error}"),
                    exit_code: 1,
                    duration_ms: 0,
                    truncated: false,
                }
            }
        };
        let start = std::time::Instant::now();
        match fs::read(&path) {
            Ok(bytes) => {
                let content = crate::encoding_util::decode_console_bytes(&bytes);
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let from = offset.saturating_sub(1).min(total);
                let to = (from + limit).min(total);
                let mut out = String::new();
                let end_disp = if total == 0 { 0 } else { to };
                let start_disp = if total == 0 { 0 } else { from + 1 };
                out.push_str(&format!(
                    "file: {rel}  lines {start_disp}-{end_disp}  (of {total})\n"
                ));
                if from > 0 {
                    out.push_str(&format!("(skipped {} lines)\n", from));
                }
                for (i, line) in lines[from..to].iter().enumerate() {
                    out.push_str(&format!("{:>4}: {}\n", from + 1 + i, line));
                }
                if to < total {
                    out.push_str(&format!("({} more lines)", total - to));
                }
                if content.contains('\u{FFFD}') {
                    out.push_str(
                        "\n[mooncoding] warning: file is not valid UTF-8; decoded with fallback.",
                    );
                }
                let ms = start.elapsed().as_millis() as u64;
                ToolResult {
                    output: out,
                    exit_code: 0,
                    duration_ms: ms,
                    truncated: false,
                }
            }
            Err(e) => {
                let ms = start.elapsed().as_millis() as u64;
                ToolResult {
                    output: format!("err: {}", e.kind()),
                    exit_code: 1,
                    duration_ms: ms,
                    truncated: false,
                }
            }
        }
    }
}
