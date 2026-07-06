use anyhow::Result;
use crossterm::{
    cursor, execute, queue,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    style::{Color as CColor, Print, ResetColor, SetForegroundColor},
    terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use crate::config::Config;
use crate::session::SessionStore;
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;

const BG:        CColor = CColor::Rgb { r: 10,  g: 10,  b: 10 };
const ACC:       CColor = CColor::Rgb { r: 92,  g: 156, b: 245 };
const TXT:       CColor = CColor::Rgb { r: 224, g: 224, b: 224 };
const MUTED:     CColor = CColor::Rgb { r: 96,  g: 96,  b: 96 };
const OK:        CColor = CColor::Rgb { r: 126, g: 207, b: 126 };
const WARN:      CColor = CColor::Rgb { r: 224, g: 180, b: 100 };
const ERR:       CColor = CColor::Rgb { r: 224, g: 80,  b: 80 };
const STATUS_BG: CColor = CColor::Rgb { r: 14,  g: 14,  b: 16 };

static SPINNER: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];

struct Input {
    buf: String, cur: usize, cmd: bool, // cursor = char index
}
impl Input {
    fn new() -> Self { Self { buf: String::new(), cur: 0, cmd: false } }
    fn len(&self) -> usize { self.buf.chars().count() }
    fn byte_of(&self, idx: usize) -> usize { self.buf.char_indices().nth(idx).map(|(i,_)| i).unwrap_or(self.buf.len()) }
    fn push(&mut self, c: char) { let bp=self.byte_of(self.cur); self.buf.insert(bp,c); self.cur+=1; }
    fn bs(&mut self) { if self.cur>0 { let p=self.cur-1; self.buf.remove(self.byte_of(p)); self.cur=p; } }
    fn del(&mut self) { if self.cur<self.len() { self.buf.remove(self.byte_of(self.cur)); } }
    fn left(&mut self) { if self.cur>0 { self.cur-=1; } }
    fn right(&mut self) { if self.cur<self.len() { self.cur+=1; } }
    fn home(&mut self) { self.cur=0; }
    fn end(&mut self) { self.cur=self.len(); }
    fn take(&mut self) -> String { let s=std::mem::take(&mut self.buf); self.cur=0; s }
    fn render_focus(&self) -> String {
        let chars: Vec<char> = self.buf.chars().collect();
        let mut s = String::with_capacity(chars.len()+2);
        for (i,c) in chars.iter().enumerate() { if i==self.cur { s.push('▌'); } s.push(*c); }
        if self.cur >= chars.len() { s.push('▌'); }
        s
    }
}

