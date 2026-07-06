use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolResult};

const MAX_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MiB
const TIMEOUT_SECS: u64 = 60;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }
    fn description(&self) -> &str {
        "Execute a shell command inside the workspace sandbox. \
         Commands run with workspace as cwd. Output is capped at 1 MiB. \
         Use for running tests, build commands, linters, and any non-vibe CLI utilities."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command line"},
                "workdir": {"type": "string", "description": "Subdirectory inside workspace (default: workspace root)"},
                "timeout": {"type": "integer", "description": "Timeout in ms (default 60000)"}
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let workdir_str = args.get("workdir").and_then(|v| v.as_str());
        let timeout_ms = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(TIMEOUT_SECS * 1000);

        let workspace_canon = ctx.workspace.canonicalize().unwrap_or_else(|_| ctx.workspace.clone());
        let cwd = match workdir_str {
            Some(d) => {
                let p = PathBuf::from(d);
                let canon = if p.is_absolute() {
                    p.canonicalize().unwrap_or(p.clone())
                } else {
                    workspace_canon.join(&p).canonicalize().unwrap_or(workspace_canon.join(&p))
                };
                if !canon.starts_with(&workspace_canon) {
                    return ToolResult {
                        output: format!("refused: workdir {} outside workspace", canon.display()),
                        exit_code: 126, duration_ms: 0, truncated: false,
                    };
                }
                canon
            }
            None => workspace_canon.clone(),
        };

        let start = Instant::now();
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("powershell");
            c.arg("-NoProfile").arg("-Command").arg(command);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(command);
            c
        };
        cmd.current_dir(&cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        cmd.env("VIBE_TEST", "1");
        // prepend vibe binary directory to PATH
        if let Some(vibe_dir) = ctx.vibe_exe.parent() {
            let cur_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{};{}", vibe_dir.display(), cur_path));
        }

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult { output: format!("spawn err: {}", e), exit_code: -1, duration_ms: 0, truncated: false },
        };
        let out = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), child.wait_with_output()).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match out {
            Ok(Ok(output)) => {
                let mut stdout = output.stdout;
                let mut stderr = output.stderr;
                let mut truncated = false;
                if stdout.len() > MAX_OUTPUT_BYTES { stdout.truncate(MAX_OUTPUT_BYTES); truncated = true; }
                if stderr.len() > MAX_OUTPUT_BYTES { stderr.truncate(MAX_OUTPUT_BYTES); truncated = true; }
                let code = output.status.code().unwrap_or(-1);
                let mut text = String::new();
                text.push_str(&format!("cwd: {}\n", cwd.display()));
                text.push_str(&format!("exit {}\n", code));
                if !stdout.is_empty() { text.push_str(&format!("--- stdout ---\n{}\n", String::from_utf8_lossy(&stdout))); }
                if !stderr.is_empty() { text.push_str(&format!("--- stderr ---\n{}\n", String::from_utf8_lossy(&stderr))); }
                if truncated { text.push_str("(output truncated at 1 MiB)\n"); }
                ToolResult { output: text, exit_code: code, duration_ms, truncated }
            }
            Ok(Err(e)) => ToolResult { output: format!("spawn err: {}", e), exit_code: -1, duration_ms, truncated: false },
            Err(_) => ToolResult { output: format!("timeout after {}ms", timeout_ms), exit_code: 124, duration_ms, truncated: false },
        }
    }
}