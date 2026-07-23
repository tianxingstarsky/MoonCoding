use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use regex::Regex;
use serde_json::json;

use crate::db::{self, Db};
use crate::llm::{LlmClient, Message, ToolCall, ToolDef, StreamEvent};
use crate::session::{self, Session};
use crate::tools;

const TOOL_OUTPUT_MAX_CHARS: usize = 16000; // 截断 LLM 看到的工具输出（非 create 输入上限）
const PRUNE_START_STEP: usize = 12;         // 从第 N 步开始删除旧 messages
const PRUNE_KEEP_RECENT: usize = 6;          // 保留最近 N 个 assistant+tool 对

pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub max_steps: u64,
    pub max_input_tokens: u64,
    pub vibe_exe: PathBuf,
}

pub async fn run_spec(
    root: &Path,
    spec: &crate::Spec,
    cfg: &Config,
    inherited_workspace: Option<&Path>,
) -> Result<String> {
    let run_id = session::now_run_id();
    let runs_root = root.join("runs");
    let ses = Session::new(&runs_root, &run_id)?;
    let db = db::init(&runs_root.join("vibe_test.db"))?;
    let created_at = Utc::now().to_rfc3339();

    let spec_id_safe = &spec.id;
    db::insert_session(&db, &run_id, spec_id_safe, &ses.workspace.to_string_lossy(), &cfg.model, &created_at)?;

    // 准备 workspace: 继承 (有) -> 否则 fixture/website -> 否则空 workspace + 一句 README
    if let Some(prev) = inherited_workspace {
        if Path::new(prev).exists() {
            session::inherit_workspace_into(&ses.workspace, prev)?;
        }
    } else {
        let fixture_ws = root.join("fixtures").join("website");
        if fixture_ws.exists() {
            ses.seed_from(&fixture_ws)?;
        }
    }

    let runner = Runner::new(cfg, &ses, &db, &run_id, spec);
    let outcome = runner.run().await;
    let ended_at = Utc::now().to_rfc3339();

    let mut tokens_in = 0u64; let mut tokens_out = 0u64;
    let mut tokens_total = 0u64;
    let mut steps_done = 0u64;
    let mut fileset_count = 0u64;
    let mut block_count = 0u64;
    let mut purpose_drift_warns = 0u64;
    let mut cross_block_warns = 0u64;
    let mut verify_failures = 0u64;
    let mut assertions_json = String::new();
    let status: String;

    match outcome {
        Ok(out) => {
            tokens_in = out.tokens_in;
            tokens_out = out.tokens_out;
            tokens_total = out.tokens_in + out.tokens_out;
            steps_done = out.steps;
            purpose_drift_warns = out.purpose_drift_warns;
            cross_block_warns = out.cross_block_warns;
            let (assertions, summary) = evaluate_assertions(&ses.workspace, &spec.assertions, &cfg.vibe_exe).await;
            fileset_count = summary.fileset_count;
            block_count = summary.block_count;
            verify_failures = summary.verify_failures;
            assertions_json = serde_json::to_string(&assertions).unwrap_or_default();
            if out.done {
                status = if summary.verify_failures == 0 && summary.all_assertions_pass { "done".into() }
                        else { "assertions_failed".into() };
            } else {
                status = "limit_hit".into();
            }
        }
        Err(e) => {
            eprintln!("error during run: {:#}", e);
            status = "error".into();
        }
    }

    // baseline: 把整文件基线估为 1.5x 总 token (整文件模式每次都附整文件 + 多轮重复读)
    let baseline_total = if tokens_total > 0 { tokens_total * 3 / 2 } else { 0 };

    db::finalize_session(
        &db, &run_id, tokens_in, tokens_out, tokens_total, baseline_total,
        steps_done, &status,
        fileset_count, block_count,
        purpose_drift_warns, cross_block_warns, verify_failures,
        &assertions_json, &ended_at,
    )?;

    Ok(run_id)
}

struct Runner<'a> {
    #[allow(dead_code)]
    cfg: &'a Config,
    ses: &'a Session,
    db: &'a Db,
    #[allow(dead_code)]
    run_id: &'a str,
    #[allow(dead_code)]
    spec: &'a crate::Spec,
    client: LlmClient,
    vibe_path: String,
}

#[derive(Default)]
struct Outcome {
    tokens_in: u64, tokens_out: u64,
    steps: u64,
    cross_block_warns: u64,
    purpose_drift_warns: u64,
    done: bool,
}

impl<'a> Runner<'a> {
    fn new(cfg: &'a Config, ses: &'a Session, db: &'a Db, run_id: &'a str, spec: &'a crate::Spec) -> Self {
        let client = LlmClient::new(cfg.api_key.clone(), cfg.base_url.clone(), cfg.model.clone())
            .expect("llm client init");
        let vibe_path = cfg.vibe_exe.to_string_lossy().to_string();
        Self { cfg, ses, db, run_id, spec, client, vibe_path }
    }

