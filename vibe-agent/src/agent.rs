use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// 全局中断标记——TUI 按 Ctrl+C 时置 true, agent 循环顶部检查
pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);

use crate::config::Config;
use crate::provider::{Message, OpenAiCompatible, ParsedToolCall, StreamEvent, ToolCall};
use crate::session::{Session, SessionStore};
use crate::stream::AgentEvent;
use crate::tools::{CommandExecution, ToolContext, ToolRegistry};
use crate::tree::TreeManager;
use crate::vector::KnowledgeBase;

const TOOL_OUTPUT_MAX_CHARS: usize = 4000;
const PRUNE_START_STEP: usize = 12;
const PRUNE_KEEP_ASSISTANT: usize = 6;
/// Max times we auto-nudge the model to keep going in one user turn.
const MAX_AUTO_CONTINUES: u32 = 4;

/// 核心 agent 循环 —— 单次用户输入 → 流式 LLM → 工具执行 → 循环到 done
pub async fn run_agent(
    cfg: &Config,
    tools: &ToolRegistry,
    session_store: &dyn SessionStore,
    user_input: &str,
    session_id: &str,
    on_event: &mut dyn FnMut(AgentEvent),
) -> Result<()> {
    INTERRUPTED.store(false, Ordering::SeqCst);
    run_agent_with_interrupt(
        cfg,
        tools,
        session_store,
        user_input,
        session_id,
        &INTERRUPTED,
        None,
        on_event,
    )
    .await
}

