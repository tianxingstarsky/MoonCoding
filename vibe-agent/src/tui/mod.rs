use anyhow::Result;
use crossterm::{
    execute,
    style::{Color as CColor, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{stdout, BufRead, Write};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::config::Config;
use crate::session::SessionStore;
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;
mod markdown;
mod syntax;

const ACC:   CColor = CColor::Rgb { r: 92,  g: 156, b: 245 };
const TXT:   CColor = CColor::Rgb { r: 224, g: 224, b: 224 };
const MUTED: CColor = CColor::Rgb { r: 96,  g: 96,  b: 96 };
const OK:    CColor = CColor::Rgb { r: 126, g: 207, b: 126 };
const ERR:   CColor = CColor::Rgb { r: 224, g: 80,  b: 80 };

fn clr(c: CColor) -> SetForegroundColor { SetForegroundColor(c) }

static MODEL_CACHE: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    let mut out = stdout();
    execute!(out, clr(ACC))?; write!(out, "vibe-agent")?;
    execute!(out, clr(TXT))?; writeln!(out, " · {}", cfg.provider.model)?;
    execute!(out, clr(MUTED))?; writeln!(out, "type /help for commands, or ask anything")?;
    execute!(out, ResetColor)?; out.flush()?;

    let (tx, rx) = mpsc::channel::<AgentEvent>();
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut agent_thread: Option<std::thread::JoinHandle<()>> = None;
    let mut picking_model = false;  // set by /model, cleared after selection

    loop {
        // ── drain any pending events ──
        while let Ok(ev) = rx.try_recv() { show(&ev)?; }

        // ── join finished agent ──
        if let Some(h) = agent_thread.take() {
            let _ = h.join();
            while let Ok(ev) = rx.try_recv() { show(&ev)?; }
        }

        // ── prompt ──
        let prompt_label = if picking_model { "pick model> " } else { "vibe-agent> " };
        execute!(out, clr(ACC))?; write!(out, "{}", prompt_label)?;
        execute!(out, ResetColor)?; out.flush()?;

        let line = match lines.next() {
            Some(Ok(l)) => l.trim().to_string(),
            _ => break,
        };
        if line.is_empty() { continue; }

        // ── built-in commands ──
        match line.as_str() {
            "/exit" | "/quit" | "/q" => break,
            "/model" | "/models" => {
                let models = fetch_models(&cfg.provider.base_url, &cfg.provider.api_key).await;
                *MODEL_CACHE.lock().unwrap() = models.clone();
                for (i, name) in models.iter().enumerate() {
                    execute!(out, clr(ACC))?; write!(out, "  [{}] ", i+1)?;
                    execute!(out, clr(TXT))?; writeln!(out, "{}", name)?;
                }
                execute!(out, clr(MUTED))?; writeln!(out, "  type a number to pick")?;
                execute!(out, ResetColor)?; out.flush()?;
                picking_model = true;
                continue;
            }
            "/help" => {
                execute!(out, clr(ACC))?; writeln!(out, "  commands:")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /model        list available models from API")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  <number>      pick a model by number (after /model)")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /status       show current model & config")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /key <sk-..>  set API key")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /clear        clear screen")?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /exit         quit")?;
                execute!(out, clr(MUTED))?; writeln!(out, "  anything else is sent to the AI agent")?;
                execute!(out, clr(MUTED))?; writeln!(out, "  ctrl+c = exit  ·  ctrl+d = exit")?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            s if picking_model && s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit()) => {
                picking_model = false;
                let idx = s.parse::<usize>().unwrap_or(0);
                let cache = MODEL_CACHE.lock().unwrap();
                if idx > 0 && idx <= cache.len() {
                    execute!(out, clr(OK))?; writeln!(out, "  model => {} (restart to apply)", cache[idx-1])?;
                } else {
                    execute!(out, clr(ERR))?; writeln!(out, "  invalid number, run /model first")?;
                }
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            s if s.starts_with("/key ") => {
                execute!(out, clr(OK))?; writeln!(out, "  key set")?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            "/status" => {
                execute!(out, clr(TXT))?; writeln!(out, "  model: {}", cfg.provider.model)?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            "/clear" => {
                execute!(out, Clear(ClearType::All))?; out.flush()?;
                continue;
            }
            prompt => {
                picking_model = false; // exit selection mode on any non-number input
                let tx2 = tx.clone(); let t2 = tools.clone(); let s2 = store.clone();
                let c2 = cfg.clone(); let sid = uuid::Uuid::new_v4().to_string();
                let p = prompt.to_string();
                agent_thread = Some(std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        let _ = crate::agent::run_agent(&c2, &t2, s2.as_ref(), &p, &sid,
                            &mut |ev| { tx2.send(ev).ok(); }).await;
                    });
                }));
            }
        }
    }

    Ok(())
}

fn show(ev: &AgentEvent) -> Result<()> {
    let mut o = stdout();
    match ev {
        AgentEvent::TextDelta(t) => {
            for line in markdown::render_markdown(t) {
                for span in line.spans {
                    write!(o, "{}", span.content)?;
                }
                writeln!(o)?;
            }
            o.flush()?;
        }
        AgentEvent::TextDone { .. } => { writeln!(o)?; execute!(o, ResetColor)?; o.flush()?; }
        AgentEvent::ToolCallStart { name, input, .. } => {
            let p: String = input.chars().take(80).collect();
            execute!(o, clr(ACC))?; write!(o, "  {} ", name)?;
            execute!(o, clr(MUTED))?; writeln!(o, "{}", p)?;
            execute!(o, ResetColor)?; o.flush()?;
        }
        AgentEvent::ToolCallResult { name, output, exit_code, .. } => {
            let cc = if *exit_code == 0 { OK } else { ERR };
            execute!(o, clr(MUTED))?; write!(o, "    {}  ", name)?;
            execute!(o, clr(cc))?; writeln!(o, "exit {}", exit_code)?;
            for l in output.lines().take(3) {
                execute!(o, clr(MUTED))?; writeln!(o, "    {}", l)?;
            }
            execute!(o, ResetColor)?; o.flush()?;
        }
        AgentEvent::Done { steps, tokens_in, tokens_out } => {
            execute!(o, clr(OK))?; write!(o, "  done  ")?;
            execute!(o, clr(TXT))?; writeln!(o, "{} steps  {}/{} tokens", steps, tokens_in, tokens_out)?;
            execute!(o, ResetColor)?; o.flush()?;
        }
        AgentEvent::Error(e) => {
            execute!(o, clr(ERR))?; writeln!(o, "  {}", e)?;
            execute!(o, ResetColor)?; o.flush()?;
        }
        _ => {}
    }
    Ok(())
}

/// 从 OpenAI 兼容 API 的 /models 端点获取真实模型列表
async fn fetch_models(base_url: &str, api_key: &str) -> Vec<String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }
    match req.send().await {
        Ok(resp) => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if let Some(arr) = body.get("data").and_then(|d| d.as_array()) {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}