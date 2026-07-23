use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// On-disk lease for the single native app runtime slot.
///
/// Survives UI/event desync and host restarts so orphaned Python children
/// can be found and reclaimed safely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLease {
    pub instance_id: String,
    pub app_id: String,
    pub pid: Option<u32>,
    pub host_pid: u32,
    pub state: String,
    pub started_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclaimReport {
    pub cleared_lease: bool,
    pub killed_pid: Option<u32>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeSandbox {
    root: PathBuf,
}

impl RuntimeSandbox {
    pub fn open(workspace: &Path) -> Result<Self> {
        let root = workspace.join(".mooncoding").join("app_runtime");
        fs::create_dir_all(&root)
            .with_context(|| format!("cannot create runtime sandbox {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lease_path(&self) -> PathBuf {
        self.root.join("current.json")
    }

    pub fn read_lease(&self) -> Result<Option<RuntimeLease>> {
        let path = self.lease_path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("cannot read runtime lease {}", path.display()))?;
        let lease = serde_json::from_str(&raw)
            .with_context(|| format!("invalid runtime lease {}", path.display()))?;
        Ok(Some(lease))
    }

    pub fn write_lease(&self, lease: &RuntimeLease) -> Result<()> {
        let path = self.lease_path();
        let tmp = self.root.join("current.json.tmp");
        let raw = serde_json::to_string_pretty(lease)?;
        fs::write(&tmp, raw)
            .with_context(|| format!("cannot write runtime lease tmp {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("cannot publish runtime lease {}", path.display()))?;
        Ok(())
    }

    pub fn clear_lease(&self) -> Result<()> {
        let path = self.lease_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("cannot remove runtime lease {}", path.display()))?;
        }
        Ok(())
    }

    pub fn update_state(&self, instance_id: &str, state: &str, pid: Option<u32>) -> Result<()> {
        let Some(mut lease) = self.read_lease()? else {
            return Ok(());
        };
        if lease.instance_id != instance_id {
            return Ok(());
        }
        lease.state = state.to_string();
        if pid.is_some() {
            lease.pid = pid;
        }
        lease.updated_at_unix = unix_now();
        self.write_lease(&lease)
    }

    /// Kill orphaned app processes left behind after a host crash / desync.
    pub fn reclaim_orphans(&self) -> Result<Option<ReclaimReport>> {
        let Some(lease) = self.read_lease()? else {
            return Ok(None);
        };

        let host_alive = process_alive(lease.host_pid);
        let child_alive = lease.pid.map(process_alive).unwrap_or(false);

        // Live host + live child: leave alone (in-memory supervisor owns it).
        if host_alive && child_alive {
            return Ok(None);
        }

        let mut killed_pid = None;
        if child_alive {
            if let Some(pid) = lease.pid {
                kill_process_tree(pid)?;
                killed_pid = Some(pid);
            }
        }

        self.clear_lease()?;
        let reason = if !host_alive && child_alive {
            "stale lease: host process gone, killed orphan app process".to_string()
        } else if !host_alive {
            "stale lease: host process gone".to_string()
        } else {
            "stale lease: app process already dead".to_string()
        };
        Ok(Some(ReclaimReport {
            cleared_lease: true,
            killed_pid,
            reason,
        }))
    }

    pub fn force_kill_lease_process(&self) -> Result<Option<u32>> {
        let Some(lease) = self.read_lease()? else {
            return Ok(None);
        };
        let Some(pid) = lease.pid else {
            self.clear_lease()?;
            return Ok(None);
        };
        if process_alive(pid) {
            kill_process_tree(pid)?;
            self.clear_lease()?;
            Ok(Some(pid))
        } else {
            self.clear_lease()?;
            Ok(None)
        }
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn current_host_pid() -> u32 {
    std::process::id()
}

pub fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        let output = Command::new("tasklist")
            .args(["/FI", &filter, "/NH"])
            .output();
        match output {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                text.lines().any(|line| {
                    let line = line.trim();
                    !line.is_empty()
                        && !line.eq_ignore_ascii_case("INFO: No tasks are running which match the specified criteria.")
                        && line.split_whitespace().any(|token| token == pid.to_string())
                })
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

pub fn kill_process_tree(pid: u32) -> Result<()> {
    if pid == 0 {
        bail!("refusing to kill pid 0");
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .with_context(|| format!("taskkill failed for pid {pid}"))?;
        if !status.success() && process_alive(pid) {
            return Err(anyhow!("taskkill returned {status} but pid {pid} is still alive"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .with_context(|| format!("kill -TERM failed for pid {pid}"))?;
        if status.success() || !process_alive(pid) {
            return Ok(());
        }
        let status = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .with_context(|| format!("kill -KILL failed for pid {pid}"))?;
        if !status.success() && process_alive(pid) {
            return Err(anyhow!("unable to kill pid {pid}"));
        }
        Ok(())
    }
}
