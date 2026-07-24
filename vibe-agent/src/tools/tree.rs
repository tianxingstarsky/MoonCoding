use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tree::{NewTreeNode, NodePatch, TreeActor, VerificationEvidence};

use super::{Tool, ToolContext, ToolResult};

pub struct TreeTool;

#[async_trait]
impl Tool for TreeTool {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "Create and maintain the persistent project tree. Empty trees: you MUST bootstrap with \
         create_nodes. Humans and AI may both update node fields including status; prefer \
         preserving recent human notes when still relevant. Prefer expected_version equal to \
         the current Tree version from the prompt (if omitted, the current version is used). \
         Example create: {\"action\":\"create_nodes\",\"expected_version\":0,\
\"nodes\":[{\"id\":\"root\",\"title\":\"App\",\"kind\":\"project\",\"status\":\"pending\"},\
{\"id\":\"ui\",\"parent_id\":\"root\",\"title\":\"index.html\",\"kind\":\"task\",\
\"status\":\"pending\",\"target_files\":[\"index.html\"]}]}. \
         Example update: {\"action\":\"update_node\",\"node_id\":\"ui\",\"expected_version\":1,\
\"status\":\"in_progress\",\"target_files\":[\"index.html\"]}. \
         status=completed requires real successful evidence from this run: copy \
         verify_command evidence_command or `vibe verify <path>` into evidence.command \
         (human form like `python apps/x/main.py` also matches)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "create_nodes", "update_node", "delete_node", "review_context"]
                },
                "expected_version": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Current tree version. Optional; defaults to the live tree version."
                },
                "node_id": {"type": "string"},
                "node": {
                    "type": "object",
                    "description": "Singular alias for create_nodes when creating one node."
                },
                "nodes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "description": "Stable ID chosen before referencing this node as a parent"},
                            "parent_id": {"type": ["string", "null"]},
                            "title": {"type": "string"},
                            "description": {"type": "string"},
                            "kind": {
                                "type": "string",
                                "enum": ["project", "feature", "module", "task", "test", "decision", "alternative", "research", "milestone"]
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "failed", "needs_review", "blocked", "rejected", "cancelled"]
                            },
                            "priority": {"type": "integer", "minimum": 0, "maximum": 100},
                            "delegate_status_to_ai": {"type": "boolean"},
                            "ai_note": {"type": ["string", "null"]},
                            "target_files": {
                                "type": "array",
                                "items": {"type": "string"}
                            },
                            "evidence": {
                                "type": "array",
                                "description": "Required with at least one successful record before AI marks a node completed",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "kind": {"type": "string", "description": "test, build, lint, or integrity; must match the executed command"},
                                        "summary": {"type": "string"},
                                        "command": {"type": ["string", "null"]},
                                        "success": {"type": "boolean"}
                                    },
                                    "required": ["kind", "summary", "success"]
                                }
                            }
                        },
                        "required": ["id", "title"]
                    }
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "failed", "needs_review", "blocked", "rejected", "cancelled"],
                    "description": "Convenience flat field for update_node (same as patch.status)"
                },
                "title": {"type": "string", "description": "Convenience flat field for update_node"},
                "description": {"type": "string", "description": "Convenience flat field for update_node"},
                "target_files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Convenience flat field for update_node"
                },
                "ai_note": {"type": ["string", "null"], "description": "Convenience flat field for update_node"},
                "patch": {
                    "type": "object",
                    "properties": {
                        "parent_id": {"type": ["string", "null"]},
                        "title": {"type": "string"},
                        "description": {"type": "string"},
                        "kind": {
                            "type": "string",
                            "enum": ["project", "feature", "module", "task", "test", "decision", "alternative", "research", "milestone"]
                        },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "in_progress", "completed", "failed", "needs_review", "blocked", "rejected", "cancelled"]
                        },
                        "priority": {"type": "integer", "minimum": 0, "maximum": 100},
                        "ai_note": {"type": ["string", "null"]},
                        "target_files": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "evidence": {
                            "type": "array",
                            "description": "Verification records. Supply successful evidence when setting status=completed.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "kind": {"type": "string", "description": "test, build, lint, or integrity; must match the executed command"},
                                    "summary": {"type": "string"},
                                    "command": {"type": ["string", "null"]},
                                    "success": {"type": "boolean"}
                                },
                                "required": ["kind", "summary", "success"]
                            }
                        }
                    }
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        match execute_tree_action(args, ctx) {
            Ok(output) => success(output),
            Err(error) => failure(error.to_string()),
        }
    }
}