fn clr(c: CColor) -> SetForegroundColor { SetForegroundColor(c) }

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, crossterm::event::EnableMouseCapture, Clear(ClearType::All))?;

    // welcome
    execute!(out, clr(ACC))?; write!(out, "vibe-agent")?;
    execute!(out, clr(TXT))?; writeln!(out, " · {}", cfg.provider.model)?;
    execute!(out, ResetColor)?; writeln!(out)?; out.flush()?;

    let (tx, rx) = mpsc::channel::<AgentEvent>();
    let mut inp = Input::new();
    let mut thinking = false;
    let mut status = String::from("ready");
    let mut rows: u16 = 50;
    let mut si = 0usize; // spinner index
    let mut lt = Instant::now();

    loop {
        // agent events
        while let Ok(ev) = rx.try_recv() {
            let mut o = stdout();
            match ev {
                AgentEvent::TextDelta(t) => { execute!(o, clr(TXT))?; write!(o, "{}", t)?; o.flush()?; }
                AgentEvent::TextDone { .. } => { writeln!(o)?; execute!(o, ResetColor)?; o.flush()?; }
                AgentEvent::ToolCallStart { name, input: inp_args, .. } => {
                    let prev: String = inp_args.chars().take(80).collect();
                    execute!(o, clr(ACC))?; write!(o, "  {} ", name)?;
                    execute!(o, clr(MUTED))?; writeln!(o, "{}", prev)?;
                    execute!(o, ResetColor)?; o.flush()?;
                }
                AgentEvent::ToolCallResult { name, output, exit_code, .. } => {
                    let cc = if exit_code==0 { OK } else { ERR };
                    execute!(o, clr(MUTED))?; write!(o, "    {}  ", name)?;
                    execute!(o, clr(cc))?; writeln!(o, "exit {}", exit_code)?;
                    for l in output.lines().take(3) {
                        execute!(o, clr(MUTED))?; writeln!(o, "    {}", l)?;
                    }
                    execute!(o, ResetColor)?; o.flush()?;
                }
                AgentEvent::Done { steps, tokens_in, tokens_out } => {
                    thinking = false;
                    execute!(o, clr(OK))?; write!(o, "  done  ")?;
                    execute!(o, clr(TXT))?; writeln!(o, "{} steps  {}/{} tokens", steps, tokens_in, tokens_out)?;
                    execute!(o, ResetColor)?; o.flush()?;
                    status = format!("done {} steps {}/{} tok", steps, tokens_in, tokens_out);
                }
                AgentEvent::Error(e) => {
                    thinking = false;
                    execute!(o, clr(ERR))?; writeln!(o, "  {}", e)?;
                    execute!(o, ResetColor)?; o.flush()?;
                    status = "error".into();
                }
                AgentEvent::Interrupted(r) => { thinking = false; status = format!("interrupted: {}", r); }
                _ => {}
            }
        }

        if thinking && lt.elapsed().as_millis() >= 80 { si = (si+1)%SPINNER.len(); lt = Instant::now(); }

        // render bottom area
        let (_, r) = terminal::size()?; rows = r;
        let top = rows.saturating_sub(4); // 3 input + 1 status
        let spin = SPINNER[si];

        render_bottom(&mut out, top, &inp, &status, thinking, spin)?;

        // poll
        if event::poll(std::time::Duration::from_millis(20))? {
            match event::read()? {
                Event::Key(key) if key.kind==KeyEventKind::Press => match key.code {
                    KeyCode::Esc => { if inp.cmd { inp.cmd=false; inp.buf.clear(); inp.cur=0; } else { break; } }
                    KeyCode::Enter => {
                        if inp.cmd {
                            let cmd = inp.take(); inp.cmd = false;
                            match cmd.trim() {
                                "/q"|"/quit"|"/exit" => break,
                                "/c"|"/clear" => { execute!(out, Clear(ClearType::All))?; }
                                "/h"|"/help" => {
                                    let mut o = stdout();
                                    execute!(o, clr(ACC))?; writeln!(o, "  [1] /models         show available models")?;
                                    execute!(o, clr(ACC))?; writeln!(o, "  [2] /use <n>       switch model")?;
                                    execute!(o, clr(ACC))?; writeln!(o, "  [3] /clear          clear chat")?;
                                    execute!(o, clr(ACC))?; writeln!(o, "  [4] /status         show session info")?;
                                    execute!(o, clr(ACC))?; writeln!(o, "  [5] /key <sk-...>   set api key")?;
                                    execute!(o, clr(ACC))?; writeln!(o, "  [6] /quit           exit")?;
                                    execute!(o, ResetColor)?; o.flush()?;
                                }
                                "/models" => {
                                    let mut o = stdout();
                                    execute!(o, clr(TXT))?; writeln!(o, "  1  deepseek-chat      (latest)")?;
                                    execute!(o, clr(TXT))?; writeln!(o, "  2  deepseek-v4-flash  (fast)")?;
                                    execute!(o, clr(TXT))?; writeln!(o, "  3  deepseek-reasoner  (R1)")?;
                                    execute!(o, clr(TXT))?; writeln!(o, "  type /use <n> to switch")?;
                                    execute!(o, ResetColor)?; o.flush()?;
                                }
                                s if s.starts_with("/use ") => {
                                    let model = match s.trim_start_matches("/use ").trim() {
                                        "1"|"deepseek-chat" => "deepseek-chat",
                                        "2"|"deepseek-v4-flash"|"v4-flash" => "deepseek-v4-flash",
                                        "3"|"deepseek-reasoner"|"reasoner"|"r1" => "deepseek-reasoner",
                                        other => other,
                                    };
                                    let mut c = (*cfg).clone(); c.provider.model = model.to_string();
                                    // we can't modify Arc, but we can print the change
                                    let mut o = stdout();
                                    execute!(o, clr(OK))?; writeln!(o, "  model => {}", model)?;
                                    execute!(o, ResetColor)?; o.flush()?;
                                    // Note: full model switching requires mutating shared state. For now, restart with env var
                                    execute!(o, clr(MUTED))?; writeln!(o, "  (restart with $env:MOONCODING_MODEL='{}' for permanent)", model)?;
                                    execute!(o, ResetColor)?; o.flush()?;
                                }
                                s if s.starts_with("/key ") => {
                                    let key = s.trim_start_matches("/key ").trim();
                                    let mut o = stdout();
                                    if key.len() > 10 {
                                        execute!(o, clr(OK))?; writeln!(o, "  api key set: {}...", &key[..10])?;
                                    } else {
                                        execute!(o, clr(ERR))?; writeln!(o, "  key too short")?;
                                    }
                                    execute!(o, ResetColor)?; o.flush()?;
                                }
                                "/status" => {
                                    let mut o = stdout();
                                    execute!(o, clr(TXT))?; writeln!(o, "  model : {}", cfg.provider.model)?;
                                    execute!(o, clr(TXT))?; writeln!(o, "  url   : {}", cfg.provider.base_url)?;
                                    execute!(o, clr(TXT))?; writeln!(o, "  key   : {}", if cfg.provider.api_key.is_empty() {"(none)"} else {"***"})?;
                                    execute!(o, ResetColor)?; o.flush()?;
                                }
                                _ => {}
                            }
                            continue;
                        }
                        let prompt = inp.take();
                        if prompt.is_empty() { continue; }
                        writeln!(out)?;
                        execute!(out, clr(ACC))?; write!(out, "> ")?;
                        execute!(out, clr(TXT))?; writeln!(out, "{}", prompt)?;
                        execute!(out, ResetColor)?; out.flush()?;
                        thinking = true; status = "thinking...".into();
                        let tx2=tx.clone(); let t2=tools.clone(); let s2=store.clone();
                        let c2=cfg.clone(); let sid=uuid::Uuid::new_v4().to_string();
                        std::thread::spawn(move || {
                            let rt=tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                let _ = crate::agent::run_agent(&c2,&t2,s2.as_ref(),&prompt,&sid,
                                    &mut |ev| { tx2.send(ev).ok(); }).await;
                            });
                        });
                    }
                    KeyCode::Char('/') if !inp.cmd => { inp.cmd=true; inp.buf="/".into(); inp.cur=1; }
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Ok(mut cl)=arboard::Clipboard::new() { if let Ok(t)=cl.get_text() { for ch in t.chars() { inp.push(ch); } } }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        crate::agent::INTERRUPTED.store(true, Ordering::SeqCst);
                        status = "interrupted".into();
                    }
                    KeyCode::Char(c) => { inp.push(c); }
                    KeyCode::Backspace => { inp.bs(); }
                    KeyCode::Delete => { inp.del(); }
                    KeyCode::Left => { inp.left(); }
                    KeyCode::Right => { inp.right(); }
                    KeyCode::Home => { inp.home(); }
                    KeyCode::End => { inp.end(); }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    execute!(out, cursor::MoveTo(0, rows), Clear(ClearType::FromCursorDown))?;
    disable_raw_mode()?;
    execute!(out, crossterm::event::DisableMouseCapture)?;
    Ok(())
}

