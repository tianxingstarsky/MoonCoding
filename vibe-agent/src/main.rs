mod agent;
mod config;
mod prompt;
mod provider;
mod session;
mod stream;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::exit;

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

#[derive(Parser)]
#[command(name = "vibe-agent", about = "MoonCoding interactive CLI agent")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    /// LLM provider base URL (env: MOONCODING_BASE_URL)
    #[arg(long = "base-url")]
    base_url: Option<String>,

    /// Model name (env: MOONCODING_MODEL)
    #[arg(long = "model")]
    model: Option<String>,

    /// API key (env: MOONCODING_API_KEY)
    #[arg(long = "api-key")]
    api_key: Option<String>,

    /// Working directory
    #[arg(short = 'C', long = "workdir")]
    workdir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start an interactive chat (default)
    Chat {
        /// Prompt text (if empty, reads from stdin)
        prompt: Vec<String>,
        /// Session ID to resume
        #[arg(long)]
        session: Option<String>,
    },
    /// List saved sessions
    List,
    /// Resume a saved session
    Resume {
        session_id: String,
    },
    /// Show project tree (Phase B placeholder)
    Tree,
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all().build().expect("tokio");
    rt.block_on(async {
        if let Err(e) = run().await {
            eprintln!("error: {:#}", e);
            exit(1);
        }
    });
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.workdir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let project = find_project_root(&root);
    std::env::set_current_dir(&project)?;

    let mut cfg = Config::load(&project)?;
    // CLI flags override env/toml
    if let Some(b) = cli.base_url { cfg.provider.base_url = b; }
    if let Some(m) = cli.model { cfg.provider.model = m; }
    if let Some(k) = cli.api_key { cfg.provider.api_key = k; }

    if cfg.provider.api_key.is_empty() {
        eprintln!("WARN: no API key set. Set MOONCODING_API_KEY or DEEPSEEK_API_KEY env var.");
        eprintln!("For local ollama, set MOONCODING_BASE_URL=http://localhost:11434/v1");
    }

    let session_store = SqliteStore::new(&cfg.session_dir.join("sessions.db"))?;
    let tools = build_tools();

    match &cli.cmd {
        Cmd::Chat { prompt, session: _ } => {
            let input = if prompt.is_empty() {
                eprint!("moon> ");
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).unwrap_or_default();
                s.trim().to_string()
            } else { prompt.join(" ") };
            if input.is_empty() { eprintln!("(no input)"); return Ok(()); }

            run_agent_loop(&cfg, &tools, &session_store, &input).await?;
        }
        Cmd::List => {
            let ids = session_store.list().await?;
            if ids.is_empty() { println!("(no saved sessions)"); }
            for id in ids {
                if let Some(s) = session_store.load(&id).await? {
                    println!("{}  step={}  tok={}/{}  {}",
                        &id[..8.min(id.len())], s.step, s.tokens_in, s.tokens_out,
                        s.updated_at.as_str());
                }
            }
        }
        Cmd::Resume { session_id } => {
            if let Some(s) = session_store.load(session_id).await? {
                println!("resumed session {} (step={}, tok={}/{})", &s.id[..8], s.step, s.tokens_in, s.tokens_out);
                eprint!("moon> ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap_or_default();
                let input = input.trim().to_string();
                if input.is_empty() { return Ok(()); }
                run_agent_loop(&cfg, &tools, &session_store, &input).await?;
            } else {
                eprintln!("session not found: {}", session_id);
            }
        }
        Cmd::Tree => {
            println!("(tree command reserved for Phase B)");
        }
    }
    Ok(())
}

fn build_tools() -> ToolRegistry {
    let mut r = ToolRegistry::new();
    r.register(ReadTool);
    r.register(GrepTool);
    r.register(GlobTool);
    r.register(BashTool);
    r.register(TodoWriteTool);
    r.register(VibeTool);
    r
}

fn find_project_root(start: &PathBuf) -> PathBuf {
    let mut p = start.clone();
    loop {
        if p.join(".mooncoding.toml").exists() || p.join(".git").exists() {
            return p;
        }
        if let Some(parent) = p.parent() {
            p = parent.to_path_buf();
        } else {
            return start.clone();
        }
    }
}

async fn run_agent_loop(cfg: &Config, tools: &ToolRegistry, store: &dyn SessionStore, input: &str) -> Result<()> {
    let mut step_label = String::new();
    agent::run_agent(cfg, tools, store, input, &mut |ev| {
        match ev {
            AgentEvent::Thinking => { eprint!("\n{}thinking...", step_label); step_label.clear(); }
            AgentEvent::TextDelta(t) => { eprint!("{}", t); }
            AgentEvent::TextDone { tokens_in, tokens_out, .. } => {
                eprintln!();
                step_label = format!("[{}/{}t] ", tokens_in, tokens_out);
            }
            AgentEvent::ToolCallStart { name, input: inp, .. } => {
                eprintln!("\n  [{}{}] {}", step_label, name, inp);
                step_label.clear();
            }
            AgentEvent::ToolCallResult { name, output, exit_code, .. } => {
                let preview: String = output.lines().take(4).collect::<Vec<_>>().join("\n  ");
                eprintln!("  [{}{} exit {}]\n  {}", step_label, name, exit_code, preview);
                step_label.clear();
            }
            AgentEvent::Done { tokens_in, tokens_out, steps } => {
                eprintln!("\n✓ done. steps={} tokens={}/{}", steps, tokens_in, tokens_out);
            }
            AgentEvent::TreeUpdated { json } => {
                eprintln!("\n[tree updated] {}", json);
            }
            AgentEvent::Error(e) => {
                eprintln!("\n✗ error: {}", e);
            }
            AgentEvent::Interrupted(reason) => {
                eprintln!("\n⏸ interrupted: {}", reason);
            }
        }
    }).await
}