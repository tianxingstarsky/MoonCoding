mod db;
mod llm;
mod prompts;
mod report;
mod runner;
mod session;
mod tools;

use anyhow::Result;
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug)]
struct Cli {
    cmd: String,
    spec: Option<String>,
    #[allow(dead_code)]
    run_id: Option<String>,
}

fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        exit(2);
    }
    let cmd = args[1].clone();
    let spec = args.get(2).cloned();
    let run_id = args.get(3).cloned();
    Cli { cmd, spec, run_id }
}

fn usage() {
    eprintln!("vibe-test - autonomous LLM test driver for the vibe CLI");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  vibe-test list                 list available specs");
    eprintln!("  vibe-test run <spec>           run a single spec");
    eprintln!("  vibe-test run-all              run all specs in order");
    eprintln!("  vibe-test report [run_id]      print report for a run (default: most recent)");
    eprintln!();
    eprintln!("env:");
    eprintln!("  DEEPSEEK_API_KEY   required for run / run-all");
    eprintln!("  VIBE_TEST_MAX_STEPS         default 40");
    eprintln!("  VIBE_TEST_MAX_INPUT_TOKENS  default 100000");
}

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("error: {}", msg.as_ref());
    exit(1);
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn main() {
    let cli = parse_cli();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime init");

    rt.block_on(async move {
        if let Err(e) = run(cli).await {
            eprintln!("error: {:#}", e);
            exit(1);
        }
    });
}

async fn run(cli: Cli) -> Result<()> {
    let root = PathBuf::from(".");
    let db_path = root.join("runs").join("vibe_test.db");
    std::fs::create_dir_all(root.join("runs"))?;
    db::init(&db_path)?;

    match cli.cmd.as_str() {
        "list" => {
            let specs = specs(&root)?;
            if specs.is_empty() {
                println!("(no specs found under fixtures/specs/)");
            }
            for s in specs {
                println!("  {}  -  {}", s.id, s.task.lines().next().unwrap_or(""));
            }
            Ok(())
        }
        "run" => {
            let spec_name = cli.spec.clone().unwrap_or_else(|| { usage(); exit(2); });
            let api_key = std::env::var("DEEPSEEK_API_KEY")
                .unwrap_or_else(|_| die("DEEPSEEK_API_KEY env var missing"));
            let specs_list = specs(&root)?;
            let spec = specs_list.into_iter().find(|s| s.id == spec_name)
                .unwrap_or_else(|| die(format!("spec {} not found", spec_name)));
            let cfg = runner::Config {
                api_key,
                model: env_or("DEEPSEEK_MODEL", "deepseek-v4-flash"),
                base_url: env_or("DEEPSEEK_BASE_URL", "https://api.deepseek.com"),
                max_steps: env_u64("VIBE_TEST_MAX_STEPS", 40),
                max_input_tokens: env_u64("VIBE_TEST_MAX_INPUT_TOKENS", 100_000),
                vibe_exe: locate_vibe_exe(),
            };
            let run_id = runner::run_spec(&root, &spec, &cfg, None).await?;
            report::print_console(&root, &run_id).await?;
            Ok(())
        }
        "run-all" => {
            let api_key = std::env::var("DEEPSEEK_API_KEY")
                .unwrap_or_else(|_| die("DEEPSEEK_API_KEY env var missing"));
            let specs_list = specs(&root)?;
            let cfg = runner::Config {
                api_key,
                model: env_or("DEEPSEEK_MODEL", "deepseek-v4-flash"),
                base_url: env_or("DEEPSEEK_BASE_URL", "https://api.deepseek.com"),
                max_steps: env_u64("VIBE_TEST_MAX_STEPS", 40),
                max_input_tokens: env_u64("VIBE_TEST_MAX_INPUT_TOKENS", 100_000),
                vibe_exe: locate_vibe_exe(),
            };
            // 先跑 01 把 workspace 产出作为 02 / 03 的继承工作区
            let mut prev_workspace: Option<PathBuf> = None;
            for s in &specs_list {
                let run_id = runner::run_spec(&root, s, &cfg, prev_workspace.as_deref()).await?;
                report::print_console(&root, &run_id).await?;
                // 下一个 spec 继承本 run 的 workspace
                let ws = root.join("runs").join(&run_id).join("workspace");
                if ws.is_dir() { prev_workspace = Some(ws); }
            }
            Ok(())
        }
        "report" => {
            let id = cli.spec.clone().unwrap_or_else(|| {
                // 最新 run
                db::latest_session(&db_path).unwrap_or_else(|e| die(format!("no runs / {e}")))
            });
            report::print_console(&root, &id).await?;
            Ok(())
        }
        _ => { usage(); exit(2); }
    }
}

fn locate_vibe_exe() -> PathBuf {
    if let Ok(p) = std::env::var("VIBE_EXE") { return PathBuf::from(p); }
    let sibling = PathBuf::from("..").join("vibe").join("target").join("release");
    #[cfg(windows)] let exe = sibling.join("vibe.exe");
    #[cfg(not(windows))] let exe = sibling.join("vibe");
    // canonicalize to absolute path
    if let Ok(canon) = std::fs::canonicalize(&exe) { return canon; }
    PathBuf::from("vibe")
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Spec {
    pub id: String,
    pub task: String,
    #[serde(default)]
    pub assertions: serde_json::Value,
    #[serde(default)]
    pub inherits_workspace: bool,
}

fn specs(root: &PathBuf) -> Result<Vec<Spec>> {
    let dir = root.join("fixtures").join("specs");
    if !dir.is_dir() { return Ok(Vec::new()); }
    let mut out: Vec<Spec> = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("yaml") { continue; }
        let text = std::fs::read_to_string(&p)?;
        let mut s: Spec = serde_yaml::from_str(&text)?;
        if s.id.is_empty() {
            s.id = p.file_stem().unwrap().to_string_lossy().to_string();
        }
        out.push(s);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}