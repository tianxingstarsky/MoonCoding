use async_trait::async_trait;
use serde_json::{json, Value};

use super::blockgate::{
    find_blockset, format_blockset_skeleton, is_code_surface, refuse_code_read, refuse_code_write,
    to_posix_rel,
};
use super::paths::{confine_app_dir, confine_app_entry, is_safe_app_name, is_safe_relative_file};
use super::{Tool, ToolContext, ToolResult};
use crate::app_runtime::StartOptions;
use crate::apps;

/// Manage apps strictly under `workspace/apps/<name>/` for the active project only.
pub struct AppsTool;

#[async_trait]
impl Tool for AppsTool {
    fn name(&self) -> &str {
        "apps"
    }

    fn description(&self) -> &str {
        "Manage apps ONLY inside the current workspace (`apps/<name>/`). \
         Never touch other projects or sibling workspaces. \
         Actions: list, get, create, update, delete, read_entry, \
         runtime_status, runtime_start, runtime_stop, runtime_force_stop, runtime_restart. \
         Runtime actions inspect/control the native Python sandbox (lease + pid) safely: \
         use runtime_status when the UI says an app is running but you cannot find it; \
         use runtime_force_stop to clear ghost supervisors/orphan processes; \
         runtime_start replaces by default. \
         Delete requires confirm=true and only removes apps/<name> in THIS workspace. \
         Prefer update over delete. Do not delete an app unless the user explicitly asks. \
         Native Python apps use the stdlib-only mooncoding_app SDK. \
         Host: desktop = Qt6 landscape; board linuxfb = portrait 720x1280. \
         Prefer layouts usable on both; touch targets ≥44px. Same ui.json schema. \
         Never put x/y/width/height in ui.json; never vibe-manage ui.json/app.json. \
         Interactive apps must include apps/<name>/test_cli.py (HeadlessApp + click events). \
         Python/entry SOURCE is vibe-managed: do not use content= to rewrite .py after create — \
         use vibe replace/insert/rewrite. create may seed initial content then program runs vibe split. \
         Note: ~16k is tool-output truncate to the model, not a create/content input limit."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "list",
                        "get",
                        "create",
                        "update",
                        "delete",
                        "read_entry",
                        "runtime_status",
                        "runtime_start",
                        "runtime_stop",
                        "runtime_force_stop",
                        "runtime_restart"
                    ]
                },
                "name": {
                    "type": "string",
                    "description": "App directory name. Only a-z, 0-9, hyphens."
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Required true for delete."
                },
                "replace": {
                    "type": "boolean",
                    "description": "For runtime_start: replace any live/stuck instance (default true)."
                },
                "title": { "type": "string" },
                "description": { "type": "string" },
                "app_type": {
                    "type": "string",
                    "enum": ["web", "cli", "python"]
                },
                "entry": { "type": "string" },
                "command": { "type": "string" },
                "content": { "type": "string" },
                "ui_schema": { "type": "object" },
                "capabilities": { "type": "object" },
                "limits": { "type": "object" },
                "tree_node_id": { "type": "string" },
                "safe_shutdown": { "type": "boolean" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let start = std::time::Instant::now();
        let action = args["action"].as_str().unwrap_or("list");
        let result = match action {
            "list" => do_list(ctx),
            "get" => do_get(ctx, &args),
            "create" => do_create(ctx, &args),
            "update" => do_update(ctx, &args),
            "delete" => do_delete(ctx, &args),
            "read_entry" => do_read_entry(ctx, &args),
            "runtime_status" => do_runtime_status(ctx),
            "runtime_start" => do_runtime_start(ctx, &args).await,
            "runtime_stop" => do_runtime_stop(ctx).await,
            "runtime_force_stop" => do_runtime_force_stop(ctx).await,
            "runtime_restart" => do_runtime_restart(ctx, &args).await,
            _ => Err(format!("unknown apps action: {action}")),
        };
        let duration_ms = start.elapsed().as_millis() as u64;
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

fn require_runtime(ctx: &ToolContext) -> Result<&std::sync::Arc<crate::app_runtime::AppRuntimeManager>, String> {
    ctx.app_runtime
        .as_ref()
        .ok_or_else(|| {
            "app runtime sandbox unavailable in this tool context (desktop runtime not attached)"
                .to_string()
        })
}

fn do_runtime_status(ctx: &ToolContext) -> Result<String, String> {
    let runtime = require_runtime(ctx)?;
    let inspect = runtime.inspect();
    serde_json::to_string_pretty(&inspect).map_err(|e| e.to_string())
}

async fn do_runtime_start(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let runtime = require_runtime(ctx)?.clone();
    let name = required_app_name(args)?;
    let replace = args["replace"].as_bool().unwrap_or(true);
    let app = apps::get_app(&ctx.workspace, &name)
        .ok_or_else(|| format!("app '{name}' not found in this workspace"))?;
    let instance_id = runtime
        .start_with_options(&app, StartOptions { replace })
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "started app '{name}' instance_id={instance_id} replace={replace}\n{}",
        serde_json::to_string_pretty(&runtime.inspect()).unwrap_or_default()
    ))
}