/// Core loop with a caller-owned cancellation flag, used by the desktop backend.
pub async fn run_agent_with_interrupt(
    cfg: &Config,
    tools: &ToolRegistry,
    session_store: &dyn SessionStore,
    user_input: &str,
    session_id: &str,
    interrupted: &AtomicBool,
    app_runtime: Option<Arc<crate::app_runtime::AppRuntimeManager>>,
    on_event: &mut dyn FnMut(AgentEvent),
) -> Result<()> {
    on_event(AgentEvent::Thinking);

    let provider = OpenAiCompatible::new(
        cfg.provider.base_url.clone(),
        cfg.provider.model.clone(),
        cfg.provider.api_key.clone(),
        cfg.provider.max_tokens,
        cfg.provider.temperature,
    )?;

    let mut session = load_or_create_session(
        session_store,
        session_id,
        &cfg.provider.model,
        &cfg.provider.base_url,
    )
    .await?;
    session.model = cfg.provider.model.clone();
    session.provider = cfg.provider.base_url.clone();
    let project_tree = Arc::new(RwLock::new(TreeManager::new(
        session.project_tree.clone().unwrap_or_default(),
    )?));
    let knowledge = Arc::new(RwLock::new(KnowledgeBase::load(&cfg.workspace)?));
    session.messages.push(Message {
        role: "user".to_string(),
        content: Some(user_input.to_string()),
        tool_calls: None,
        tool_call_id: None,
    });

    let tool_defs = tools.definitions();
    let app_runtime = match app_runtime {
        Some(runtime) => Some(runtime),
        None => crate::app_runtime::AppRuntimeManager::for_workspace(&cfg.workspace)
            .ok()
            .map(Arc::new),
    };
    let tool_ctx = ToolContext {
        workspace: cfg.workspace.clone(),
        vibe_exe: cfg.vibe_exe.clone(),
        session_id: session.id.clone(),
        project_tree: project_tree.clone(),
        command_log: Arc::new(RwLock::new(Vec::<CommandExecution>::new())),
        knowledge: knowledge.clone(),
        app_runtime,
    };
    let project_instructions = crate::prompt::load_project_instructions(&cfg.workspace);
    {
        let manager = project_tree
            .read()
            .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?;
        on_event(AgentEvent::TreeUpdated {
            json: manager.to_json()?,
        });
    }

    let max_steps = cfg.agent.max_steps.unwrap_or(200);
    let prune_after = cfg.agent.prune_after.unwrap_or(PRUNE_START_STEP);
    let prune_keep = cfg.agent.prune_keep.unwrap_or(PRUNE_KEEP_ASSISTANT);
    let mut total_tokens_in = session.tokens_in;
    let mut total_tokens_out = session.tokens_out;
    let mut step = session.step;
    let mut run_steps = 0u64;
    let mut auto_continues = 0u32;
    let runtime_env = crate::prompt::build_runtime_env(cfg);

    loop {
        if interrupted.load(Ordering::SeqCst) {
            on_event(AgentEvent::Interrupted("user interrupted".into()));
            break;
        }
        if run_steps >= max_steps {
            on_event(AgentEvent::Interrupted(format!("max_steps={}", max_steps)));
            break;
        }

        // ── 构建 system prompt ──
        let tree_summary = project_tree
            .read()
            .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?
            .prompt_summary();
        let vector_guidance = knowledge
            .read()
            .map_err(|_| anyhow::anyhow!("knowledge base lock poisoned"))?
            .prompt_guidance(user_input, 5);
        let system_prompt = crate::prompt::PromptBuilder::new(&crate::prompt::load_personality())
            .with_language(&cfg.language)
            .with_project_instructions(&project_instructions)
            .with_tools(&tool_descriptions_text(tools))
            .with_tree_summary(&tree_summary)
            .with_vector_guidance(&vector_guidance)
            .with_runtime_env(&runtime_env)
            .with_session_context(&format!(
                "current run step {}/{}; cumulative session step {}; auto-continues used {}/{}",
                run_steps, max_steps, step, auto_continues, MAX_AUTO_CONTINUES
            ))
            .build();

        // 注入 system prompt padding
        match session.messages.first() {
            Some(m) if m.role == "system" => {
                session.messages[0].content = Some(system_prompt.clone());
            }
            _ => {
                session.messages.insert(
                    0,
                    Message {
                        role: "system".to_string(),
                        content: Some(system_prompt),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                );
            }
        }

        // ── 调用 LLM ──
        let mut assistant_text = String::new();
        let mut parsed_calls: Vec<ParsedToolCall> = Vec::new();
        let mut finish_reason: Option<String> = None;

        on_event(AgentEvent::Thinking);

        let result = tokio::select! {
            result = provider.chat_stream(&session.messages, &tool_defs, |ev| {
                match ev {
                    StreamEvent::ThinkingDelta(t) => {
                        on_event(AgentEvent::ThinkingDelta(t));
                    }
                    StreamEvent::TextDelta(t) => {
                        assistant_text.push_str(&t);
                        on_event(AgentEvent::TextDelta(t));
                    }
                    StreamEvent::ToolCallDone(tc) => { parsed_calls.push(tc); }
                    StreamEvent::Finish { finish_reason: reason, .. } => {
                        finish_reason = reason;
                    }
                }
            }) => Some(result),
            _ = wait_for_interrupt(interrupted) => None,
        };
        let Some(result) = result else {
            on_event(AgentEvent::Interrupted("user interrupted".into()));
            break;
        };

        let (ct_in, ct_out, stream_finish) = match result {
            Ok(tuple) => tuple,
            Err(e) => {
                on_event(AgentEvent::Error(e.to_string()));
                break;
            }
        };
        if finish_reason.is_none() {
            finish_reason = stream_finish;
        }

        total_tokens_in += ct_in;
        total_tokens_out += ct_out;
        step += 1;
        run_steps += 1;

        on_event(AgentEvent::TextDone {
            content: assistant_text.clone(),
            tokens_in: ct_in,
            tokens_out: ct_out,
        });

        // ── 组装 assistant message ──
        let tool_calls: Vec<ToolCall> = parsed_calls
            .iter()
            .map(|pc| ToolCall {
                id: pc.id.clone(),
                r#type: "function".to_string(),
                function: crate::provider::FunctionCall {
                    name: pc.name.clone(),
                    arguments: pc.arguments.clone(),
                },
            })
            .collect();

        if parsed_calls.is_empty() {
            session.messages.push(Message {
                role: "assistant".to_string(),
                content: Some(assistant_text.clone()),
                tool_calls: None,
                tool_call_id: None,
            });

            let truncated = finish_reason.as_deref() == Some("length");
            let should_nudge = truncated
                || assistant_text.trim().is_empty()
                || looks_like_incomplete_turn(&assistant_text);

            if should_nudge && auto_continues < MAX_AUTO_CONTINUES {
                auto_continues += 1;
                let nudge = if truncated {
                    "上一次输出因 max_tokens 被截断。请从中断处继续：立刻调用下一个所需工具，不要重复已完成的思考。"
                        .to_string()
                } else if assistant_text.trim().is_empty() {
                    "你刚才只做了内部思考，没有调用任何工具。请立即用工具继续执行任务（read/grep/vibe/tree/verify_command 等），不要只回复文字计划。"
                        .to_string()
                } else {
                    "任务尚未完成。请不要停在文字计划上——立刻调用下一个工具继续执行，并在验证通过后更新项目树状态。"
                        .to_string()
                };
                session.messages.push(Message {
                    role: "user".to_string(),
                    content: Some(nudge),
                    tool_calls: None,
                    tool_call_id: None,
                });
                continue;
            }

            on_event(AgentEvent::Done {
                tokens_in: total_tokens_in,
                tokens_out: total_tokens_out,
                steps: run_steps,
            });
            break;
        }

        // Successful tool-using turn resets soft continue budget slightly so long
        // jobs can keep self-driving after real progress.
        if auto_continues > 0 {
            auto_continues -= 1;
        }

        session.messages.push(Message {
            role: "assistant".to_string(),
            content: if assistant_text.is_empty() {
                None
            } else {
                Some(assistant_text.clone())
            },
            tool_calls: Some(tool_calls.clone()),
            tool_call_id: None,
        });

        // ── 执行工具 ──
        for tc in &parsed_calls {
            let input: serde_json::Value = match serde_json::from_str(&tc.arguments) {
                Ok(input) => input,
                Err(error) => {
                    let output = format!("invalid tool arguments: {error}");
                    on_event(AgentEvent::ToolCallStart {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: tc.arguments.clone(),
                    });
                    on_event(AgentEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        output: output.clone(),
                        exit_code: 1,
                        duration_ms: 0,
                    });
                    session.messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(output),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                    });
                    continue;
                }
            };
            on_event(AgentEvent::ToolCallStart {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: input.to_string(),
            });

            let start = Instant::now();
            let tree_version_before = project_tree
                .read()
                .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?
                .version();
            let authorization = {
                let manager = project_tree
                    .read()
                    .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?;
                authorize_tool_call(&tc.name, &input, &manager)
            };
            let result = if let Err(error) = authorization {
                crate::tools::ToolResult {
                    output: error.to_string(),
                    exit_code: 1,
                    duration_ms: 0,
                    truncated: false,
                }
            } else {
                let dispatched = tokio::select! {
                    result = tools.dispatch(&tc.name, input, &tool_ctx) => Some(result),
                    _ = wait_for_interrupt(interrupted) => None,
                };
                match dispatched {
                    None => crate::tools::ToolResult {
                        output: "user interrupted".to_string(),
                        exit_code: 130,
                        duration_ms: 0,
                        truncated: false,
                    },
                    Some(None) => crate::tools::ToolResult {
                        output: format!("unknown tool: {}", tc.name),
                        exit_code: 1,
                        duration_ms: 0,
                        truncated: false,
                    },
                    Some(Some(result)) => result,
                }
            };
            let ms = start.elapsed().as_millis() as u64;

            let output_text = truncate_tool_output(result.output);

            on_event(AgentEvent::ToolCallResult {
                id: tc.id.clone(),
                name: tc.name.clone(),
                output: output_text.clone(),
                exit_code: result.exit_code,
                duration_ms: ms,
            });

            session.messages.push(Message {
                role: "tool".to_string(),
                content: Some(output_text),
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
            });

            let tree_update = {
                let manager = project_tree
                    .read()
                    .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?;
                if manager.version() != tree_version_before {
                    Some((
                        manager.to_json()?,
                        manager.tree().clone(),
                        manager.version(),
                    ))
                } else {
                    None
                }
            };
            if let Some((json, tree, version)) = tree_update {
                session.project_tree = Some(tree);
                session.tree_version = version;
                if !session_store
                    .save_tree_cas(&session, tree_version_before)
                    .await?
                {
                    return Err(anyhow::anyhow!(
                        "tree changed concurrently; reload before continuing"
                    ));
                }
                on_event(AgentEvent::TreeUpdated { json });
            }
        }

        // ── 上下文剪枝 ──
        if step as usize >= prune_after {
            prune_messages(&mut session.messages, prune_keep);
        }
        // Next round starts with Thinking at top of loop — don't fire here.
    }

    // ── 持久化 ──
    session.step = step;
    session.tokens_in = total_tokens_in;
    session.tokens_out = total_tokens_out;
    {
        let manager = project_tree
            .read()
            .map_err(|_| anyhow::anyhow!("project tree lock poisoned"))?;
        session.project_tree = Some(manager.tree().clone());
        session.tree_version = manager.version();
    }
    session_store.save(&session).await?;
    Ok(())
}

