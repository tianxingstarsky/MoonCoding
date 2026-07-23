use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Component, Path};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::blockgate::{
    find_blockset, is_vibe_forbidden_path, projection_len, refuse_vibe_on_non_code,
};
use super::{Tool, ToolContext, ToolResult};

const VIBE_TIMEOUT_SECS: u64 = 120;

/// 把 vibe CLI 12 个命令包装成整洁的工具接口。
/// AI 只需传 action + path + args, 程序自动组装 stdin JSON 并 spawn vibe 子进程。
pub struct VibeTool;

#[async_trait]
impl Tool for VibeTool {
    fn name(&self) -> &str {
        "vibe"
    }
    fn description(&self) -> &str {
        "MoonCoding block workspace (program-guided). You do NOT edit source files and you do NOT \
         invent shell/CLI lines. Call this tool with structured fields; the program mutates blocks \
         and assembles the projection. Typical loop: overview → read(seq) → replace|insert|drop → verify. \
         Every result ends with NEXT steps the program allows. Fields may be top-level (preferred)."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Program operation (not a shell command)",
                    "enum": ["new", "split", "overview", "peek", "read", "meta", "insert", "replace", "drop", "assemble", "verify", "deps", "lookup", "info"]
                },
                "path": {
                    "type": "string",
                    "description": "Managed projection path, e.g. apps/demo/main.py"
                },
                "seq": {"type": "integer", "description": "Block number from overview (for read/peek/replace/drop)"},
                "after": {"type": "integer", "description": "Insert after this seq (0 = front)"},
                "rev": {"type": "integer", "description": "Optional; program fills from blockset if omitted"},
                "code": {"type": "string", "description": "Full replacement/insert body for one block"},
                "tail": {
                    "type": "object",
                    "description": "Block annotation {summary, purpose}",
                    "properties": {
                        "summary": {"type": "string"},
                        "purpose": {"type": "string"}
                    }
                },
                "purpose": {"type": "string", "description": "File purpose for new/split, or purpose_decision.changed"},
                "purpose_decision": {
                    "type": "object",
                    "description": "Optional; defaults to {unchanged:true}",
                    "properties": {
                        "unchanged": {"type": "boolean"},
                        "changed": {"type": "string"}
                    }
                },
                "name": {"type": "string"},
                "lang": {"type": "string", "enum": ["python", "rust", "cpp"]},
                "line": {"type": "integer"},
                "args": {"type": "object", "description": "Legacy nested bag; prefer top-level fields"}
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let mut extra = merge_vibe_extra(&args);
        // Allow purpose string at top-level to mean purpose_decision.changed when mutating.
        if matches!(action, "insert" | "replace" | "drop") {
            if purpose_decision(&extra).is_none() {
                let purpose_owned = extra
                    .get("purpose")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty());
                if let Some(p) = purpose_owned {
                    if let Some(obj) = extra.as_object_mut() {
                        obj.insert(
                            "purpose_decision".to_string(),
                            json!({"changed": p}),
                        );
                    }
                }
            }
        }
        if !is_safe_project_path(path) {
            let mut output = "path must be a non-empty workspace-relative path without '..'".to_string();
            append_program_guide(&mut output, path, action, false, ctx);
            return ToolResult {
                output,
                exit_code: 126,
                duration_ms: 0,
                truncated: false,
            };
        }
        // ui.json / app.json are NOT block-managed — LLM emptied calculator UI this way.
        if is_vibe_forbidden_path(path) {
            let mut output = refuse_vibe_on_non_code(path);
            output.push_str(
                "\n计算器类故障常见原因: 对 ui.json 误建空区块后再 assemble，会把界面投影写成 0 字节。",
            );
            return ToolResult {
                output,
                exit_code: 126,
                duration_ms: 0,
                truncated: false,
            };
        }
        if action == "assemble" {
            if let Some(out) = extra.get("out").and_then(Value::as_str) {
                if !is_safe_project_path(out) {
                    let mut output = "unsafe assemble output path: use a workspace-relative path without '..'"
                        .to_string();
                    append_program_guide(&mut output, path, action, false, ctx);
                    return ToolResult {
                        output,
                        exit_code: 1,
                        duration_ms: 0,
                        truncated: false,
                    };
                }
            }
        }
        if matches!(action, "new" | "split")
            && extra
                .get("purpose")
                .and_then(Value::as_str)
                .map_or(true, |purpose| purpose.trim().is_empty())
        {
            let mut output = "new/split require purpose=\"why this file exists\" (tool field, not a shell flag)".to_string();
            append_program_guide(&mut output, path, action, false, ctx);
            return ToolResult {
                output,
                exit_code: 1,
                duration_ms: 0,
                truncated: false,
            };
        }
        if matches!(action, "insert" | "replace" | "drop") {
            prepare_mutation_extra(path, &mut extra, ctx);
        }
        if matches!(action, "read" | "peek" | "replace" | "drop")
            && extra.get("seq").and_then(|v| v.as_u64()).unwrap_or(0) == 0
        {
            let mut output =
                "missing seq: first call action=overview, then read/replace with seq from the list"
                    .to_string();
            append_program_guide(&mut output, path, action, false, ctx);
            return ToolResult {
                output,
                exit_code: 1,
                duration_ms: 0,
                truncated: false,
            };
        }

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
            _ => (
                "unknown action — use overview|read|replace|insert|drop|verify (tool fields only)"
                    .into(),
                1,
            ),
        };
        let mut result = result;
        // Program maps blocks → source file. Do not leave LLM to "write the file".
        if result.1 == 0 && matches!(action, "insert" | "replace" | "drop") {
            if let Some(meta) = find_blockset(&ctx.workspace, path) {
                let disk = projection_len(&ctx.workspace, path);
                if meta.code_bytes == 0 && disk > 0 {
                    result.0.push_str(
                        "\n[program] SKIP assemble: blockset is empty but disk file has content — \
                         refuse to wipe the projection. Fix blocks (insert/split) first.\n",
                    );
                } else {
                    let (asm_out, asm_code) = run_vibe(&["assemble", path], None, ctx).await;
                    if asm_code == 0 {
                        result.0.push_str(
                            "\n[program] projection assembled from blocks (source file updated by MoonCoding).\n",
                        );
                        result.0.push_str(&asm_out);
                    } else {
                        result.0.push_str(
                            "\n[program] WARN: auto-assemble failed — blocks remain truth; call action=assemble.\n",
                        );
                        result.0.push_str(&asm_out);
                    }
                }
            } else {
                let (asm_out, asm_code) = run_vibe(&["assemble", path], None, ctx).await;
                if asm_code == 0 {
                    result.0.push_str(
                        "\n[program] projection assembled from blocks (source file updated by MoonCoding).\n",
                    );
                    result.0.push_str(&asm_out);
                } else {
                    result.0.push_str(
                        "\n[program] WARN: auto-assemble failed — blocks remain truth; call action=assemble.\n",
                    );
                    result.0.push_str(&asm_out);
                }
            }
        }
        append_program_guide(&mut result.0, path, action, result.1 == 0, ctx);
        let duration_ms = start.elapsed().as_millis() as u64;
        if action == "verify" {
            if let Ok(mut log) = ctx.command_log.write() {
                log.push(super::CommandExecution {
                    command: format!("vibe verify {path}"),
                    exit_code: result.1,
                    tool: "vibe".to_string(),
                    verification_kind: "integrity".to_string(),
                    working_directory: ctx.workspace.clone(),
                    completed_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }
        ToolResult {
            output: result.0,
            exit_code: result.1,
            duration_ms,
            truncated: false,
        }
    }
}

fn is_safe_project_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    let has_windows_prefix =
        (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
            || path.starts_with("\\\\");
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && !has_windows_prefix
        && path
            .components()
            .any(|component| matches!(component, Component::Normal(_)))
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Accept mutation fields at top-level OR nested under `args` (LLM footgun).
fn merge_vibe_extra(top: &Value) -> Value {
    let mut extra = top.get("args").cloned().unwrap_or_else(|| json!({}));
    if !extra.is_object() {
        extra = json!({});
    }
    if let Some(obj) = extra.as_object_mut() {
        for key in [
            "rev",
            "seq",
            "after",
            "code",
            "tail",
            "purpose_decision",
            "purpose",
            "name",
            "lang",
            "line",
            "out",
        ] {
            if !obj.contains_key(key) {
                if let Some(v) = top.get(key) {
                    obj.insert(key.to_string(), v.clone());
                }
            }
        }
    }
    extra
}

fn prepare_mutation_extra(path: &str, extra: &mut Value, ctx: &ToolContext) {
    let rev = extra.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let need_pd = purpose_decision(extra).is_none();
    let Some(obj) = extra.as_object_mut() else {
        return;
    };
    if rev == 0 {
        if let Some(meta) = find_blockset(&ctx.workspace, path) {
            obj.insert("rev".to_string(), json!(meta.rev));
        }
    }
    if need_pd {
        obj.insert(
            "purpose_decision".to_string(),
            json!({"unchanged": true}),
        );
    }
}

/// Program-facing NEXT menu so the model fills tool fields instead of inventing CLI.
fn append_program_guide(
    out: &mut String,
    path: &str,
    last_action: &str,
    ok: bool,
    ctx: &ToolContext,
) {
    out.push_str("\n\n—— 程序引导（填 vibe 工具字段；禁止改源文件、禁止自拼命令行）——\n");
    match find_blockset(&ctx.workspace, path) {
        None => {
            out.push_str(&format!(
                "状态: 尚无区块集 path=`{path}`\n\
                 下一步只能选其一:\n\
                 • action=new   path={path}  purpose=\"...\"  name=<stem>  lang=python|rust|cpp\n\
                 • action=split path={path}  purpose=\"...\"   （磁盘投影已存在时）\n\
                 然后: action=overview → action=read seq=N → action=replace seq=N code=...\n"
            ));
        }
        Some(meta) => {
            out.push_str(&format!(
                "状态: path=`{}`  rev={}  blocks={}  purpose=\"{}\"\n",
                meta.path, meta.rev, meta.block_count, meta.purpose
            ));
            if !meta.summaries.is_empty() {
                out.push_str("区块标注:\n");
                for (seq, summary) in &meta.summaries {
                    out.push_str(&format!("  [{seq}] {summary}\n"));
                }
            }
            out.push_str("允许的下一步 (只调 vibe 工具):\n");
            out.push_str(&format!("• overview  path={path}\n"));
            if meta.block_count > 0 {
                out.push_str(&format!(
                    "• read      path={path}  seq=<1..{}>     ← 只读一块标注对应代码\n",
                    meta.block_count
                ));
                out.push_str(&format!(
                    "• replace   path={path}  seq=<n>  code=\"...\"  tail={{\"summary\":\"...\",\"purpose\":\"...\"}}  (rev自动={})\n",
                    meta.rev
                ));
                out.push_str(&format!(
                    "• insert    path={path}  after=<n|0>  code=\"...\"  tail={{...}}\n"
                ));
                out.push_str(&format!("• drop      path={path}  seq=<n>\n"));
            } else {
                out.push_str(&format!(
                    "• insert    path={path}  after=0  code=\"...\"  tail={{...}}   ← 先插入第一块\n"
                ));
            }
            out.push_str(&format!("• verify    path={path}\n"));
            if !ok {
                out.push_str(&format!(
                    "上次 action={last_action} 失败: 按上面字段重试；rev 以程序给出的 {} 为准。\n",
                    meta.rev
                ));
            } else if last_action == "overview" || last_action == "read" {
                out.push_str("提示: 已看到标注/块内容后，用 replace/insert 改块；程序会自动 assemble 投影。\n");
            } else if matches!(last_action, "replace" | "insert" | "drop") {
                out.push_str("提示: 块已由程序落盘投影；需要收尾时 action=verify。\n");
            }
        }
    }
}

async fn run_vibe(args: &[&str], stdin_text: Option<&str>, ctx: &ToolContext) -> (String, i32) {
    if ctx.vibe_exe.as_os_str().is_empty()
        || (ctx.vibe_exe.is_absolute() && !ctx.vibe_exe.is_file())
    {
        return (
            format!(
                "vibe executable not found at `{}`. Place vibe.exe beside mooncoding.exe \
                 or set VIBE_PATH.",
                ctx.vibe_exe.display()
            ),
            127,
        );
    }
    let mut cmd = Command::new(&ctx.vibe_exe);
    for a in args {
        cmd.arg(a);
    }
    cmd.current_dir(&ctx.workspace);
    // Force UTF-8 stdio so Chinese Windows does not emit GBK into the agent UI.
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    #[cfg(windows)]
    {
        // Rust 1.87+ respects this for console; for pipes we still decode carefully.
        cmd.env("RUST_LIB_BACKTRACE", "0");
    }
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    if let Some(vibe_dir) = ctx.vibe_exe.parent() {
        let mut paths = vec![vibe_dir.to_path_buf()];
        if let Some(current) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&current));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            cmd.env("PATH", joined);
        }
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return (
                format!(
                    "spawn vibe failed (`{}`): {e}. Check VIBE_PATH / vibe.exe beside the UI.",
                    ctx.vibe_exe.display()
                ),
                -1,
            );
        }
    };
    if let Some(txt) = stdin_text {
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(txt.as_bytes()).await {
                return (format!("write vibe stdin failed: {e}"), -1);
            }
        }
    }
    // drop stdin so child can finish reading
    drop(child.stdin.take());
    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(VIBE_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return (format!("wait err: {error}"), -1),
        Err(_) => return (format!("timeout after {VIBE_TIMEOUT_SECS}s"), 124),
    };
    let code = output.status.code().unwrap_or(-1);
    let stdout = crate::encoding_util::decode_console_bytes(&output.stdout);
    let stderr = crate::encoding_util::decode_console_bytes(&output.stderr);
    let mut text = String::new();
    if !stdout.is_empty() {
        text.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    if text.contains('\u{FFFD}') {
        text.push_str(
            "\n[mooncoding] warning: tool output contained invalid encoding bytes; \
             some characters were replaced. Prefer UTF-8 sources/tools.",
        );
    }
    (text, code)
}

