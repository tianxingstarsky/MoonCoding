use anyhow::Result;
use crossterm::{
    execute,
    style::{Color as CColor, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{stdout, BufRead, Write};
use std::sync::atomic::{AtomicU8, Ordering};
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
static LANG: AtomicU8 = AtomicU8::new(0); // 0=en 1=zh

fn t(en: &str, zh: &str) -> String { if LANG.load(Ordering::Relaxed) == 0 { en.to_string() } else { zh.to_string() } }

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    let mut out = stdout();
    execute!(out, clr(ACC))?; write!(out, "vibe-agent")?;
    execute!(out, clr(TXT))?; writeln!(out, " · {}", cfg.provider.model)?;
    execute!(out, clr(MUTED))?; writeln!(out, "{}", t("type /help for commands, or ask anything", "输入 /help 查看命令, 或直接提问"))?;
    execute!(out, ResetColor)?; out.flush()?;

    let (tx, rx) = mpsc::channel::<AgentEvent>();
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut agent_thread: Option<std::thread::JoinHandle<()>> = None;
    let mut picking_model = false;
    let mut text_buf = String::new();  // accumulate streaming text

    loop {
        // ── drain any pending events ──
        while let Ok(ev) = rx.try_recv() {
            match &ev {
                AgentEvent::TextDelta(t) => { text_buf.push_str(t); }
                AgentEvent::TextDone { .. } => {
                    let md_lines = markdown::render_markdown(&text_buf);
                    for line in &md_lines {
                        for span in &line.spans {
                            let style = &span.style;
                            let c = style.fg.unwrap_or(ratatui::style::Color::Rgb(224,224,224));
                            let (r,g,b) = match c {
                                ratatui::style::Color::Rgb(r,g,b) => (r,g,b),
                                ratatui::style::Color::White => (224,224,224),
                                ratatui::style::Color::Yellow => (224,180,100),
                                ratatui::style::Color::Cyan => (92,156,245),
                                ratatui::style::Color::Green => (126,207,126),
                                ratatui::style::Color::Red => (224,80,80),
                                _ => (224,224,224),
                            };
                            execute!(out, SetForegroundColor(CColor::Rgb { r, g, b }))?;
                            write!(out, "{}", span.content)?;
                        }
                        writeln!(out)?;
                    }
                    execute!(out, ResetColor)?; out.flush()?;
                    text_buf.clear();
                }
                _ => { show(&ev)?; }
            }
        }

        // ── join finished agent ──
        if let Some(h) = agent_thread.take() {
            let _ = h.join();
            while let Ok(ev) = rx.try_recv() { show(&ev)?; }
        }

        // ── prompt ──
        let prompt_label = if picking_model { t("pick model> ", "选模型> ") } else { "vibe-agent> ".to_string() };
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
                execute!(out, clr(MUTED))?; writeln!(out, "  {}", t("type a number to pick", "输入数字选择"))?;
                execute!(out, ResetColor)?; out.flush()?;
                picking_model = true;
                continue;
            }
            "/help" => {
                execute!(out, clr(ACC))?; writeln!(out, "  {}", t("commands:", "命令:"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /model        {}", t("list available models from API", "从 API 获取可用模型列表"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  <number>      {}", t("pick a model by number (after /model)", "用数字选模型(执行 /model 后)"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /status       {}", t("show current model & config", "查看当前模型和配置"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /key <sk-..>  {}", t("set API key", "设置 API 密钥"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /clear        {}", t("clear screen", "清屏"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /exit         {}", t("quit", "退出"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /zh           {}", t("switch to Chinese", "切换到中文"))?;
                execute!(out, clr(TXT))?;  writeln!(out, "  /en           {}", t("switch to English", "切换到英文"))?;
                execute!(out, clr(MUTED))?; writeln!(out, "  {}", t("anything else is sent to the AI agent", "其他内容直接发给 AI"))?;
                execute!(out, clr(MUTED))?; writeln!(out, "  {}  ·  {}", t("ctrl+c = exit", "ctrl+c = 退出"), t("ctrl+d = exit", "ctrl+d = 退出"))?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            s if picking_model && s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit()) => {
                picking_model = false;
                let idx = s.parse::<usize>().unwrap_or(0);
                let cache = MODEL_CACHE.lock().unwrap();
                if idx > 0 && idx <= cache.len() {
                    execute!(out, clr(OK))?; writeln!(out, "  {} => {} ({})", t("model", "模型"), cache[idx-1], t("restart to apply", "重启后生效"))?;
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
            "/zh" => { LANG.store(1, Ordering::Relaxed); execute!(out, clr(OK))?; writeln!(out, "  已切换为中文")?; execute!(out, ResetColor)?; out.flush()?; continue; }
            "/en" => { LANG.store(0, Ordering::Relaxed); execute!(out, clr(OK))?; writeln!(out, "  switched to English")?; execute!(out, ResetColor)?; out.flush()?; continue; }
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