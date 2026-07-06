use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::db::{self, SessionRow};

pub async fn print_console(root: &Path, run_id: &str) -> Result<()> {
    let db_path = root.join("runs").join("vibe_test.db");
    let row = db::load_session(&db_path, run_id)?;
    print(&row);
    render_md(root, &row)?;
    Ok(())
}

fn print(r: &SessionRow) {
    let pass = r.status == "done"
        && r.verify_failures == 0
        && parse_assertions_pass(&r.assertions_json);
    let mark = if pass { "PASS" } else { "FAIL" };
    let saved = if r.baseline_total > 0 {
        let pct = 100 - (r.tokens_total as u64 * 100 / r.baseline_total.max(1));
        format!("{}%", pct)
    } else { String::from("n/a") };

    eprintln!();
    eprintln!("================ {}  spec={}  run={} ================", mark, r.spec, r.id);
    eprintln!("  run_id      : {}", r.id);
    eprintln!("  steps       : {} / 40", r.steps);
    eprintln!("  tokens      : in {} / out {} / total {}", r.tokens_in, r.tokens_out, r.tokens_total);
    eprintln!("  baseline    : {}", if r.baseline_total > 0 { r.baseline_total.to_string() } else { String::from("(none)") });
    eprintln!("  saved       : {}", saved);
    eprintln!("  filesets    : {}", r.fileset_count);
    eprintln!("  blocks      : {}", r.block_count);
    eprintln!("  warns       : {} purpose-drift, {} cross-block, {} verify-fail",
        r.purpose_drift_warns, r.cross_block_warns, r.verify_failures);
    eprintln!("  status      : {}", r.status);
    eprintln!("  artifacts   : runs/{}/{{run.jsonl, md}}", r.id);
    eprintln!("==========================================================");
}

fn parse_assertions_pass(json: &str) -> bool {
    if json.is_empty() { return true; }
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if let Some(arr) = v.as_array() {
        let all_pass = arr.iter().all(|a| {
            a.get("pass").and_then(|p| p.as_bool()).unwrap_or(false)
        });
        return all_pass;
    }
    true
}

fn render_md(root: &Path, r: &SessionRow) -> Result<()> {
    let dir: PathBuf = root.join("runs").join(&r.id);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("report.md");
    let saved = if r.baseline_total > 0 {
        let pct = 100 - (r.tokens_total as u64 * 100 / r.baseline_total.max(1));
        format!("{}%", pct)
    } else { String::from("n/a") };
    let pass = r.status == "done" && r.verify_failures == 0 && parse_assertions_pass(&r.assertions_json);
    let mark = if pass { "PASS" } else { "FAIL" };

    let mut md = String::new();
    md.push_str(&format!("# Run {} — {}\n\n", r.id, mark));
    md.push_str(&format!("- spec: `{}`\n", r.spec));
    md.push_str(&format!("- status: `{}`\n", r.status));
    md.push_str(&format!("- steps: {}\n", r.steps));
    md.push_str(&format!("- tokens: in {} / out {} / total {}\n", r.tokens_in, r.tokens_out, r.tokens_total));
    md.push_str(&format!("- baseline estimate: {} — vibe saved {}\n", r.baseline_total, saved));
    md.push_str(&format!("- filesets: {}, blocks: {}\n", r.fileset_count, r.block_count));
    md.push_str(&format!("- warns: {} purpose-drift, {} cross-block\n", r.purpose_drift_warns, r.cross_block_warns));
    md.push_str(&format!("- verify failures: {}\n\n", r.verify_failures));
    md.push_str("## Assertions\n\n");
    if !r.assertions_json.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&r.assertions_json) {
            if let Some(arr) = v.as_array() {
                for a in arr {
                    let pass = a.get("pass").and_then(|p| p.as_bool()).unwrap_or(false);
                    let name = a.get("name").and_then(|s| s.as_str()).unwrap_or("?");
                    let detail = a.get("detail").and_then(|s| s.as_str()).unwrap_or("");
                    md.push_str(&format!("- [{}] {}  {}\n", if pass {"x"} else {" "}, name, detail));
                }
            }
        }
    } else {
        md.push_str("(no assertions)\n");
    }
    md.push_str("\nFull step transcript: `run.jsonl`.\n");
    std::fs::write(path, md)?;
    Ok(())
}