// ── helpers ──

fn purpose_decision(extra: &Value) -> Option<String> {
    let pd = extra.get("purpose_decision")?;
    if let Some(c) = pd.get("changed").and_then(|v| v.as_str()) {
        Some(format!("{{\"changed\":\"{}\"}}", c))
    } else if let Some(u) = pd.get("unchanged").and_then(|v| v.as_bool()) {
        if u {
            Some("{\"unchanged\":true}".to_string())
        } else {
            None
        }
    } else {
        None
    }
}

async fn vibe_new(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let name = extra.get("name").and_then(|v| v.as_str()).unwrap_or(path);
    let lang = extra
        .get("lang")
        .and_then(|v| v.as_str())
        .unwrap_or("python");
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    run_vibe(
        &[
            "new",
            path,
            "--name",
            name,
            "--lang",
            lang,
            "--purpose",
            purpose,
        ],
        None,
        ctx,
    )
    .await
}

async fn vibe_split(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    let language = extra.get("lang").and_then(|v| v.as_str());
    let mut args = vec!["split", path, "--purpose", purpose];
    if let Some(language) = language {
        args.push("--lang");
        args.push(language);
    }
    run_vibe(&args, None, ctx).await
}

async fn vibe_overview(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["overview", path], None, ctx).await
}

