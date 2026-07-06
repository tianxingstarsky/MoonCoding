use anyhow::Result;
use std::time::Instant;

use crate::config::Config;
use crate::provider::{Message, OpenAiCompatible, ParsedToolCall, StreamEvent, ToolCall};
use crate::session::{Session, SessionStore};
use crate::stream::AgentEvent;
use crate::tools::todowrite;
use crate::tools::{ToolContext, ToolRegistry};

const TOOL_OUTPUT_MAX_CHARS: usize = 4000;
const PRUNE_START_STEP: usize = 12;
const PRUNE_KEEP_ASSISTANT: usize = 6;

/// 核心 agent 循环 —— 单次用户输入 → 流式 LLM → 工具执行 → 循环到 done
pub async fn run_agent(
    cfg: &Config,
    tools: &ToolRegistry,
    session_store: &dyn SessionStore,
    user_input: &str,
    on_event: &mut dyn FnMut(AgentEvent),
) -> Result<()> {
    on_event(AgentEvent::Thinking);

    let provider = OpenAiCompatible::new(
        cfg.provider.base_url.clone(), cfg.provider.model.clone(), cfg.provider.api_key.clone(),
        cfg.provider.max_tokens, cfg.provider.temperature,
    )?;

    let mut session = load_or_create_session(session_store, &cfg.provider.model, &cfg.provider.base_url).await?;
    session.messages.push(Message {
        role: "user".to_string(),
        content: Some(user_input.to_string()),
        tool_calls: None, tool_call_id: None,
    });

    let tool_defs = tools.definitions();
    let tool_ctx = ToolContext {
        workspace: std::env::current_dir().unwrap_or_default(),
        vibe_exe: cfg.vibe_exe.clone(),
        session_id: session.id.clone(),
    };

    let max_steps = cfg.agent.max_steps.unwrap_or(40);
    let prune_after = cfg.agent.prune_after.unwrap_or(PRUNE_START_STEP);
    let prune_keep = cfg.agent.prune_keep.unwrap_or(PRUNE_KEEP_ASSISTANT);
    let mut total_tokens_in = session.tokens_in;
    let mut total_tokens_out = session.tokens_out;
    let mut step = session.step;

    loop {
        if step >= max_steps {
            on_event(AgentEvent::Interrupted(format!("max_steps={}", max_steps)));
            break;
        }

        // ── 构建 system prompt ──
        let system_prompt = crate::prompt::PromptBuilder::new(&crate::prompt::load_personality())
            .with_tools(&tool_descriptions_text(tools))
            .with_tree_summary(&todowrite::current_summary())
            .with_session_context(&format!("step {}/{}", step, max_steps))
            .build();

        // 注入 system prompt 到第一条消息 (替换旧的)
        if let Some(first) = session.messages.first_mut() {
            if first.role == "system" { first.content = Some(system_prompt.clone()); }
        } else {
            session.messages.insert(0, Message { role: "system".to_string(), content: Some(system_prompt), tool_calls: None, tool_call_id: None });
        }

        // ── 调用 LLM ──
        let mut assistant_text = String::new();
        let mut parsed_calls: Vec<ParsedToolCall> = Vec::new();
        let mut this_in = 0u64; let mut this_out = 0u64;

        let result = provider.chat_stream(&session.messages, &tool_defs, |ev| {
            match ev {
                StreamEvent::TextDelta(t) => {
                    assistant_text.push_str(&t);
                    on_event(AgentEvent::TextDelta(t));
                }
                StreamEvent::ToolCallDone(tc) => { parsed_calls.push(tc); }
                StreamEvent::Finish { prompt_tokens, completion_tokens } => {
                    this_in = prompt_tokens; this_out = completion_tokens;
                }
            }
        }).await;

        let (ct_in, ct_out) = match result {
            Ok((in_t, out_t)) => (in_t, out_t),
            Err(e) => {
                on_event(AgentEvent::Error(e.to_string()));
                break;
            }
        };

        total_tokens_in += ct_in;
        total_tokens_out += ct_out;
        step += 1;

        on_event(AgentEvent::TextDone {
            content: assistant_text.clone(),
            tokens_in: ct_in, tokens_out: ct_out,
        });

        // ── 组装 assistant message ──
        let tool_calls: Vec<ToolCall> = parsed_calls.iter().map(|pc| ToolCall {
            id: pc.id.clone(),
            r#type: "function".to_string(),
            function: crate::provider::FunctionCall { name: pc.name.clone(), arguments: pc.arguments.clone() },
        }).collect();

        if parsed_calls.is_empty() {
            // 模型返回纯文本 → 本轮结束
            session.messages.push(Message {
                role: "assistant".to_string(),
                content: Some(assistant_text.clone()),
                tool_calls: None, tool_call_id: None,
            });
            on_event(AgentEvent::Done { tokens_in: total_tokens_in, tokens_out: total_tokens_out, steps: step });
            break;
        }

        session.messages.push(Message {
            role: "assistant".to_string(),
            content: if assistant_text.is_empty() { None } else { Some(assistant_text.clone()) },
            tool_calls: Some(tool_calls.clone()), tool_call_id: None,
        });

        // ── 执行工具 ──
        for tc in &parsed_calls {
            let input = serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            on_event(AgentEvent::ToolCallStart { id: tc.id.clone(), name: tc.name.clone(), input: serde_json::to_string(&input).unwrap_or_default() });

            let start = Instant::now();
            let result = tools.dispatch(&tc.name, input, &tool_ctx).await.unwrap_or_else(|| {
                super::tools::ToolResult { output: format!("unknown tool: {}", tc.name), exit_code: 1, duration_ms: 0, truncated: false }
            });
            let ms = start.elapsed().as_millis() as u64;

            let mut output_text = result.output;
            if output_text.len() > TOOL_OUTPUT_MAX_CHARS {
                output_text.truncate(TOOL_OUTPUT_MAX_CHARS);
                output_text.push_str("\n(output trimmed)");
            }

            on_event(AgentEvent::ToolCallResult {
                id: tc.id.clone(), name: tc.name.clone(), output: output_text.clone(),
                exit_code: result.exit_code, duration_ms: ms,
            });

            session.messages.push(Message {
                role: "tool".to_string(),
                content: Some(output_text),
                tool_calls: None, tool_call_id: Some(tc.id.clone()),
            });
        }

        // ── 上下文剪枝 ──
        if step as usize >= prune_after {
            prune_messages(&mut session.messages, prune_keep);
        }

        // 多轮继续
        on_event(AgentEvent::Thinking);
    }

    // ── 持久化 ──
    session.step = step;
    session.tokens_in = total_tokens_in;
    session.tokens_out = total_tokens_out;
    session_store.save(&session).await?;
    Ok(())
}

