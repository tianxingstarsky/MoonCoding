//! Hard gates so the LLM cannot treat assembled source files as the edit surface.
//! Blocksets under `.vibe/` are the source of truth; source paths are projections.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BlocksetMeta {
    pub path: String,
    pub ulid: String,
    pub rev: u64,
    pub purpose: String,
    pub block_count: usize,
    pub code_bytes: u64,
    pub summaries: Vec<(usize, String)>, // seq, summary
    pub index_path: PathBuf,
    pub dir: PathBuf,
}

/// Paths historically managed only through vibe blocks.
/// When vibe is disabled (default), html/css/js/py are ordinary files.
pub fn is_code_surface(rel_or_name: &str) -> bool {
    if !vibe_mode_enabled() {
        return false;
    }
    let lower = rel_or_name.replace('\\', "/").to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "py" | "rs" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "c" | "js" | "ts" | "tsx" | "jsx"
            | "go" | "java" | "kt" | "cs"
    )
}

fn vibe_mode_enabled() -> bool {
    std::env::var("MOONCODING_ENABLE_VIBE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Extensions the product agent may freely read/write (OpenCode-style).
pub fn is_web_project_file(rel_or_name: &str) -> bool {
    let lower = rel_or_name.replace('\\', "/").to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    matches!(ext, "html" | "htm" | "css" | "js" | "mjs" | "py")
}

/// JSON/UI/manifests must NOT be vibe-block managed (LLM keeps breaking ui.json).
pub fn is_vibe_forbidden_path(rel_or_name: &str) -> bool {
    let lower = to_posix_rel(rel_or_name).to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(lower.as_str());
    name == "ui.json"
        || name == "app.json"
        || name == "package.json"
        || name == "cargo.toml"
        || name == "cmakelists.txt"
        || lower.ends_with(".qss")
        || lower.ends_with(".md")
}

/// Allow-list for the plain `read` tool.
pub fn is_plain_readable(rel_or_name: &str) -> bool {
    let lower = rel_or_name.replace('\\', "/").to_ascii_lowercase();
    if lower.contains("/.vibe/") || lower.starts_with(".vibe/") {
        return false;
    }
    if !vibe_mode_enabled() && is_web_project_file(rel_or_name) {
        return true;
    }
    if is_code_surface(rel_or_name) {
        return false;
    }
    true
}

pub fn to_posix_rel(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Look up a blockset by workspace-relative POSIX path.
pub fn find_blockset(workspace: &Path, rel_path: &str) -> Option<BlocksetMeta> {
    let want = to_posix_rel(rel_path);
    let vibe_root = workspace.join(".vibe");
    let entries = fs::read_dir(&vibe_root).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".vibe") {
            continue;
        }
        let ulid = name.trim_end_matches(".vibe").to_string();
        let dir = entry.path();
        let index_path = dir.join("index.json");
        let Ok(text) = fs::read_to_string(&index_path) else {
            continue;
        };
        let Ok(idx) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let path = idx
            .pointer("/fileset/path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if to_posix_rel(path) != want {
            continue;
        }
        let rev = idx.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
        let purpose = idx
            .pointer("/fileset/purpose")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut summaries = Vec::new();
        if let Some(blocks) = idx.get("blocks").and_then(|v| v.as_array()) {
            for b in blocks {
                let seq = b.get("seq").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let summary = b
                    .pointer("/tail/summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                summaries.push((seq, summary));
            }
        }
        let block_count = summaries.len();
        let code_bytes = fs::metadata(dir.join("blocks.vib.code"))
            .map(|m| m.len())
            .unwrap_or(0);
        return Some(BlocksetMeta {
            path: want,
            ulid,
            rev,
            purpose,
            block_count,
            code_bytes,
            summaries,
            index_path,
            dir,
        });
    }
    None
}

pub fn format_blockset_skeleton(meta: &BlocksetMeta) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "BLOCKSET (source of truth) for `{}`\n\
         rev={}  blocks={}  code_bytes={}  purpose=\"{}\"\n\
         On-disk source is only an assemble projection — edit via vibe tool fields.\n",
        meta.path, meta.rev, meta.block_count, meta.code_bytes, meta.purpose
    ));
    if meta.block_count == 0 {
        out.push_str("(no blocks yet — use vibe insert, or vibe split if migrating existing text)\n");
    } else {
        out.push_str("Blocks (annotations — call vibe action=read seq=N for one block):\n");
        for (seq, summary) in &meta.summaries {
            out.push_str(&format!("  [{seq:>2}] {summary}\n"));
        }
    }
    out
}

pub fn refuse_code_read(rel: &str, meta: Option<&BlocksetMeta>) -> String {
    let header = format!(
        "【违规】不要用 plain `read` 读代码文件 `{rel}`。\n\
         规则: 非必要不得使用 read；代码只通过 vibe 工具操作区块。\n\
         若只想确认改动是否落盘: 先 vibe action=assemble（或改块后由程序自动映射），\
         再用 vibe action=overview / action=read seq=N 看区块——不要读空的/过期的投影文件。\n\
         正确路径: overview → read(seq) → replace/insert →（程序 assemble）→ verify。\n"
    );
    match meta {
        Some(m) => format!("{header}\n{}", format_blockset_skeleton(m)),
        None => format!(
            "{header}\n尚无区块集。下一步: vibe action=new 或 action=split（填工具字段，勿敲命令行）。"
        ),
    }
}

pub fn refuse_code_write(rel: &str) -> String {
    format!(
        "【违规】不能直接写代码文件 `{rel}`。\
         用 vibe replace/insert/drop 改区块；程序负责 assemble 映射到磁盘投影。"
    )
}

pub fn refuse_vibe_on_non_code(rel: &str) -> String {
    format!(
        "refused: `{rel}` 不是代码区块面（如 ui.json/app.json）。\
         用 plain read 看 JSON；用 apps update 的 ui_schema 改 UI。不要对这类文件 vibe new/split。"
    )
}

/// Projection path bytes on disk (0 if missing).
pub fn projection_len(workspace: &Path, rel: &str) -> u64 {
    let path = workspace.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