fn tool_descriptions_text(registry: &ToolRegistry) -> String {
    let mut s = String::new();
    for d in registry.definitions() {
        s.push_str(&format!(
            "- {}: {}\n",
            d.function.name, d.function.description
        ));
    }
    s
}

fn truncate_tool_output(mut output: String) -> String {
    if output.len() <= TOOL_OUTPUT_MAX_CHARS {
        return output;
    }
    let mut truncate_at = TOOL_OUTPUT_MAX_CHARS;
    while truncate_at > 0 && !output.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    output.truncate(truncate_at);
    output.push_str("\n(output trimmed)");
    output
}

async fn wait_for_interrupt(interrupted: &AtomicBool) {
    while !interrupted.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn authorize_tool_call(
    tool_name: &str,
    input: &serde_json::Value,
    tree: &TreeManager,
) -> Result<()> {
    if tool_name != "vibe" {
        return Ok(());
    }
    let action = input
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(
        action,
        "new" | "split" | "meta" | "insert" | "replace" | "drop" | "assemble"
    ) {
        return Ok(());
    }
    let path = if action == "assemble" {
        input
            .get("args")
            .and_then(|args| args.get("out"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| input.get("path").and_then(serde_json::Value::as_str))
    } else {
        input.get("path").and_then(serde_json::Value::as_str)
    }
    .unwrap_or_default();

    // No project tree yet: allow edits so small tasks are not dead on arrival.
    // Once the human/AI creates a tree, edits must match an in_progress node.
    if tree.is_empty() {
        return Ok(());
    }

    tree.authorize_file_edit(path).map(|_| ()).map_err(|error| {
        anyhow!(
            "{error}. Fix: use the tree tool to set one node to `in_progress` with \
             target_files including `{path}`, then retry the vibe edit."
        )
    })
}

async fn load_or_create_session(
    store: &dyn SessionStore,
    id: &str,
    model: &str,
    provider: &str,
) -> Result<Session> {
    if let Some(s) = store.load(id).await? {
        return Ok(s);
    }
    Ok(Session::new(
        id.to_string(),
        model.to_string(),
        provider.to_string(),
    ))
}

/// 剪枝: 保留 system + first_user + 最近 N 个 assistant 消息(及它们的 tool 回复)
fn prune_messages(messages: &mut Vec<Message>, keep_assistant: usize) {
    if messages.len() <= keep_assistant * 2 + 3 {
        return;
    }

    // count assistant messages from end
    let mut assistant_seen = 0usize;
    let mut split_idx = messages.len();
    for (i, m) in messages.iter().enumerate().rev() {
        if m.role == "assistant" {
            assistant_seen += 1;
        }
        if assistant_seen >= keep_assistant {
            split_idx = i;
            break;
        }
    }
    if split_idx < 2 {
        return;
    } // must keep system + first_user

    let sys = messages[0].clone();
    let fu = messages[1].clone();
    let tail = messages.split_off(split_idx);
    let old_count = messages.len();
    messages.clear();
    messages.push(sys);
    messages.push(fu);
    messages.push(Message {
        role: "user".to_string(),
        content: Some(format!("[{} earlier tool turns pruned]", old_count / 2)),
        tool_calls: None,
        tool_call_id: None,
    });
    messages.extend(tail);
}

/// Heuristic: model wrote a plan / intent but did not emit tool_calls.
fn looks_like_incomplete_turn(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    if looks_like_final_summary(t) {
        return false;
    }
    let lower = t.to_lowercase();
    const MARKERS: &[&str] = &[
        "接下来我",
        "我先",
        "让我",
        "我会",
        "稍后",
        "然后我",
        "准备调用",
        "准备检查",
        "准备修改",
        "先看一下",
        "let me",
        "i'll ",
        "i will ",
        "next i",
        "going to",
        "i am going to",
    ];
    let has_marker = MARKERS
        .iter()
        .any(|m| lower.contains(m) || t.contains(m));
    if !has_marker {
        return false;
    }
    t.chars().count() < 420 || t.ends_with('…') || t.ends_with("...")
}

fn looks_like_final_summary(text: &str) -> bool {
    const DONE: &[&str] = &[
        "已完成",
        "验证通过",
        "全部完成",
        "任务完成",
        "修改完成",
        "completed successfully",
        "all done",
    ];
    let lower = text.to_lowercase();
    DONE.iter()
        .any(|m| text.contains(m) || lower.contains(m))
        && text.chars().count() > 40
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_unicode_tool_output_at_valid_boundary() {
        let source = "界".repeat(2_000);
        let output = truncate_tool_output(source);
        assert!(output.ends_with("(output trimmed)"));
        assert!(output.is_char_boundary(output.len()));
        assert!(output.len() <= TOOL_OUTPUT_MAX_CHARS + "\n(output trimmed)".len());
    }

    #[test]
    fn detects_incomplete_plan_without_tools() {
        assert!(looks_like_incomplete_turn("接下来我先用 grep 搜索一下。"));
        assert!(!looks_like_incomplete_turn(
            "已完成：已用 vibe verify 验证 src/foo.rs，并更新了树节点 code 为 completed。"
        ));
    }
}