fn tool_descriptions_text(registry: &ToolRegistry) -> String {
    let mut s = String::new();
    for d in registry.definitions() {
        s.push_str(&format!("- {}: {}\n", d.function.name, d.function.description));
    }
    s
}

async fn load_or_create_session(store: &dyn SessionStore, model: &str, provider: &str) -> Result<Session> {
    if let Some(id) = store.latest().await? {
        if let Some(s) = store.load(&id).await? {
            return Ok(s);
        }
    }
    Ok(Session {
        id: uuid::Uuid::new_v4().to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        messages: Vec::new(),
        step: 0, tokens_in: 0, tokens_out: 0,
        project_tree: None, tree_version: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        metadata: std::collections::HashMap::new(),
    })
}

/// 剪枝: 保留 system + first_user + 最近 N 个 assistant 消息(及它们的 tool 回复)
fn prune_messages(messages: &mut Vec<Message>, keep_assistant: usize) {
    if messages.len() <= keep_assistant * 2 + 3 { return; }

    // count assistant messages from end
    let mut assistant_seen = 0usize;
    let mut split_idx = messages.len();
    for (i, m) in messages.iter().enumerate().rev() {
        if m.role == "assistant" { assistant_seen += 1; }
        if assistant_seen >= keep_assistant { split_idx = i; break; }
    }
    if split_idx < 2 { return; } // must keep system + first_user

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
        tool_calls: None, tool_call_id: None,
    });
    messages.extend(tail);
}