use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolResult};

/// 把 vibe CLI 12 个命令包装成整洁的工具接口。
/// AI 只需传 action + path + args, 程序自动组装 stdin JSON 并 spawn vibe 子进程。
pub struct VibeTool;

#[async_trait]
impl Tool for VibeTool {
    fn name(&self) -> &str { "vibe" }
    fn description(&self) -> &str {
        "Execute a vibe CLI command to interact with block-set protocol. \
         Supported actions: new, split, overview, peek, read, meta, insert, replace, drop, assemble, verify, deps, lookup. \
         For insert/replace/drop, include 'rev', 'code'(for insert/replace), 'tail'(for insert/replace), \
         and 'purpose_decision'(required). The tool auto-builds stdin JSON and calls vibe subprocess."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["new", "split", "overview", "peek", "read", "meta", "insert", "replace", "drop", "assemble", "verify", "deps", "lookup", "info"]
                },
                "path": {"type": "string", "description": "POSIX path to the blockset (e.g. src/server.py)"},
                "args": {"type": "object", "description": "Action-specific arguments. For insert/replace/drop: must include 'rev' and 'purpose_decision'. For insert/replace: 'code' and 'tail'"}
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let extra = args.get("args").cloned().unwrap_or(json!({}));

        let start = Instant::now();
        let result = match action {
            "new" => vibe_new(path, &extra, ctx).await,
            "split" => vibe_split(path, &extra, ctx).await,
            "overview" => vibe_overview(path, ctx).await,
            "peek" => vibe_peek(path, &extra, ctx).await,
            "read" => vibe_read(path, &extra, ctx).await,
            "meta" => vibe_meta(path, &extra, ctx).await,
            "insert" => vibe_insert(path, &extra, ctx).await,
            "replace" => vibe_replace(path, &extra, ctx).await,
            "drop" => vibe_drop(path, &extra, ctx).await,
            "assemble" => vibe_assemble(path, &extra, ctx).await,
            "verify" => vibe_verify(path, ctx).await,
            "deps" => vibe_deps(path, ctx).await,
            "lookup" => vibe_lookup(path, &extra, ctx).await,
            "info" => vibe_info(path, ctx).await,
            _ => (String::from("unknown action"), 1),
        };
        let duration_ms = start.elapsed().as_millis() as u64;
        ToolResult { output: result.0, exit_code: result.1, duration_ms, truncated: false }
    }
}

async fn run_vibe(args: &[&str], stdin_text: Option<&str>, ctx: &ToolContext) -> (String, i32) {
    let mut cmd = Command::new(&ctx.vibe_exe);
    for a in args { cmd.arg(a); }
    cmd.current_dir(&ctx.workspace);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    if let Some(vibe_dir) = ctx.vibe_exe.parent() {
        let cur_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", vibe_dir.display(), cur_path));
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (format!("spawn err: {}", e), -1),
    };
    if let Some(txt) = stdin_text {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(txt.as_bytes()).await;
        }
    }
    // drop stdin so child can finish reading
    drop(child.stdin.take());
    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => return (format!("wait err: {}", e), -1),
    };
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut text = String::new();
    if !stdout.is_empty() { text.push_str(&stdout); }
    if !stderr.is_empty() {
        if !text.is_empty() { text.push('\n'); }
        text.push_str(&stderr);
    }
    (text, code)
}

// ── helpers ──

fn purpose_decision(extra: &Value) -> Option<String> {
    let pd = extra.get("purpose_decision")?;
    if let Some(c) = pd.get("changed").and_then(|v| v.as_str()) {
        Some(format!("{{\"changed\":\"{}\"}}", c))
    } else if let Some(u) = pd.get("unchanged").and_then(|v| v.as_bool()) {
        if u { Some("{\"unchanged\":true}".to_string()) } else { None }
    } else { None }
}

async fn vibe_new(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let name = extra.get("name").and_then(|v| v.as_str()).unwrap_or(path);
    let lang = extra.get("lang").and_then(|v| v.as_str()).unwrap_or("python");
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    run_vibe(&["new", path, "--name", name, "--lang", lang, "--purpose", purpose], None, ctx).await
}

async fn vibe_split(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    run_vibe(&["split", path, "--purpose", purpose], None, ctx).await
}

async fn vibe_overview(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["overview", path], None, ctx).await
}

async fn vibe_peek(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let seq = extra.get("seq").and_then(|v| v.as_u64()).unwrap_or(1).to_string();
    run_vibe(&["peek", path, &seq], None, ctx).await
}

async fn vibe_read(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let seq = extra.get("seq").and_then(|v| v.as_u64()).unwrap_or(1).to_string();
    run_vibe(&["read", path, &seq], None, ctx).await
}

async fn vibe_meta(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    run_vibe(&["meta", path, "--purpose", purpose], None, ctx).await
}

async fn vibe_assemble(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let out = extra.get("out").and_then(|v| v.as_str());
    let mut args = vec!["assemble", path];
    let out_str;
    if let Some(o) = out { args.push("-o"); args.push(o); out_str = o.to_string(); } else { out_str = path.to_string(); }
    let _ = out_str;
    run_vibe(&args, None, ctx).await
}

async fn vibe_verify(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["verify", path], None, ctx).await
}

async fn vibe_deps(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["deps", path], None, ctx).await
}

async fn vibe_lookup(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let line = extra.get("line").and_then(|v| v.as_u64()).unwrap_or(1).to_string();
    run_vibe(&["lookup", path, &line], None, ctx).await
}

async fn vibe_info(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["info", path], None, ctx).await
}

async fn vibe_insert(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let rev = extra.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let after = extra.get("after").and_then(|v| v.as_u64()).unwrap_or(0);
    let code = extra.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let tail = extra.get("tail").cloned().unwrap_or(json!({"summary": "", "purpose": ""}));
    let pd = purpose_decision(extra);
    if pd.is_none() { return ("purpose_decision required".into(), 1); }
    let payload = json!({
        "rev": rev, "after": after, "code": code, "tail": tail, "purpose_decision": serde_json::from_str::<Value>(&pd.unwrap()).unwrap_or(json!({}))
    });
    let stdin_str = serde_json::to_string(&payload).unwrap_or_default();
    run_vibe(&["insert", path], Some(&stdin_str), ctx).await
}

async fn vibe_replace(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let rev = extra.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let seq = extra.get("seq").and_then(|v| v.as_u64()).unwrap_or(0);
    let code = extra.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let tail = extra.get("tail").cloned().unwrap_or(json!({"summary": "", "purpose": ""}));
    let pd = purpose_decision(extra);
    if pd.is_none() { return ("purpose_decision required".into(), 1); }
    let payload = json!({
        "rev": rev, "seq": seq, "code": code, "tail": tail, "purpose_decision": serde_json::from_str::<Value>(&pd.unwrap()).unwrap_or(json!({}))
    });
    let stdin_str = serde_json::to_string(&payload).unwrap_or_default();
    run_vibe(&["replace", path], Some(&stdin_str), ctx).await
}

async fn vibe_drop(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let rev = extra.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let seq = extra.get("seq").and_then(|v| v.as_u64()).unwrap_or(0);
    let pd = purpose_decision(extra);
    if pd.is_none() { return ("purpose_decision required".into(), 1); }
    let payload = json!({
        "rev": rev, "seq": seq, "purpose_decision": serde_json::from_str::<Value>(&pd.unwrap()).unwrap_or(json!({}))
    });
    let stdin_str = serde_json::to_string(&payload).unwrap_or_default();
    run_vibe(&["drop", path], Some(&stdin_str), ctx).await
}