async fn do_runtime_stop(ctx: &ToolContext) -> Result<String, String> {
    let runtime = require_runtime(ctx)?.clone();
    runtime
        .stop("llm requested stop")
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "stopped app runtime\n{}",
        serde_json::to_string_pretty(&runtime.inspect()).unwrap_or_default()
    ))
}

async fn do_runtime_force_stop(ctx: &ToolContext) -> Result<String, String> {
    let runtime = require_runtime(ctx)?.clone();
    runtime
        .force_stop("llm requested force_stop")
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "force-stopped app runtime (cleared supervisor + sandbox lease)\n{}",
        serde_json::to_string_pretty(&runtime.inspect()).unwrap_or_default()
    ))
}

async fn do_runtime_restart(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let runtime = require_runtime(ctx)?.clone();
    let name = required_app_name(args)?;
    let app = apps::get_app(&ctx.workspace, &name)
        .ok_or_else(|| format!("app '{name}' not found in this workspace"))?;
    let _ = runtime.force_stop("llm requested restart").await;
    let instance_id = runtime
        .start_with_options(&app, StartOptions { replace: true })
        .await
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "restarted app '{name}' instance_id={instance_id}\n{}",
        serde_json::to_string_pretty(&runtime.inspect()).unwrap_or_default()
    ))
}

fn do_list(ctx: &ToolContext) -> Result<String, String> {
    let apps = apps::list_apps(&ctx.workspace);
    if apps.is_empty() {
        return Ok(
            "No apps found in the current workspace. Use `apps create` to build one.".to_string(),
        );
    }
    let lines: Vec<String> = apps
        .iter()
        .map(|a| {
            format!(
                "- **{}** (`{}`, {}) — {} [entry: {}]",
                a.manifest.title,
                a.manifest.name,
                a.manifest.app_type,
                a.manifest.description,
                if a.entry_exists { "exists" } else { "missing" }
            )
        })
        .collect();
    Ok(format!(
        "Apps in THIS workspace only ({}/apps):\n{}",
        ctx.workspace.display(),
        lines.join("\n")
    ))
}

fn do_get(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let name = required_app_name(args)?;
    let app =
        apps::get_app(&ctx.workspace, &name).ok_or_else(|| format!("app '{name}' not found"))?;
    Ok(serde_json::to_string_pretty(&app).unwrap_or_else(|e| e.to_string()))
}