fn execute_tree_action(args: Value, ctx: &ToolContext) -> anyhow::Result<String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("tree action is required"))?;
    let mut manager = ctx
        .project_tree
        .write()
        .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?;

    match action {
        "get" => manager.to_json(),
        "create_nodes" => {
            let expected_version = expected_version(&args, &manager)?;
            let values: Vec<Value> = if let Some(arr) = args.get("nodes").and_then(Value::as_array)
            {
                arr.clone()
            } else if let Some(single) = args.get("node") {
                vec![single.clone()]
            } else {
                anyhow::bail!(
                    "nodes array is required (or pass singular node={{...}}). \
                     Example: {{\"action\":\"create_nodes\",\"nodes\":[{{\"id\":\"root\",\
\"title\":\"App\",\"kind\":\"project\",\"status\":\"pending\"}}]}}"
                );
            };
            if values.is_empty() {
                anyhow::bail!("nodes array cannot be empty");
            }

            let mut candidate = manager.clone();
            let mut current_version = expected_version;
            let mut ids = Vec::with_capacity(values.len());
            for value in &values {
                let mut input: NewTreeNode = serde_json::from_value(value.clone())?;
                validate_evidence(&mut input.evidence, ctx)?;
                let id = candidate.add_node(input, TreeActor::Ai, current_version)?;
                current_version = candidate.version();
                ids.push(id);
            }
            *manager = candidate;
            Ok(json!({
                "created_ids": ids,
                "tree_version": manager.version(),
                "tree": manager.tree()
            })
            .to_string())
        }
        "update_node" => {
            let expected_version = expected_version(&args, &manager)?;
            let id = node_id(&args)?;
            let mut patch: NodePatch = parse_node_patch(&args)?;
            if let Some(evidence) = patch.evidence.as_deref_mut() {
                validate_evidence(evidence, ctx)?;
            }
            match manager.update_node(id, patch, TreeActor::Ai, expected_version) {
                Ok(()) => mutation_output(&manager),
                Err(error) => Err(enrich_tree_error(error, &manager)),
            }
        }
        "delete_node" => {
            let expected_version = expected_version(&args, &manager)?;
            let id = node_id(&args)?;
            match manager.delete_node(id, TreeActor::Ai, expected_version) {
                Ok(deleted_ids) => Ok(json!({
                    "deleted_ids": deleted_ids,
                    "tree_version": manager.version(),
                    "tree": manager.tree()
                })
                .to_string()),
                Err(error) => Err(enrich_tree_error(error, &manager)),
            }
        }
        "review_context" => {
            if let Ok(id) = node_id(&args) {
                manager.review_context(id)
            } else {
                Ok(manager.full_review_context())
            }
        }
        _ => anyhow::bail!("unknown tree action: {action}"),
    }
}

fn expected_version(args: &Value, manager: &crate::tree::TreeManager) -> anyhow::Result<u64> {
    // Kill the footgun: if the model forgets expected_version, use the live tree
    // version instead of failing the whole mutation.
    if let Some(v) = args.get("expected_version").and_then(Value::as_u64) {
        return Ok(v);
    }
    Ok(manager.version())
}