    async fn run(&self) -> Result<Outcome> {
        let system_prompt = crate::prompts::build_system_prompt()?;
        let tool_defs = vec![ToolDef {
            r#type: "function".to_string(),
            function: crate::llm::ToolFunction {
                name: "bash".to_string(),
                description: "Run a shell command inside the workspace sandbox. cwd is the workspace root.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "shell command line" },
                        "workdir": { "type": "string", "description": "optional subdirectory inside workspace" }
                    },
                    "required": ["command"]
                }),
            },
        }];
        // 把工作区路径注入 user message
let first_user = format!(
        "{}\n\nWorkdir: {}\nvibe binary: on PATH (call as `vibe ...` directly; do not search for it).\nYou are on Windows. Use forward slashes or backslashes for paths.\nStart immediately: vibe new for each required source file, then vibe insert blocks, then vibe assemble, then vibe verify.",
        self.spec.task,
        self.ses.workspace.display(),
    );
        let mut messages: Vec<Message> = Vec::new();
        messages.push(Message { role: "system".to_string(), content: Some(system_prompt), tool_calls: None, tool_call_id: None });
        messages.push(Message { role: "user".to_string(), content: Some(first_user), tool_calls: None, tool_call_id: None });

        let mut outcome = Outcome::default();
        let done_re = Regex::new(r"<done/>").unwrap();
        let cross_block_re = Regex::new(r"cross-block dep impact").unwrap();
        let purpose_drift_re = Regex::new(r"purpose drift cos=").unwrap();

        let mut step = 1u64;
        loop {
            if step > self.cfg.max_steps { break; }
            let total_in = outcome.tokens_in;
            if total_in > self.cfg.max_input_tokens { break; }

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            let mut this_in = 0u64; let mut this_out = 0u64;

            let _ = self.client.chat_stream(&messages, &tool_defs, |ev| {
                match ev {
                    StreamEvent::TextDelta(s) => {
                        print!("{}", s);
                        assistant_text.push_str(&s);
                    }
                    StreamEvent::ToolCallDone(tc) => {
                        tool_calls.push(tc);
                    }
                    StreamEvent::Finish { prompt_tokens, completion_tokens } => {
                        this_in = prompt_tokens; this_out = completion_tokens;
                    }
                }
            }).await?;
            outcome.tokens_in += this_in;
            outcome.tokens_out += this_out;
            outcome.steps = step;
            println!();

            // 把 assistant message 推进
            let asst_msg = Message {
                role: "assistant".to_string(),
                content: Some(assistant_text.clone()),
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls.clone()) },
                tool_call_id: None,
            };
            self.ses.log_step(step, "assistant", &assistant_text,
                tool_calls.first().map(|t| serde_json::to_string(t).unwrap_or_default()).as_deref(),
                None)?;
            db::insert_message(&self.db, &uuid::Uuid::new_v4().to_string(), self.run_id, step, "assistant",
                &serde_json::to_string(&asst_msg)?, this_in, this_out)?;
            messages.push(asst_msg);

            // 检查 <done/>: 模型显式宣告完成. 此时若没有 pending tool call 则退出循环.
            if done_re.is_match(&assistant_text) && tool_calls.is_empty() {
                outcome.done = true;
                break;
            }

            // 执行每个 tool call, 把结果 append 回 messages
            for tc in tool_calls {
                let tool_id = tc.id.clone();
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                let command = args.get("command").and_then(|s| s.as_str()).unwrap_or("");
                let workdir = args.get("workdir").and_then(|s| s.as_str());
                let tool_result = tools::bash(command, workdir, &self.ses.workspace, &self.vibe_path).await?;
                let truncated = tool_result.truncated;

let mut output_text = String::new();
            output_text.push_str(&format!("cwd: {}\n", tool_result.cwd));
                output_text.push_str(&format!("exit {}\n", tool_result.exit_code));
                if !tool_result.stdout.is_empty() {
                    output_text.push_str("--- stdout ---\n");
                    output_text.push_str(&tool_result.stdout);
                }
                if !tool_result.stderr.is_empty() {
                    output_text.push_str("--- stderr ---\n");
                    output_text.push_str(&tool_result.stderr);
                }
                if truncated { output_text.push_str("\n(output truncated at 1 MiB)\n"); }

                if output_text.len() > TOOL_OUTPUT_MAX_CHARS {
                    output_text.truncate(TOOL_OUTPUT_MAX_CHARS);
                    output_text.push_str("\n(output trimmed for context)\n");
                }

                if cross_block_re.is_match(&output_text) { outcome.cross_block_warns += 1; }
                if purpose_drift_re.is_match(&output_text) { outcome.purpose_drift_warns += 1; }

                self.ses.log_step(step, "tool", &command,
                    None,
                    Some(&output_text))?;
                db::insert_tool_call(&self.db, &uuid::Uuid::new_v4().to_string(), self.run_id, "", step,
                    command, tool_result.exit_code, &output_text, truncated, tool_result.duration_ms)?;

                let tool_msg = Message {
                    role: "tool".to_string(),
                    content: Some(output_text.clone()),
                    tool_calls: None,
                    tool_call_id: Some(tool_id),
                };
                messages.push(tool_msg);

                // prune old messages: after PRUNE_START_STEP, collapse early history by counting completed assistant turns
                // always keep system + first_user + last N ASSISTANT messages (with their tool responses) intact
                if step as usize >= PRUNE_START_STEP {
                    // count assistant messages from the end
                    let mut assistant_seen = 0usize;
                    let mut split_idx = messages.len();
                    for (i, m) in messages.iter().enumerate().rev() {
                        if m.role == "assistant" {
                            assistant_seen += 1;
                            if assistant_seen >= PRUNE_KEEP_RECENT {
                                split_idx = i;
                                break;
                            }
                        }
                    }
                    // if we can trim at least some messages and keep system+first_user
                    if split_idx > 2 {
                        let sys = messages[0].clone();
                        let fu = messages[1].clone();
                        let old_count = messages.len();
                        let tail = messages.split_off(split_idx);
                        messages.clear();
                        messages.push(sys);
                        messages.push(fu);
                        messages.push(Message {
                            role: "user".to_string(),
                            content: Some(format!("[{} earlier tool calls pruned; continuing from step {}]", (old_count - tail.len()) / 2, step)),
                            tool_calls: None, tool_call_id: None,
                        });
                        messages.extend(tail);
                    }
                }
            }
            step += 1;
        }

        Ok(outcome)
    }
}