fn do_create(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let name = required_app_name(args)?;
    let app_type = required_string(args, "app_type")?;
    let title = required_string(args, "title")?;
    let description = args["description"].as_str().unwrap_or("").to_string();
    let entry = args["entry"].as_str().map(|s| s.to_string());
    let command = args["command"].as_str().map(|s| s.to_string());
    let content = args["content"].as_str().unwrap_or("");
    let ui_schema = args.get("ui_schema").filter(|value| !value.is_null());
    let capabilities = args
        .get("capabilities")
        .cloned()
        .unwrap_or_else(|| json!({"gpio": []}));
    let limits = args.get("limits").cloned().unwrap_or_else(|| json!({}));
    let tree_node_id = args.get("tree_node_id").cloned().unwrap_or(Value::Null);
    let safe_shutdown = args["safe_shutdown"].as_bool().unwrap_or(true);
    let tags: Vec<String> = args["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if app_type != "web" && app_type != "cli" && app_type != "python" {
        return Err("app_type must be 'web', 'cli', or 'python'".to_string());
    }

    let app_dir = confine_app_dir(&ctx.workspace, &name)?;
    if app_dir.exists() {
        return Err(format!(
            "app '{name}' already exists in this workspace. Use update to modify it."
        ));
    }
    std::fs::create_dir_all(&app_dir).map_err(|e| format!("cannot create app directory: {e}"))?;

    let entry_name = if app_type == "web" {
        entry.clone().unwrap_or_else(|| "index.html".to_string())
    } else if app_type == "python" {
        entry.clone().unwrap_or_else(|| "main.py".to_string())
    } else {
        command.clone().unwrap_or_else(|| format!("{name}.sh"))
    };
    let entry_path = confine_app_entry(&app_dir, &entry_name)?;

    let manifest = if app_type == "python" {
        json!({
            "schema_version": 2,
            "type": app_type,
            "title": title,
            "description": description,
            "version": "0.1.0",
            "entry": entry_name,
            "runtime": {
                "kind": "python",
                "entry": entry_name,
                "interpreter": "auto"
            },
            "ui": {"kind": "native_json", "entry": "ui.json"},
            "capabilities": capabilities,
            "limits": limits,
            "tree_node_id": tree_node_id,
            "safe_shutdown": safe_shutdown,
            "author": "AI",
            "tags": tags,
        })
    } else {
        json!({
            "type": app_type,
            "title": title,
            "description": description,
            "version": "0.1.0",
            "entry": if app_type == "web" { entry } else { None::<String> },
            "command": if app_type == "cli" { Some(entry_name.clone()) } else { None::<String> },
            "author": "AI",
            "tags": tags,
        })
    };

    let manifest_json =
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| manifest.to_string());
    std::fs::write(app_dir.join("app.json"), &manifest_json)
        .map_err(|e| format!("cannot write app.json: {e}"))?;
    if !content.is_empty() {
        std::fs::write(&entry_path, content)
            .map_err(|e| format!("cannot write entry file: {e}"))?;
    }
    if app_type == "python" {
        let ui = ui_schema.cloned().unwrap_or_else(|| {
            json!({"version": 1, "type": "screen", "id": "root", "children": []})
        });
        let encoded = serde_json::to_string_pretty(&ui)
            .map_err(|e| format!("cannot encode ui_schema: {e}"))?;
        let ui_path = confine_app_entry(&app_dir, "ui.json")?;
        std::fs::write(ui_path, encoded).map_err(|e| format!("cannot write ui.json: {e}"))?;
    }

    let entry_rel = to_posix_rel(&format!("apps/{name}/{entry_name}"));
    let mut note = String::new();
    if is_code_surface(&entry_rel) {
        note = ensure_code_blockset(ctx, &entry_rel, &format!("{title} app entry"))?;
    }

    Ok(format!(
        "Created {app_type} app '{title}' at apps/{name}/ in THIS workspace only.\n\
         Entry file: {entry_name} (projection). Edit code ONLY via vibe blocks.\n{note}"
    ))
}

fn do_update(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let name = required_app_name(args)?;
    let app_dir = confine_app_dir(&ctx.workspace, &name)?;
    if !app_dir.is_dir() {
        return Err(format!("app '{name}' not found in this workspace"));
    }

    let manifest_path = confine_app_entry(&app_dir, "app.json")?;
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("cannot read app.json: {e}"))?;
    let mut manifest: Value =
        serde_json::from_str(&text).map_err(|e| format!("invalid app.json: {e}"))?;

    if let Some(val) = args.get("title") {
        manifest["title"] = val.clone();
    }
    if let Some(val) = args.get("description") {
        manifest["description"] = val.clone();
    }
    if let Some(val) = args.get("command") {
        let command = val
            .as_str()
            .ok_or_else(|| "command must be a string".to_string())?;
        if !is_safe_relative_file(command) {
            return Err("command must be a relative file inside the app directory".to_string());
        }
        manifest["command"] = val.clone();
    }
    if let Some(val) = args.get("entry") {
        let entry = val
            .as_str()
            .ok_or_else(|| "entry must be a string".to_string())?;
        if !is_safe_relative_file(entry) {
            return Err("entry must be a relative file inside the app directory".to_string());
        }
        manifest["entry"] = val.clone();
        if manifest.get("runtime").is_some() {
            manifest["runtime"]["entry"] = val.clone();
        }
    }
    if let Some(val) = args.get("tags") {
        manifest["tags"] = val.clone();
    }
    if let Some(val) = args.get("capabilities") {
        manifest["capabilities"] = val.clone();
    }
    if let Some(val) = args.get("limits") {
        manifest["limits"] = val.clone();
    }
    if let Some(val) = args.get("tree_node_id") {
        manifest["tree_node_id"] = val.clone();
    }
    if let Some(val) = args.get("safe_shutdown") {
        manifest["safe_shutdown"] = val.clone();
    }

    let updated = serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| manifest.to_string());
    std::fs::write(&manifest_path, updated).map_err(|e| format!("cannot write app.json: {e}"))?;

    if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
        let entry = manifest["entry"]
            .as_str()
            .or_else(|| manifest["runtime"]["entry"].as_str())
            .or(manifest["command"].as_str())
            .unwrap_or("index.html");
        let entry_rel = to_posix_rel(&format!("apps/{name}/{entry}"));
        if is_code_surface(&entry_rel) {
            return Err(refuse_code_write(&entry_rel));
        }
        let entry_path = confine_app_entry(&app_dir, entry)?;
        std::fs::write(entry_path, content).map_err(|e| format!("cannot write entry file: {e}"))?;
    }
    if let Some(ui) = args.get("ui_schema") {
        let encoded = serde_json::to_string_pretty(ui)
            .map_err(|e| format!("cannot encode ui_schema: {e}"))?;
        let ui_path = confine_app_entry(&app_dir, "ui.json")?;
        std::fs::write(ui_path, encoded).map_err(|e| format!("cannot write ui.json: {e}"))?;
    }

    Ok(format!("Updated app '{name}' in THIS workspace only"))
}

