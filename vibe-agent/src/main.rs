mod agent;
mod config;
mod prompt;
mod provider;
mod session;
mod stream;
mod tools;
mod tui;

use anyhow::Result;
use colored::*;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config as RustyConfig, Context, EditMode, Editor, Helper};
use rustyline::history::DefaultHistory;
use std::path::PathBuf;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Config;
use crate::session::{SessionStore, SqliteStore};
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;
use crate::tools::bash::BashTool;
use crate::tools::glob::GlobTool;
use crate::tools::grep::GrepTool;
use crate::tools::read::ReadTool;
use crate::tools::todowrite::TodoWriteTool;
use crate::tools::vibe::VibeTool;

struct MoonHelper {
    completer: FilenameCompleter,
}
impl Highlighter for MoonHelper {}
impl Hinter for MoonHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> { None }
}
impl Validator for MoonHelper {}
impl Completer for MoonHelper {
    type Candidate = Pair;
    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }
}
impl Helper for MoonHelper {}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all().build().expect("tokio");
    rt.block_on(async {
        if let Err(e) = run().await {
            eprintln!("{} {}", "✗".red().bold(), e.to_string().red());
            exit(1);
        }
    });
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut cmd = String::from("chat");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-C" | "--workdir" => {
                if let Some(dir) = args.get(i+1) { std::env::set_current_dir(PathBuf::from(dir))?; }
                i += 2;
            }
            "--base-url" | "--model" | "--api-key" => { i += 2; }
            s if !s.starts_with('-') => { cmd = s.to_string(); i += 1; }
            _ => { i += 1; }
        }
    }
    let root = find_project_root(&std::env::current_dir().unwrap_or_default());
    std::env::set_current_dir(&root)?;
    let mut cfg = Config::load(&root)?;

    // apply CLI flag overrides to config
    let mut j = 1;
    while j < args.len() {
        match args[j].as_str() {
            "--base-url" => { cfg.provider.base_url = args.get(j+1).cloned().unwrap_or_default(); j += 2; }
            "--model"    => { cfg.provider.model = args.get(j+1).cloned().unwrap_or_default(); j += 2; }
            "--api-key"  => { cfg.provider.api_key = args.get(j+1).cloned().unwrap_or_default(); j += 2; }
            _ => { j += 1; }
        }
    }

    if cfg.provider.api_key.is_empty() && cmd != "tui" {
        eprintln!("{}  Set {}", "WARN".yellow().bold(), "MOONCODING_API_KEY".cyan());
        eprintln!("   or pass --api-key <key>");
    }

    let session_store = SqliteStore::new(&cfg.session_dir.join("sessions.db"))?;
    let tools = build_tools();

    match cmd.as_str() {
        "list"   => cmd_list(&session_store).await,
        "resume" => cmd_resume(args.get(2).cloned().unwrap_or_default(), &cfg, &tools, &session_store).await,
        "new"    => cmd_new_session(&cfg, &tools, &session_store).await,
        "tui"    => cmd_tui(cfg, tools, session_store).await,
        _        => cmd_chat(&cfg, &tools, &session_store).await,
    }
}

async fn cmd_chat(cfg: &Config, tools: &ToolRegistry, store: &dyn SessionStore) -> Result<()> {
    let (session_id, resumed) = if let Some(id) = store.latest().await? { (id, true) } else { (uuid::Uuid::new_v4().to_string(), false) };
    print_welcome(cfg, &session_id, resumed, store).await;
    repl_loop(cfg, tools, store, &session_id).await
}

async fn cmd_list(store: &dyn SessionStore) -> Result<()> {
    let ids = store.list().await?;
    if ids.is_empty() { println!("  (no saved sessions)"); return Ok(()); }
    println!("{}", "─ sessions ─".dimmed());
    for id in &ids {
        if let Some(s) = store.load(id).await? {
            let preview = s.messages.iter().filter(|m| m.role=="user").last()
                .and_then(|m| m.content.as_deref()).unwrap_or("").chars().take(60).collect::<String>();
            let tag = if s.step > 0 { format!("step={}", s.step) } else { "new".into() };
            println!("  {}  {}  {}  {}",
                &id[..8.min(id.len())].cyan(), tag.dimmed(),
                format!("{}/{}t", s.tokens_in, s.tokens_out).dimmed(), preview.dimmed());
        }
    }
    Ok(())
}

async fn cmd_resume(id: String, cfg: &Config, tools: &ToolRegistry, store: &dyn SessionStore) -> Result<()> {
    if let Some(_s) = store.load(&id).await? {
        print_welcome(cfg, &id, true, store).await;
        repl_loop(cfg, tools, store, &id).await
    } else { eprintln!("{} not found", "✗".red()); Ok(()) }
}

async fn cmd_new_session(cfg: &Config, tools: &ToolRegistry, store: &dyn SessionStore) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    print_welcome(cfg, &id, false, store).await;
    repl_loop(cfg, tools, store, &id).await
}

async fn cmd_tui(cfg: Config, tools: ToolRegistry, store: SqliteStore) -> Result<()> {
    tui::run(Arc::new(cfg), Arc::new(tools), Arc::new(store)).await
}

// ── REPL ──

