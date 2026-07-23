//! Project-scoped HTML preview backend (`backend.py`).
//!
//! Lifecycle (product contract):
//! - Marked by `backend.py` at workspace root.
//! - Host / tool auto-starts on preview; may keep running while staying on the same project.
//! - Switching workspace or explicit stop destroys the process (frees the port).
//! - LLM must stop before editing `backend.py` (enforced by write gate + tool).

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app_runtime::sandbox::{kill_process_tree, process_alive};

const PORT_BASE: u32 = 18765;
const PORT_SPAN: u32 = 2000;
const SCRIPT_NAME: &str = "backend.py";
const LEASE_REL: &str = ".mooncoding/preview_backend.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewBackendLease {
    pub pid: u32,
    pub port: u16,
    pub workspace: String,
    pub script: String,
    pub api_base: String,
    pub started_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewBackendStatus {
    pub has_backend: bool,
    pub running: bool,
    pub port: u16,
    pub api_base: String,
    pub lease: Option<PreviewBackendLease>,
    pub message: String,
}

/// Stable per-workspace port in `[18765, 20764]`.
pub fn port_for_workspace(workspace: &Path) -> u16 {
    let key = normalize_workspace_key(workspace);
    let hash = fnv1a32(key.as_bytes());
    (PORT_BASE + (hash % PORT_SPAN)) as u16
}

pub fn api_base_for_port(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn lease_path(workspace: &Path) -> PathBuf {
    workspace.join(LEASE_REL)
}

pub fn backend_script(workspace: &Path) -> PathBuf {
    workspace.join(SCRIPT_NAME)
}

pub fn has_backend(workspace: &Path) -> bool {
    backend_script(workspace).is_file()
}

pub fn read_lease(workspace: &Path) -> Result<Option<PreviewBackendLease>> {
    let path = lease_path(workspace);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("cannot read preview backend lease {}", path.display()))?;
    let lease: PreviewBackendLease = serde_json::from_str(&raw)
        .with_context(|| format!("invalid preview backend lease {}", path.display()))?;
    Ok(Some(lease))
}

pub fn write_lease(workspace: &Path, lease: &PreviewBackendLease) -> Result<()> {
    let path = lease_path(workspace);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(lease)?;
    fs::write(&tmp, raw).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("cannot publish {}", path.display()))?;
    Ok(())
}

pub fn clear_lease(workspace: &Path) -> Result<()> {
    let path = lease_path(workspace);
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("cannot remove preview backend lease {}", path.display()))?;
    }
    Ok(())
}

pub fn status(workspace: &Path) -> Result<PreviewBackendStatus> {
    let port = port_for_workspace(workspace);
    let api_base = api_base_for_port(port);
    let has = has_backend(workspace);
    let lease = read_lease(workspace)?;
    let running = match &lease {
        Some(l) if process_alive(l.pid) => true,
        Some(_) => {
            // Stale lease — drop it.
            let _ = clear_lease(workspace);
            false
        }
        None => false,
    };
    let lease = if running { read_lease(workspace)? } else { None };
    let message = if !has {
        "no backend.py".to_string()
    } else if running {
        format!(
            "running pid={} port={}",
            lease.as_ref().map(|l| l.pid).unwrap_or(0),
            lease.as_ref().map(|l| l.port).unwrap_or(port)
        )
    } else {
        "backend.py present, not running".to_string()
    };
    Ok(PreviewBackendStatus {
        has_backend: has,
        running,
        port,
        api_base,
        lease,
        message,
    })
}

/// True when a live lease process exists for this workspace.
pub fn is_running(workspace: &Path) -> bool {
    status(workspace).map(|s| s.running).unwrap_or(false)
}

