//! OpenCode-style workspace file write (html/css/js/py only).
//! Vibe block editing remains in-tree but is not registered by default.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::paths::{confine_new_path, confine_to_workspace};
use super::{Tool, ToolContext, ToolResult};

const ALLOWED_EXT: &[&str] = &["html", "htm", "css", "js", "mjs", "py"];

fn extension_ok(rel: &str) -> bool {
    let lower = rel.replace('\\', "/").to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    ALLOWED_EXT.contains(&ext)
}

fn is_safe_rel(rel: &str) -> bool {
    let lower = rel.replace('\\', "/");
    if lower.is_empty() || lower.starts_with('/') || lower.contains("..") {
        return false;
    }
    if lower.starts_with(".mooncoding/") || lower.starts_with(".vibe/") {
        return false;
    }
    true
}

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write or overwrite a project file. ONLY html/css/js/py under the current workspace. \
         Entry must be index.html at project root. Prefer creating files from scratch in a \
         NEW empty project — never copy or edit another project's files."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Workspace-relative path (e.g. index.html, styles.css, app.js, backend.py)"
                },
                "content": {
                    "type": "string",
                    "description": "Full file contents to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let started = Instant::now();
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

        if !is_safe_rel(path_str) {
            return ToolResult {
                output: format!(
                    "拒绝写入「{path_str}」：路径必须是当前工作区内的相对路径，且不能进入 .mooncoding/.vibe。"
                ),
                exit_code: 1,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            };
        }
        if !extension_ok(path_str) {
            return ToolResult {
                output: format!(
                    "拒绝写入「{path_str}」：本产品 Agent 只能写 html / css / js / py。\
                     竖屏 Web 应用入口必须是 index.html。"
                ),
                exit_code: 1,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            };
        }
        if let Some(msg) = crate::preview_backend::refuse_if_running_for_path(&ctx.workspace, path_str)
        {
            return ToolResult {
                output: msg,
                exit_code: 1,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            };
        }

        let target: PathBuf = if Path::new(path_str).is_absolute() {
            match confine_to_workspace(&ctx.workspace, Path::new(path_str)) {
                Ok(p) => p,
                Err(e) => {
                    return ToolResult {
                        output: format!("路径越界（只能访问当前项目工作区）：{e}"),
                        exit_code: 1,
                        duration_ms: started.elapsed().as_millis() as u64,
                        truncated: false,
                    };
                }
            }
        } else {
            match confine_new_path(&ctx.workspace, Path::new(path_str)) {
                Ok(p) => p,
                Err(e) => {
                    return ToolResult {
                        output: format!("路径越界（只能访问当前项目工作区）：{e}"),
                        exit_code: 1,
                        duration_ms: started.elapsed().as_millis() as u64,
                        truncated: false,
                    };
                }
            }
        };

        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolResult {
                    output: format!("无法创建目录 {}: {e}", parent.display()),
                    exit_code: 1,
                    duration_ms: started.elapsed().as_millis() as u64,
                    truncated: false,
                };
            }
        }

        let mut file = match fs::File::create(&target) {
            Ok(f) => f,
            Err(e) => {
                return ToolResult {
                    output: format!("无法写入 {}: {e}", target.display()),
                    exit_code: 1,
                    duration_ms: started.elapsed().as_millis() as u64,
                    truncated: false,
                };
            }
        };
        if let Err(e) = file.write_all(content.as_bytes()) {
            return ToolResult {
                output: format!("写入失败 {}: {e}", target.display()),
                exit_code: 1,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            };
        }

        let bytes = content.len();
        let rel = target
            .strip_prefix(&ctx.workspace)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path_str.replace('\\', "/"));

        ToolResult {
            output: format!(
                "已写入 {rel} ({bytes} bytes) — 仅当前工作区 {}\n\
                 NEXT: 若改了 UI，确认 index.html 仍是入口；需要 Python 后端时在 index.html 放启动按钮。",
                ctx.workspace.display()
            ),
            exit_code: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            truncated: false,
        }
    }
}