fn do_delete(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let name = required_app_name(args)?;
    if args["confirm"].as_bool() != Some(true) {
        return Err(
            "refused: apps delete requires confirm=true, and only deletes apps/<name> \
             inside the current workspace. Ask the user before deleting."
                .to_string(),
        );
    }
    let app_dir = confine_app_dir(&ctx.workspace, &name)?;
    if !app_dir.is_dir() {
        return Err(format!("app '{name}' not found in this workspace"));
    }
    std::fs::remove_dir_all(&app_dir).map_err(|e| format!("cannot delete app: {e}"))?;
    Ok(format!(
        "Deleted app '{name}' from THIS workspace only ({}/apps/{name})",
        ctx.workspace.display()
    ))
}

fn do_read_entry(ctx: &ToolContext, args: &Value) -> Result<String, String> {
    let name = required_app_name(args)?;
    let app_dir = confine_app_dir(&ctx.workspace, &name)?;
    let listing = apps::list_apps(&ctx.workspace)
        .into_iter()
        .find(|a| a.manifest.name == name || a.dir == app_dir)
        .ok_or_else(|| format!("app '{name}' not found"))?;
    let entry_rel = listing
        .entry_path
        .as_ref()
        .and_then(|p| {
            p.strip_prefix(&ctx.workspace)
                .ok()
                .map(|r| to_posix_rel(&r.to_string_lossy()))
        })
        .unwrap_or_else(|| to_posix_rel(&format!("apps/{name}/main.py")));
    if is_code_surface(&entry_rel) {
        let meta = find_blockset(&ctx.workspace, &entry_rel);
        return Err(refuse_code_read(&entry_rel, meta.as_ref()));
    }
    let content = apps::read_app_entry(&ctx.workspace, &name).map_err(|e| format!("{e}"))?;
    const MAX_CHARS: usize = 16000;
    if content.chars().count() > MAX_CHARS {
        let truncated: String = content.chars().take(MAX_CHARS).collect();
        Ok(format!(
            "{truncated}\n\n[Truncated at {MAX_CHARS} characters]"
        ))
    } else {
        Ok(content)
    }
}

/// After seeding a code entry, make blocks the truth (split or new).
fn ensure_code_blockset(ctx: &ToolContext, entry_rel: &str, purpose: &str) -> Result<String, String> {
    if let Some(meta) = find_blockset(&ctx.workspace, entry_rel) {
        return Ok(format_blockset_skeleton(&meta));
    }
    let vibe = &ctx.vibe_exe;
    if vibe.as_os_str().is_empty() || !vibe.is_file() {
        return Err(format!(
            "created entry but vibe binary missing at {}; cannot establish blockset",
            vibe.display()
        ));
    }
    let abs = ctx.workspace.join(entry_rel);
    let purpose_arg = if purpose.trim().is_empty() {
        "app entry"
    } else {
        purpose
    };
    let output = if abs.is_file()
        && std::fs::metadata(&abs)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        std::process::Command::new(vibe)
            .args(["split", entry_rel, "--purpose", purpose_arg])
            .current_dir(&ctx.workspace)
            .output()
    } else {
        let name = std::path::Path::new(entry_rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main");
        std::process::Command::new(vibe)
            .args([
                "new",
                entry_rel,
                "--name",
                name,
                "--lang",
                "python",
                "--purpose",
                purpose_arg,
            ])
            .current_dir(&ctx.workspace)
            .output()
    }
    .map_err(|e| format!("vibe blockset setup failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "vibe blockset setup failed:\n{stdout}\n{stderr}"
        ));
    }
    let meta = find_blockset(&ctx.workspace, entry_rel);
    Ok(format!(
        "Blockset established for `{entry_rel}`.\n{}\n{stdout}{stderr}",
        meta.as_ref()
            .map(format_blockset_skeleton)
            .unwrap_or_default()
    ))
}

fn required_app_name(args: &Value) -> Result<String, String> {
    let name = required_string(args, "name")?;
    if !is_safe_app_name(&name) {
        return Err(
            "invalid app name: use a-z, 0-9, hyphens only (max 64 chars)".to_string(),
        );
    }
    Ok(name)
}

fn required_string(args: &Value, field: &str) -> Result<String, String> {
    args[field]
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("'{field}' is required and must be a non-empty string"))
}