/// Stop the preview backend for this workspace (lease pid). Idempotent.
pub fn stop(workspace: &Path) -> Result<PreviewBackendStatus> {
    if let Some(lease) = read_lease(workspace)? {
        if process_alive(lease.pid) {
            kill_process_tree(lease.pid)?;
            // Brief wait so port is released before callers restart.
            for _ in 0..20 {
                if !process_alive(lease.pid) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    clear_lease(workspace)?;
    let mut st = status(workspace)?;
    st.message = "stopped".to_string();
    Ok(st)
}

/// Start backend.py if present and not already running. Returns status.
pub fn ensure_started(workspace: &Path) -> Result<PreviewBackendStatus> {
    if !has_backend(workspace) {
        bail!("no backend.py in workspace");
    }
    let current = status(workspace)?;
    if current.running {
        return Ok(current);
    }
    start_new(workspace)
}

fn start_new(workspace: &Path) -> Result<PreviewBackendStatus> {
    let script = backend_script(workspace);
    if !script.is_file() {
        bail!("backend.py not found");
    }
    let port = port_for_workspace(workspace);
    let api_base = api_base_for_port(port);
    let child = spawn_backend(&script, workspace, port)?;
    let pid = child.id();
    // Intentionally detach: on Windows, Drop for Child kills the process.
    std::mem::forget(child);

    let lease = PreviewBackendLease {
        pid,
        port,
        workspace: normalize_workspace_key(workspace),
        script: SCRIPT_NAME.to_string(),
        api_base: api_base.clone(),
        started_at_unix: unix_now(),
    };
    write_lease(workspace, &lease)?;

    // Brief settle so process_alive / tasklist sees the new pid.
    for _ in 0..20 {
        if process_alive(pid) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    Ok(PreviewBackendStatus {
        has_backend: true,
        running: process_alive(pid),
        port,
        api_base,
        lease: Some(lease),
        message: format!("started pid={pid} port={port}"),
    })
}

fn spawn_backend(script: &Path, workspace: &Path, port: u16) -> Result<Child> {
    let mut last_err = None;
    // Windows: prefer `python` / `py` — `python3` is often the Store stub that exits immediately.
    #[cfg(windows)]
    let candidates: &[&str] = &["python", "py", "python3"];
    #[cfg(not(windows))]
    let candidates: &[&str] = &["python3", "python"];

    for python in candidates {
        let mut cmd = Command::new(python);
        // `py -3 script.py` on Windows.
        if python == &"py" {
            cmd.arg("-3");
        }
        cmd.arg(script)
            .current_dir(workspace)
            .env("MOONCODING_BACKEND_PORT", port.to_string())
            .env("MOONCODING_BACKEND_HOST", "127.0.0.1")
            .env("MOONCODING_API_BASE", api_base_for_port(port))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                // Confirm the interpreter actually stayed up (filters Store stubs).
                thread::sleep(Duration::from_millis(150));
                if process_alive(pid) {
                    return Ok(child);
                }
                // Stub / immediate exit — try next candidate.
                let _ = kill_process_tree(pid);
                last_err = Some(std::io::Error::other(format!(
                    "{python} pid={pid} exited immediately"
                )));
            }
            Err(err) => last_err = Some(err),
        }
    }
    Err(anyhow!(
        "failed to spawn python for backend.py: {}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".into())
    ))
}

fn normalize_workspace_key(workspace: &Path) -> String {
    let path = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut key = path.to_string_lossy().replace('\\', "/").to_lowercase();
    // Match Qt QFileInfo::canonicalFilePath (no Windows extended-length prefix).
    if let Some(rest) = key.strip_prefix("//?/") {
        key = rest.to_string();
    } else if let Some(rest) = key.strip_prefix("/?/") {
        key = rest.to_string();
    }
    key
}

fn fnv1a32(data: &[u8]) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in data {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Refuse mutating `backend.py` while the preview backend is live.
pub fn refuse_if_running_for_path(workspace: &Path, rel_path: &str) -> Option<String> {
    let norm = rel_path.replace('\\', "/");
    let name = Path::new(&norm)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !name.eq_ignore_ascii_case(SCRIPT_NAME) {
        return None;
    }
    if !is_running(workspace) {
        return None;
    }
    Some(
        "preview backend is running — call tool preview_backend action=stop before editing backend.py \
         (keeps the port free and avoids half-updated servers)."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn port_is_stable_and_in_range() {
        let a = port_for_workspace(Path::new("/tmp/proj-one"));
        let b = port_for_workspace(Path::new("/tmp/proj-one"));
        let c = port_for_workspace(Path::new("/tmp/proj-two"));
        assert_eq!(a, b);
        assert!(a >= 18765 && a < 18765 + 2000);
        // Different paths usually differ; allow rare hash collision.
        let _ = c;
    }

    #[test]
    fn status_without_backend() {
        let dir = tempfile_dir("pb-none");
        let st = status(&dir).expect("status");
        assert!(!st.has_backend);
        assert!(!st.running);
    }

    #[test]
    fn ensure_start_stop_roundtrip() {
        let dir = tempfile_dir("pb-run");
        let script = dir.join(SCRIPT_NAME);
        let mut f = fs::File::create(&script).expect("create");
        // Stay alive until killed; bind nothing (port ownership is host contract for now).
        writeln!(
            f,
            "import os, time\nprint('READY', os.environ.get('MOONCODING_API_BASE',''), flush=True)\nwhile True:\n    time.sleep(1)\n"
        )
        .expect("write");
        drop(f);

        let started = ensure_started(&dir).expect("start");
        assert!(started.running, "{}", started.message);
        assert!(is_running(&dir));

        let again = ensure_started(&dir).expect("ensure");
        assert_eq!(again.lease.as_ref().map(|l| l.pid), started.lease.as_ref().map(|l| l.pid));

        let stopped = stop(&dir).expect("stop");
        assert!(!stopped.running);
        assert!(!is_running(&dir));
        assert!(refuse_if_running_for_path(&dir, "backend.py").is_none());
    }

    #[test]
    fn refuse_edit_while_running() {
        let dir = tempfile_dir("pb-gate");
        let script = dir.join(SCRIPT_NAME);
        fs::write(
            &script,
            "import time\nwhile True:\n    time.sleep(1)\n",
        )
        .expect("write");
        let _ = ensure_started(&dir).expect("start");
        let msg = refuse_if_running_for_path(&dir, "backend.py").expect("refuse");
        assert!(msg.contains("preview_backend"));
        assert!(refuse_if_running_for_path(&dir, "index.html").is_none());
        let _ = stop(&dir);
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "mooncoding-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("tmpdir");
        root
    }
}
