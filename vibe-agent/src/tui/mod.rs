use anyhow::Result;
use crossterm::{
    execute,
    style::{Color as CColor, Print, ResetColor, SetForegroundColor},
    terminal::Clear,
    terminal::ClearType,
};
use std::io::{stdout, BufRead, Write};
use std::sync::{mpsc, Arc};

use crate::config::Config;
use crate::session::SessionStore;
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;

const ACC:   CColor = CColor::Rgb { r: 92,  g: 156, b: 245 };
const TXT:   CColor = CColor::Rgb { r: 224, g: 224, b: 224 };
const MUTED: CColor = CColor::Rgb { r: 96,  g: 96,  b: 96 };
const OK:    CColor = CColor::Rgb { r: 126, g: 207, b: 126 };
const ERR:   CColor = CColor::Rgb { r: 224, g: 80,  b: 80 };

fn clr(c: CColor) -> SetForegroundColor { SetForegroundColor(c) }

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    let mut out = stdout();

    execute!(out, clr(ACC))?; write!(out, "vibe-agent")?;
    execute!(out, clr(TXT))?; writeln!(out, " · {}", cfg.provider.model)?;
    execute!(out, ResetColor)?; out.flush()?;

    let (tx, rx) = mpsc::channel::<AgentEvent>();
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut agent_thread: Option<std::thread::JoinHandle<()>> = None;

    loop {
        while let Ok(ev) = rx.try_recv() { show(&ev)?; }

        if let Some(h) = agent_thread.take() {
            let _ = h.join();
            while let Ok(ev) = rx.try_recv() { show(&ev)?; }
        }

        execute!(out, clr(ACC))?; write!(out, "vibe-agent> ")?;
        execute!(out, ResetColor)?; out.flush()?;

        let line = match lines.next() {
            Some(Ok(l)) => l.trim().to_string(),
            Some(Err(_)) | None => break,
        };
        if line.is_empty() { continue; }

        match line.as_str() {
            "/exit" | "/quit" | "/q" => break,
            "/model" | "/models" => {
                for (n, name) in [("1","deepseek-chat"),("2","deepseek-v4-flash"),("3","deepseek-reasoner"),
                    ("4","gpt-4o"),("5","gpt-4o-mini"),("6","claude-3.5-sonnet"),("7","llama-3.1-70b")] {
                    execute!(out, clr(ACC))?; write!(out, "  [{}] ", n)?;
                    execute!(out, clr(TXT))?; writeln!(out, "{}", name)?;
                }
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            "/help" | "/h" => {
                execute!(out, clr(TXT))?; writeln!(out, "  /model  /key  /status  /clear  /exit")?;
                execute!(out, clr(TXT))?; writeln!(out, "  type a number to pick model, e.g. 2")?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            s if s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit()) => {
                let model = match s { "1"=>"deepseek-chat","2"=>"deepseek-v4-flash","3"=>"deepseek-reasoner",
                    "4"=>"gpt-4o","5"=>"gpt-4o-mini","6"=>"claude-3.5-sonnet","7"=>"llama-3.1-70b", _=>"" };
                if !model.is_empty() {
                    execute!(out, clr(OK))?; writeln!(out, "  model => {} (restart to apply)", model)?;
                    execute!(out, ResetColor)?; out.flush()?;
                }
                continue;
            }
            s if s.starts_with("/key ") => {
                let key = s.trim_start_matches("/key ").trim();
                if key.len() > 10 { execute!(out, clr(OK))?; writeln!(out, "  key set")?; }
                else { execute!(out, clr(ERR))?; writeln!(out, "  key too short")?; }
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            "/status" => {
                execute!(out, clr(TXT))?; writeln!(out, "  model: {}  key: {}", cfg.provider.model,
                    if cfg.provider.api_key.is_empty() {"(none)"} else {"***"})?;
                execute!(out, ResetColor)?; out.flush()?;
                continue;
            }
            "/clear" => { execute!(out, Clear(ClearType::All))?; out.flush()?; continue; }
            prompt => {
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
        AgentEvent::TextDelta(t) => { execute!(o, clr(TXT))?; write!(o, "{}", t)?; o.flush()?; }
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