async fn vibe_peek(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let seq = extra
        .get("seq")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .to_string();
    run_vibe(&["peek", path, &seq], None, ctx).await
}

async fn vibe_read(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let seq = extra
        .get("seq")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .to_string();
    run_vibe(&["read", path, &seq], None, ctx).await
}

async fn vibe_meta(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let purpose = extra.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    run_vibe(&["meta", path, "--purpose", purpose], None, ctx).await
}

async fn vibe_assemble(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let out = extra.get("out").and_then(|v| v.as_str());
    let mut args = vec!["assemble", path];
    if let Some(o) = out {
        args.push("-o");
        args.push(o);
    }
    run_vibe(&args, None, ctx).await
}

async fn vibe_verify(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["verify", path], None, ctx).await
}

async fn vibe_deps(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["deps", path], None, ctx).await
}

async fn vibe_lookup(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let line = extra
        .get("line")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .to_string();
    run_vibe(&["lookup", path, &line], None, ctx).await
}

async fn vibe_info(path: &str, ctx: &ToolContext) -> (String, i32) {
    run_vibe(&["info", path], None, ctx).await
}

async fn vibe_insert(path: &str, extra: &Value, ctx: &ToolContext) -> (String, i32) {
    let rev = extra.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let after = extra.get("after").and_then(|v| v.as_u64()).unwrap_or(0);
    let code = extra.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let tail = extra
        .get("tail")
        .cloned()
        .unwrap_or(json!({"summary": "", "purpose": ""}));
    let pd = purpose_decision(extra);
    if pd.is_none() {
        return ("purpose_decision required".into(), 1);
    }
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
    let tail = extra
        .get("tail")
        .cloned()
        .unwrap_or(json!({"summary": "", "purpose": ""}));
    let pd = purpose_decision(extra);
    if pd.is_none() {
        return ("purpose_decision required".into(), 1);
    }
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
    if pd.is_none() {
        return ("purpose_decision required".into(), 1);
    }
    let payload = json!({
        "rev": rev, "seq": seq, "purpose_decision": serde_json::from_str::<Value>(&pd.unwrap()).unwrap_or(json!({}))
    });
    let stdin_str = serde_json::to_string(&payload).unwrap_or_default();
    run_vibe(&["drop", path], Some(&stdin_str), ctx).await
}

#[cfg(test)]
mod tests {
    use super::is_safe_project_path;

    #[test]
    fn project_paths_cannot_escape_workspace() {
        assert!(is_safe_project_path("src/main.rs"));
        assert!(is_safe_project_path("src\\main.rs"));
        assert!(!is_safe_project_path("../outside.rs"));
        assert!(!is_safe_project_path(""));
        assert!(!is_safe_project_path("/tmp/outside.rs"));
        assert!(!is_safe_project_path("C:\\outside.rs"));
    }
}
