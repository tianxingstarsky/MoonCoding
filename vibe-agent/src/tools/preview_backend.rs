//! Tool: preview_backend — control workspace `backend.py` lifecycle.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolResult};
use crate::preview_backend;

pub struct PreviewBackendTool;

#[async_trait]
impl Tool for PreviewBackendTool {
    fn name(&self) -> &str {
        "preview_backend"
    }

    fn description(&self) -> &str {
        "Control the project HTML preview backend (workspace-root backend.py). \
         Actions: status, ensure (start if needed), stop. \
         Host auto-starts on Apps preview; process stays alive while on the SAME project \
         (background OK when switching Chat/Tree). Switching project kills it. \
         ALWAYS call stop before editing/replacing backend.py, changing bind port, or when \
         verification complains the port is busy. Prefer stop over kill via shell."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "ensure", "start", "stop"],
                    "description": "status=inspect; ensure/start=auto-start if backend.py exists; stop=kill and free port"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let started = std::time::Instant::now();
        let action = args["action"].as_str().unwrap_or("status");
        let result = match action {
            "status" => preview_backend::status(&ctx.workspace)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::to_string_pretty(&s).map_err(|e| e.to_string())),
            "ensure" | "start" => preview_backend::ensure_started(&ctx.workspace)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    let mut body = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
                    body.push_str(
                        "\n\nNEXT: page may use window.__MOONCODING_API_BASE__ or env MOONCODING_API_BASE. \
                         Before editing backend.py, call preview_backend action=stop.",
                    );
                    Ok(body)
                }),
            "stop" => preview_backend::stop(&ctx.workspace)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    let mut body = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
                    body.push_str("\n\nNEXT: port freed. Safe to edit backend.py or restart with action=ensure.");
                    Ok(body)
                }),
            other => Err(format!("unknown preview_backend action: {other}")),
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        match result {
            Ok(output) => ToolResult {
                output,
                exit_code: 0,
                duration_ms,
                truncated: false,
            },
            Err(output) => ToolResult {
                output,
                exit_code: 1,
                duration_ms,
                truncated: false,
            },
        }
    }
}