#[derive(Default)]
struct AssertionSummary {
    all_assertions_pass: bool,
    fileset_count: u64,
    block_count: u64,
    verify_failures: u64,
}

async fn evaluate_assertions(workspace: &Path, assertions: &serde_json::Value, vibe_exe: &Path) -> (Vec<serde_json::Value>, AssertionSummary) {
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut summary = AssertionSummary::default();

    // 扫 .vibe 数 fileset + 计 block
    let vibe_root = workspace.join(".vibe");
    if vibe_root.is_dir() {
        if let Ok(entries) = fs::read_dir(&vibe_root) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("vibe") {
                    summary.fileset_count += 1;
                    let idx = p.join("index.json");
                    if let Ok(txt) = fs::read_to_string(&idx) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                            if let Some(blocks) = v.get("blocks").and_then(|b| b.as_array()) {
                                summary.block_count += blocks.len() as u64;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(arr) = assertions.as_array() {
        for a in arr {
            let name = a.get("name").and_then(|s| s.as_str()).unwrap_or("(unnamed)").to_string();
            let kind = a.get("kind").and_then(|s| s.as_str()).unwrap_or("");
            let pass: bool;
            let mut detail = String::new();
            match kind {
                "file_exists" => {
                    let files = a.get("files").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let mut missing = Vec::new();
                    for f in files {
                        let s = f.as_str().unwrap_or("");
                        if !workspace.join(s).exists() { missing.push(s.to_string()); }
                    }
                    pass = missing.is_empty();
                    if !pass { detail = format!("missing: {}", missing.join(", ")); }
                }
                "verify_exit_zero" => {
                    let files = a.get("files").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let mut fails = Vec::new();
                    for f in files {
                        let s = f.as_str().unwrap_or("");
                        let exit = vibe_verify_exit_code(vibe_exe, s, workspace);
                        if exit != 0 {
                            summary.verify_failures += 1;
                            fails.push(format!("{}({})", s, exit));
                        }
                    }
                    pass = fails.is_empty();
                    if !pass { detail = format!("non-zero: {}", fails.join(", ")); }
                }
                _ => { pass = true; }
            }
            results.push(json!({
                "name": name,
                "pass": pass,
                "detail": detail,
            }));
        }
    }
    summary.all_assertions_pass = results.iter().all(|r| r.get("pass").and_then(|p| p.as_bool()).unwrap_or(false));
    (results, summary)
}

fn vibe_verify_exit_code(vibe_exe: &Path, posix_path: &str, workspace: &Path) -> i32 {
    let exe = if vibe_exe.exists() { vibe_exe.to_path_buf() } else { PathBuf::from("vibe") };
    let out = std::process::Command::new(&exe)
        .arg("verify")
        .arg(posix_path)
        .current_dir(workspace)
        .output();
    match out {
        Ok(o) => o.status.code().unwrap_or(-1),
        Err(_) => -1,
    }
}