fn node_id(args: &Value) -> anyhow::Result<&str> {
    args.get("node_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow::anyhow!("node_id is required"))
}

/// Accept either `patch: {...}` or flat fields (`status`, `title`, ...) for LLM convenience.
fn parse_node_patch(args: &Value) -> anyhow::Result<NodePatch> {
    if let Some(patch_value) = args.get("patch").cloned() {
        return Ok(serde_json::from_value(patch_value)?);
    }

    let mut flat = serde_json::Map::new();
    for key in [
        "parent_id",
        "title",
        "description",
        "kind",
        "status",
        "priority",
        "ai_note",
        "target_files",
        "evidence",
    ] {
        if let Some(value) = args.get(key) {
            flat.insert(key.to_string(), value.clone());
        }
    }
    if flat.is_empty() {
        anyhow::bail!(
            "patch is required (or pass flat fields like status/title/target_files on the tool call)"
        );
    }
    Ok(serde_json::from_value(Value::Object(flat))?)
}

fn enrich_tree_error(
    error: anyhow::Error,
    manager: &crate::tree::TreeManager,
) -> anyhow::Error {
    let message = error.to_string();
    if message.contains("tree version stale") {
        return anyhow::anyhow!(
            "{message}. Fix: call tree action=get, then retry with expected_version={}.",
            manager.version()
        );
    }
    if message.contains("verification evidence") || message.contains("completed") {
        return anyhow::anyhow!(
            "{message}. Fix: only set status=completed after verify_command or vibe verify \
             exited 0 in THIS run. Set evidence.command from the tool's evidence_command line \
             (or human form like `python apps/x/main.py` / `vibe verify apps/x/main.py`). \
             Kind is auto-filled. For in_progress/failed/needs_review you do not need evidence."
        );
    }
    error
}

fn mutation_output(manager: &crate::tree::TreeManager) -> anyhow::Result<String> {
    Ok(json!({
        "tree_version": manager.version(),
        "tree": manager.tree()
    })
    .to_string())
}

fn validate_evidence(
    evidence: &mut [VerificationEvidence],
    ctx: &ToolContext,
) -> anyhow::Result<()> {
    if !evidence.iter().any(|item| item.success) {
        return Ok(());
    }
    let log = ctx
        .command_log
        .read()
        .map_err(|_| anyhow::anyhow!("command evidence lock poisoned"))?;
    for item in evidence.iter_mut().filter(|item| item.success) {
        let command = item
            .command
            .as_deref()
            .filter(|command| !command.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "successful AI evidence requires command copied from verify_command \
                     `evidence_command:` line or `vibe verify <path>`"
                )
            })?
            .to_string();
        let execution = log.iter().rev().find(|execution| {
            execution.exit_code == 0 && evidence_command_matches(&execution.command, &command)
        });
        let Some(execution) = execution else {
            let available: Vec<String> = log
                .iter()
                .filter(|e| e.exit_code == 0)
                .map(|e| format!("{} ({})", e.command, e.verification_kind))
                .collect();
            let hint = if available.is_empty() {
                "No successful verify_command/vibe verify in this run yet. \
                 Run one (exit 0), then copy its evidence_command / `vibe verify <path>`."
                    .to_string()
            } else {
                format!(
                    "Successful commands available to cite: {}",
                    available.join(" | ")
                )
            };
            anyhow::bail!(
                "verification evidence rejected: command did not succeed in this run: {command}. {hint}"
            );
        };
        // Trust the execution log for kind/tool/cwd so models cannot fail on
        // "test" vs "run" wording while citing a real successful command.
        item.tool = Some(execution.tool.clone());
        item.kind = execution.verification_kind.clone();
        item.command = Some(execution.command.clone());
        item.working_directory = Some(execution.working_directory.to_string_lossy().to_string());
        item.recorded_at = execution.completed_at.clone();
    }
    Ok(())
}

/// Match evidence.command against a logged identity.
/// Accepts JSON `{"program","args"}`, human `program arg…`, or `vibe verify path`.
fn evidence_command_matches(logged: &str, claimed: &str) -> bool {
    let logged = logged.trim();
    let claimed = claimed.trim();
    if logged.is_empty() || claimed.is_empty() {
        return false;
    }
    if logged == claimed {
        return true;
    }
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    if norm(logged) == norm(claimed) {
        return true;
    }
    if let Ok(v) = serde_json::from_str::<Value>(logged) {
        if let (Some(prog), Some(args)) = (
            v.get("program").and_then(|p| p.as_str()),
            v.get("args").and_then(|a| a.as_array()),
        ) {
            let joined = args
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let human = if joined.is_empty() {
                prog.to_string()
            } else {
                format!("{prog} {joined}")
            };
            if norm(&human) == norm(claimed) {
                return true;
            }
            if let Ok(c) = serde_json::from_str::<Value>(claimed) {
                return c.get("program") == v.get("program") && c.get("args") == v.get("args");
            }
        }
    }
    false
}

fn success(output: String) -> ToolResult {
    ToolResult {
        output,
        exit_code: 0,
        duration_ms: 0,
        truncated: false,
    }
}

