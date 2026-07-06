use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::Utc;

pub struct Session {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub runs_root: PathBuf,
    pub workspace: PathBuf,
    pub jsonl_path: PathBuf,
    pub jsonl_lock: std::sync::Mutex<()>,
}

impl Session {
    pub fn new(runs_root: &Path, id: &str) -> Result<Self> {
        let run_dir = runs_root.join(id);
        let workspace = run_dir.join("workspace");
        let jsonl_path = run_dir.join("run.jsonl");
        fs::create_dir_all(&run_dir)?;
        fs::create_dir_all(&workspace)?;
        Ok(Self { id: id.to_string(), runs_root: runs_root.to_path_buf(), workspace, jsonl_path, jsonl_lock: std::sync::Mutex::new(()) })
    }

    /// 把 events (JSON) 按 jsonl 追加写入, 锁防止并发交错
    pub fn append_event(&self, ev: &serde_json::Value) -> Result<()> {
        let _g = self.jsonl_lock.lock().unwrap();
        let mut f = fs::OpenOptions::new().create(true).append(true).open(&self.jsonl_path)?;
        let line = serde_json::to_string(ev)?;
        writeln!(f, "{}", line)?;
        Ok(())
    }

    pub fn log_step(&self, step: u64, role: &str, content: &str, tool_call: Option<&str>, tool_result: Option<&str>) -> Result<()> {
        let ev = serde_json::json!({
            "t": Utc::now().to_rfc3339(),
            "step": step,
            "role": role,
            "content": content,
            "tool_call": tool_call,
            "tool_result": tool_result,
        });
        self.append_event(&ev)
    }

    /// 把 fixture/workspace 起步文件拷到 workspace 内, 作为 AI 的开局
    pub fn seed_from(&self, fixture_workspace: &Path) -> Result<()> {
        if !fixture_workspace.exists() { return Ok(()); }
        copy_recursively(fixture_workspace, &self.workspace)?;
        Ok(())
    }
}

/// 把上一次 run 的 workspace 作为下一次的继承工作区
pub fn inherit_workspace_into(target: &Path, prev_workspace: &Path) -> Result<()> {
    if !prev_workspace.exists() { return Ok(()); }
    copy_recursively(prev_workspace, target)?;
    Ok(())
}

fn copy_recursively(src: &Path, dst: &Path) -> Result<()> {
    if src.is_file() {
        fs::copy(src, dst)?;
        return Ok(());
    }
    if !dst.exists() { fs::create_dir_all(dst)?; }
    for e in fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        // 跳过 .vibe 历史以让下一个 spec 重新初始化？-- 不跳,保留区块集让 AI 接续修
        let target = dst.join(&name);
        if e.file_type()?.is_dir() { copy_recursively(&e.path(), &target)?; }
        else { fs::copy(e.path(), target)?; }
    }
    Ok(())
}

pub fn now_run_id() -> String {
    let now = Utc::now();
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() % 100000)
        .unwrap_or(0);
    format!("{}-{:05}", now.format("%Y%m%d-%H%M"), secs)
}