async fn repl_loop(cfg: &Config, tools: &ToolRegistry, store: &dyn SessionStore, session_id: &str) -> Result<()> {
    let rl_cfg = RustyConfig::builder().history_ignore_space(true).edit_mode(EditMode::Vi).build();
    let mut rl = Editor::<MoonHelper, DefaultHistory>::with_config(rl_cfg)?;
    rl.set_helper(Some(MoonHelper { completer: FilenameCompleter::default() }));
    let history_path = cfg.session_dir.join("history");
    let _ = rl.load_history(&history_path);

    let interrupted = Arc::new(AtomicBool::new(false));

    loop {
        let prompt = format!("{} ", "moon>".green().bold());
        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() { continue; }
                rl.add_history_entry(&line)?;
                if line.starts_with('/') {
                    let quit = handle_command(&line, cfg, store, session_id).await?;
                    if quit { break; }
                    continue;
                }
                let flag = interrupted.clone();
                let result = agent::run_agent(cfg, tools, store, &line, session_id, &mut |ev| render_event(ev, &flag)).await;
                if let Err(e) = result { eprintln!("\n{} {}", "✗".red(), e.to_string().red()); }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                interrupted.store(true, Ordering::SeqCst);
                println!("\n{}  type new prompt or /exit", "⏸".yellow());
                interrupted.store(false, Ordering::SeqCst);
            }
            Err(rustyline::error::ReadlineError::Eof) => { println!("\n{} Goodbye.", "✓".green()); break; }
            Err(e) => { eprintln!("{}", e); break; }
        }
        let _ = rl.save_history(&history_path);
    }
    Ok(())
}

async fn handle_command(line: &str, _cfg: &Config, store: &dyn SessionStore, current_id: &str) -> Result<bool> {
    let parts: Vec<&str> = line[1..].split_whitespace().collect();
    if parts.is_empty() { return Ok(false); }
    match parts[0] {
        "exit" | "quit" | "q" => { println!("{} Goodbye.", "✓".green()); return Ok(true); }
        "help" | "h" => {
            println!("{}", "─ commands ─".bold());
            println!("  {:<20} exit", "/exit, /q".cyan());
            println!("  {:<20} help", "/help".cyan());
            println!("  {:<20} list sessions", "/sessions".cyan());
            println!("  {:<20} current status", "/status".cyan());
        }
        "sessions" => { cmd_list(store).await?; }
        "status" => {
            println!("{}", "─ status ─".bold());
            println!("  session  : {}", &current_id[..8.min(current_id.len())].cyan());
            if let Some(s) = store.load(current_id).await? {
                println!("  steps    : {}", s.step);
                println!("  tokens   : in {} / out {}", s.tokens_in, s.tokens_out);
            }
        }
        _ => { eprintln!("{}  try /help", "?".yellow()); }
    }
    Ok(false)
}

// ── rendering ──

async fn print_welcome(cfg: &Config, session_id: &str, resumed: bool, store: &dyn SessionStore) {
    let tag = if resumed { "resumed".dimmed() } else { "new".dimmed() };
    let line1 = format!("--- vibe-agent {} {} {} session {} @ {} ---",
        cfg.provider.model.cyan(), tag,
        "session".dimmed(), &session_id[..8].dimmed(), cfg.provider.base_url.dimmed());
    println!("{}", line1);
    println!("  {}  {}",
        format!("max {}", cfg.agent.max_steps.unwrap_or(40)).dimmed(), "/help . Ctrl+D".dimmed());
    if resumed {
        if let Ok(Some(s)) = store.load(session_id).await {
            println!("  {}", format!("restored step={} tok={}/{}", s.step, s.tokens_in, s.tokens_out).dimmed());
        }
    }
    let bot = "-".repeat(line1.len().min(80));
    println!("{}", bot);
    println!();
}

fn render_event(ev: AgentEvent, interrupted: &AtomicBool) {
    if interrupted.load(Ordering::SeqCst) { return; }
    match ev {
        AgentEvent::Thinking => { eprint!("{} ", "⏳".yellow()); }
        AgentEvent::TextDelta(t) => { eprint!("{}", t); }
        AgentEvent::TextDone { tokens_in, tokens_out, .. } => {
            eprintln!("\n{}", format!("  ── {}/{} tokens ──", tokens_in, tokens_out).dimmed());
        }
        AgentEvent::ToolCallStart { name, input: inp, .. } => {
            let preview: String = inp.chars().take(80).collect();
            eprintln!("  [{}] {} {}",
                name.bright_cyan().bold(), "args:".dimmed(), preview.dimmed());
        }
        AgentEvent::ToolCallResult { name, output, exit_code, .. } => {
            let code = if exit_code==0 { "0".green() } else { exit_code.to_string().red() };
            eprintln!("  [{} exit {}]", name, code);
            for line in output.lines().take(3) {
                eprintln!("    {}", line.dimmed());
            }
        }
        AgentEvent::Done { tokens_in, tokens_out, steps } => {
            eprintln!("\n{} {} steps, {}/{} tokens",
                "done".green().bold(), steps.to_string().cyan(),
                tokens_in.to_string().yellow(), tokens_out.to_string().yellow());
        }
        AgentEvent::Error(e) => { eprintln!("\n{} {}", "✗".red().bold(), e.red()); }
        AgentEvent::Interrupted(r) => { eprintln!("\n{} {}", "⏸".yellow(), r.yellow()); }
        _ => {}
    }
}

// ── helpers ──

fn build_tools() -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register(ReadTool); r.register(GrepTool); r.register(GlobTool);
    r.register(BashTool); r.register(TodoWriteTool); r.register(VibeTool);
    r
}

fn find_project_root(start: &PathBuf) -> PathBuf {
    let mut p = start.clone();
    loop {
        if p.join(".mooncoding.toml").exists() || p.join(".git").exists() { return p; }
        if let Some(pr) = p.parent() { p = pr.to_path_buf(); } else { return start.clone(); }
    }
}