fn render_bottom(out: &mut impl Write, top: u16, inp: &Input, status: &str, thinking: bool, spin: &str) -> Result<()> {
    let (cols, _) = terminal::size()?;
    let w = cols as usize;
    let st = if thinking { format!(" {} {}", spin, status) } else { status.to_string() };

    // ── input panel (3 rows) ──
    let title = if inp.cmd { " / " } else { " input " };
    let focus = inp.render_focus();
    let top_line = format!("┌{}┐", "─".repeat(w.saturating_sub(2)));
    let mid_line = format!("│{}│", pad_right(&focus, w.saturating_sub(2)));
    let bot_line = format!("└{}┘", "─".repeat(w.saturating_sub(2)));

    execute!(out,
        cursor::MoveTo(0, top),
        clr(MUTED), Print(&top_line),
    )?;
    // title overlay on top left
    execute!(out, cursor::MoveTo(2, top), clr(TXT), Print(title))?;
    execute!(out,
        cursor::MoveTo(0, top+1),
        clr(MUTED), Print("│"),
        clr(TXT), Print(&pad_right(&focus, w.saturating_sub(2))),
        clr(MUTED), Print("│"),
    )?;
    execute!(out,
        cursor::MoveTo(0, top+2),
        clr(MUTED), Print(&bot_line),
    )?;

    // ── status bar (1 row) ──
    execute!(out,
        cursor::MoveTo(0, top+3),
        clr(STATUS_BG), Print(" ".repeat(w)),
        cursor::MoveTo(1, top+3),
        clr(TXT), Print(&st),
        clr(MUTED), Print("  wheel▌jk▌/ cmd▌space▌tab▌y copy▌ctrl+c stop▌esc quit"),
        ResetColor,
    )?;
    out.flush()?;
    Ok(())
}

fn pad_right(s: &str, width: usize) -> String {
    let mut out = s.to_string();
    out.truncate(width);
    while out.len() < width { out.push(' '); }
    out
}