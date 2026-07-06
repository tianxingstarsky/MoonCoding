use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::Command;

const MAX_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MiB
const TIMEOUT_SECS: u64 = 60;

pub struct ToolResult {
    pub cwd: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub duration_ms: u64,
}

/// 在 sandbox 内执行 shell 命令; workdir 必须在 workspace_canonical 之下
pub async fn bash(command: &str, workdir: Option<&str>, workspace: &Path, vibe_path: &str) -> Result<ToolResult> {
    let workspace_canon = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
    let cwd = match workdir {
        Some(d) => {
            let p = PathBuf::from(d);
            let canon = if p.is_absolute() {
                p.canonicalize().unwrap_or(p.clone())
            } else {
                workspace_canon.join(&p).canonicalize().unwrap_or(workspace_canon.join(&p))
            };
            if !canon.starts_with(&workspace_canon) {
                return Ok(ToolResult {
                    cwd: workspace_canon.to_string_lossy().into_owned(),
                    exit_code: 126,
                    stdout: String::new(),
                    stderr: format!("refused: workdir {} outside workspace", canon.display()),
                    truncated: false, duration_ms: 0,
                });
            }
            canon
        }
        None => workspace_canon.clone(),
    };
    let cwd_display = cwd.to_string_lossy().into_owned();

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
    let vibe_dir = std::path::Path::new(vibe_path).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if !vibe_dir.is_empty() {
        let cur_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", vibe_dir, cur_path));
    }
    let child = cmd.spawn()?;
    let out = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), child.wait_with_output()).await;
    let duration_ms = start.elapsed().as_millis() as u64;
    match out {
        Ok(Ok(output)) => {
            let mut stdout = output.stdout;
            let mut stderr = output.stderr;
            let mut truncated = false;
            if stdout.len() > MAX_OUTPUT_BYTES {
                stdout.truncate(MAX_OUTPUT_BYTES);
                truncated = true;
            }
            if stderr.len() > MAX_OUTPUT_BYTES {
                stderr.truncate(MAX_OUTPUT_BYTES);
                truncated = true;
            }
            let code = output.status.code().unwrap_or(-1);
            Ok(ToolResult {
                cwd: cwd_display,
                exit_code: code,
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
                truncated, duration_ms,
            })
        }
        Ok(Err(_)) | Err(_) => Ok(ToolResult {
            cwd: cwd_display,
            exit_code: 124, stdout: String::new(),
            stderr: format!("timeout after {}s", TIMEOUT_SECS),
            truncated: false, duration_ms,
        }),
    }
}