fn failure(output: String) -> ToolResult {
    ToolResult {
        output,
        exit_code: 1,
        duration_ms: 0,
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::CommandExecution;
    use crate::tree::{TreeActor, TreeManager};
    use crate::vector::KnowledgeBase;
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};

    fn context(log: Vec<CommandExecution>) -> ToolContext {
        ToolContext {
            workspace: PathBuf::from("."),
            vibe_exe: PathBuf::from("vibe"),
            session_id: "test".to_string(),
            project_tree: Arc::new(RwLock::new(TreeManager::empty())),
            command_log: Arc::new(RwLock::new(log)),
            knowledge: Arc::new(RwLock::new(KnowledgeBase::empty(PathBuf::from(".")))),
            app_runtime: None,
        }
    }

    fn evidence(command: &str) -> VerificationEvidence {
        VerificationEvidence {
            kind: "test".to_string(),
            summary: "tests passed".to_string(),
            command: Some(command.to_string()),
            tool: None,
            working_directory: None,
            success: true,
            recorded_by: TreeActor::Ai,
            recorded_at: String::new(),
        }
    }

    #[test]
    fn accepts_only_commands_that_succeeded_in_current_run() {
        let ctx = context(vec![CommandExecution {
            command: "cargo test".to_string(),
            exit_code: 0,
            tool: "verify_command".to_string(),
            verification_kind: "test".to_string(),
            working_directory: PathBuf::from("."),
            completed_at: "2026-01-01T00:00:00Z".to_string(),
        }]);
        let mut accepted = [evidence("cargo test")];
        assert!(validate_evidence(&mut accepted, &ctx).is_ok());
        assert_eq!(accepted[0].tool.as_deref(), Some("verify_command"));
        let mut rejected = [evidence("cargo test --all")];
        assert!(validate_evidence(&mut rejected, &ctx).is_err());
    }

    #[test]
    fn accepts_json_identity_or_human_form_and_autocorrects_kind() {
        let identity = r#"{"program":"python","args":["apps/demo/main.py"]}"#;
        let ctx = context(vec![CommandExecution {
            command: identity.to_string(),
            exit_code: 0,
            tool: "verify_command".to_string(),
            verification_kind: "run".to_string(),
            working_directory: PathBuf::from("/ws"),
            completed_at: "2026-01-01T00:00:00Z".to_string(),
        }]);
        let mut by_human = [VerificationEvidence {
            kind: "test".to_string(),
            summary: "script ok".to_string(),
            command: Some("python apps/demo/main.py".to_string()),
            tool: None,
            working_directory: None,
            success: true,
            recorded_by: TreeActor::Ai,
            recorded_at: String::new(),
        }];
        assert!(validate_evidence(&mut by_human, &ctx).is_ok());
        assert_eq!(by_human[0].kind, "run");
        assert_eq!(by_human[0].command.as_deref(), Some(identity));
        assert_eq!(by_human[0].tool.as_deref(), Some("verify_command"));
    }

    #[test]
    fn accepts_vibe_verify_integrity_evidence() {
        let ctx = context(vec![CommandExecution {
            command: "vibe verify apps/demo/main.py".to_string(),
            exit_code: 0,
            tool: "vibe".to_string(),
            verification_kind: "integrity".to_string(),
            working_directory: PathBuf::from("."),
            completed_at: "2026-01-01T00:00:00Z".to_string(),
        }]);
        let mut item = [VerificationEvidence {
            kind: "integrity".to_string(),
            summary: "blocks ok".to_string(),
            command: Some("vibe verify apps/demo/main.py".to_string()),
            tool: None,
            working_directory: None,
            success: true,
            recorded_by: TreeActor::Ai,
            recorded_at: String::new(),
        }];
        assert!(validate_evidence(&mut item, &ctx).is_ok());
        assert_eq!(item[0].tool.as_deref(), Some("vibe"));
    }

    #[tokio::test]
    async fn create_nodes_bootstraps_empty_tree_without_expected_version() {
        let ctx = context(Vec::new());
        let tool = TreeTool;
        let out = tool
            .execute(
                json!({
                    "action": "create_nodes",
                    "nodes": [
                        {
                            "id": "root",
                            "title": "App",
                            "kind": "project",
                            "status": "pending"
                        },
                        {
                            "id": "ui",
                            "parent_id": "root",
                            "title": "index.html",
                            "kind": "task",
                            "status": "pending",
                            "target_files": ["index.html"]
                        }
                    ]
                }),
                &ctx,
            )
            .await;
        assert_eq!(out.exit_code, 0, "{}", out.output);
        let value: Value = serde_json::from_str(&out.output).expect("json");
        assert_eq!(value["created_ids"].as_array().unwrap().len(), 2);
        assert_eq!(value["tree_version"], 2);
        assert_eq!(ctx.project_tree.read().unwrap().tree().nodes.len(), 2);
    }

    #[tokio::test]
    async fn create_nodes_accepts_singular_node_alias() {
        let ctx = context(Vec::new());
        let tool = TreeTool;
        let out = tool
            .execute(
                json!({
                    "action": "create_nodes",
                    "node": {
                        "id": "root",
                        "title": "Solo",
                        "kind": "project",
                        "status": "pending"
                    }
                }),
                &ctx,
            )
            .await;
        assert_eq!(out.exit_code, 0, "{}", out.output);
        let value: Value = serde_json::from_str(&out.output).expect("json");
        assert_eq!(value["created_ids"][0], "root");
    }
}
