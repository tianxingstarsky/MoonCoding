use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Terminal,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use crate::config::Config;
use crate::session::SessionStore;
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;

mod chat; mod diff; mod input; mod markdown; mod side; mod status; mod syntax;
use chat::ChatPanel;
use input::InputPanel;
use side::SidePanel;
use status::StatusBar;

// ── opencode dark theme colors ──
const BG:        Color = Color::Rgb(10, 10, 10);
const BG_PANEL:  Color = Color::Rgb(20, 20, 20);
const BORDER:    Color = Color::Rgb(50, 50, 50);
const BORDER_ACT:Color = Color::Rgb(96, 96, 96);
const TEXT:       Color = Color::Rgb(224, 224, 224);
const TEXT_MUTED: Color = Color::Rgb(96, 96, 96);
const ACCENT:     Color = Color::Rgb(92, 156, 245);
const SUCCESS:    Color = Color::Rgb(126, 207, 126);
const WARN:       Color = Color::Rgb(224, 180, 100);
const ERROR:      Color = Color::Rgb(224, 80, 80);

static SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Copy, PartialEq)]
enum Focus { Input, Chat, Side }

pub struct App {
    pub cfg: Arc<Config>,
    pub chat: ChatPanel,
    pub side: SidePanel,
    pub input: InputPanel,
    pub status: StatusBar,
    pub focus: Focus,
    pub thinking: bool,
    pub interrupt: Arc<AtomicBool>,
    pub session_ids: Vec<String>,
    pub spinner_idx: usize,
    pub last_spinner_tick: Instant,
}

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // clear screen on start
    execute!(terminal.backend_mut(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

    let app = App::new(cfg.clone());
    let rows_before = terminal.size()?.height;
    let res = app.run_loop(&mut terminal, tools, store).await;

    // cleanup: move cursor down past rendered area, clear remainder
    execute!(
        terminal.backend_mut(),
        cursor::MoveTo(0, rows_before),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown),
    )?;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture)?;
    terminal.show_cursor()?;
    res
}

impl App {
    fn new(cfg: Arc<Config>) -> Self {
        let mut chat = ChatPanel::new();
        let model = cfg.provider.model.clone();
        chat.push(Line::from(vec![
            Span::styled("vibe-agent", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::raw(" · "),
            Span::styled(model, Style::default().fg(TEXT)),
        ]));
        let mut side = SidePanel::new(" session ");
        side.set_entries(vec![
            ("model".into(), cfg.provider.model.clone()),
            ("tokens".into(), "0/0".into()),
            ("steps".into(), "0".into()),
        ]);
        let status = StatusBar::new("ready");
        Self { cfg, chat, side, input: InputPanel::new(), status,
            focus: Focus::Input, thinking: false,
            interrupt: Arc::new(AtomicBool::new(false)), session_ids: Vec::new(),
            spinner_idx: 0, last_spinner_tick: Instant::now() }
    }

    async fn run_loop(mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()>
    {
        let (tx, rx) = mpsc::channel::<AgentEvent>();

        loop {
            // ── spinner tick ──
            if self.thinking && self.last_spinner_tick.elapsed().as_millis() >= 80 {
                self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
                self.last_spinner_tick = Instant::now();
            }
            let spinner_char = SPINNER_FRAMES[self.spinner_idx];

            terminal.draw(|f| {
                let area = f.area();
                // background
                f.render_widget(ratatui::widgets::Clear, area);
                let main = Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)]).split(area);
                let top = Layout::default().direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(72), Constraint::Percentage(28)]).split(main[0]);
                self.chat.render(f, top[0], self.focus == Focus::Chat, &spinner_char, self.thinking);
                self.side.render(f, top[1], self.focus == Focus::Side);
                self.input.render(f, main[1], self.focus == Focus::Input);
                self.status.render(f, main[2]);
            })?;

            while let Ok(ev) = rx.try_recv() { self.handle_event(ev); }

            if event::poll(std::time::Duration::from_millis(80))? {
                let Event::Key(key) = event::read()? else { continue };
                if key.kind != KeyEventKind::Press { continue; }

                match key.code {
                    KeyCode::Tab => { self.focus = match self.focus {
                        Focus::Input => Focus::Chat, Focus::Chat => Focus::Side, Focus::Side => Focus::Input };
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.interrupt.store(true, Ordering::SeqCst);
                        crate::agent::INTERRUPTED.store(true, Ordering::SeqCst);
                        self.status.set("interrupted");
                    }
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.chat = ChatPanel::new();
                        self.status.set("chat cleared");
                    }
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // paste from clipboard into input
                        if let Ok(mut cl) = arboard::Clipboard::new() {
                            if let Ok(text) = cl.get_text() {
                                for ch in text.chars() { self.input.push_char(ch); }
                            }
                        }
                    }
                    KeyCode::Char('/') if !self.input.command_mode => { self.input.toggle_command(); }
                    KeyCode::Esc => { if self.input.command_mode { self.input.toggle_command(); } else { break; } }
                    KeyCode::Enter => {
                        if self.input.command_mode {
                            let cmd = self.input.take(); self.input.toggle_command();
                            match cmd.trim() {
                                "/q"|"/quit"|"/exit" => break,
                                "/?"|"/h"|"/help" => {
                                    self.chat.append_delta("keys: jk scroll · space expand · tab focus · y copy · ctrl+c stop · ctrl+k clear · ctrl+v paste · esc quit\ncommands: /models /use /clear /help /quit");
                                }
                                "/c"|"/clear" => { self.chat = ChatPanel::new(); }
                                "/models" | "/m" => {
                                    self.chat.append_delta("available models:\n  [1] deepseek-chat     (latest)\n  [2] deepseek-v4-flash (fast)\n  [3] deepseek-reasoner (R1)\n\n  type /use <name> or /use <number>");
                                }
                                "/use" => {
                                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                                    if parts.len() > 1 {
                                        let model = match parts[1] {
                                            "1" | "deepseek-chat" => "deepseek-chat",
                                            "2" | "deepseek-v4-flash" => "deepseek-v4-flash",
                                            "3" | "deepseek-reasoner" => "deepseek-reasoner",
                                            _ => parts[1],
                                        };
                                        // update config (only in-memory for current session)
                                        let mut cfg = (*self.cfg).clone();
                                        cfg.provider.model = model.to_string();
                                        self.cfg = Arc::new(cfg);
                                        self.chat.append_delta(&format!("model switched to: {}", model));
                                        self.status.set(&format!("model: {}", model));
                                    } else {
                                        self.chat.append_delta("usage: /use <model-name> or /use <number>");
                                    }
                                }
                                "/s"|"/sessions" => { self.chat.append_delta("saved sessions: (use /switch <id> to resume)"); }
                                "/m"|"/model" => {
                                    self.chat.append_delta(&format!("model: {}\napi key: {}",
                                        self.cfg.provider.model,
                                        if self.cfg.provider.api_key.is_empty() {"(not set)"} else {"***"}));
                                }
                                _ => {}
                            }
                            continue;
                        }
                        let prompt = self.input.take();
                        if prompt.is_empty() { continue; }
                        self.chat.push_user(&prompt); self.status.set("thinking..."); self.thinking = true;
                        let tx2 = tx.clone(); let tools2 = tools.clone(); let store2 = store.clone();
                        let cfg2 = self.cfg.clone(); let sid = uuid::Uuid::new_v4().to_string();
                        let interrupt2 = self.interrupt.clone();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                let _ = crate::agent::run_agent(&cfg2, &tools2, store2.as_ref(), &prompt, &sid,
                                    &mut |ev| { if !interrupt2.load(Ordering::SeqCst) { tx2.send(ev).ok(); } }).await;
                            });
                        });
                    }
                    KeyCode::Char('j')|KeyCode::Down if self.focus == Focus::Chat => { self.chat.scroll_down(); }
                    KeyCode::Char('k')|KeyCode::Up if self.focus == Focus::Chat => { self.chat.scroll_up(); }
                    KeyCode::Char('g') if self.focus == Focus::Chat => { self.chat.scroll_to_bottom(); }
                    KeyCode::Char('y') if self.focus == Focus::Chat => {
                        let text = self.chat.last_message_text();
                        if let Ok(mut cl) = arboard::Clipboard::new() { let _ = cl.set_text(&text); self.status.set("copied"); }
                    }
                    KeyCode::Char(' ') => {
                        let p = self.chat.scroll_pos(); let i = self.chat.line_count().saturating_sub(1 + p);
                        self.chat.toggle_line(i);
                    }
                    KeyCode::Char(c) => { self.input.push_char(c); }
                    KeyCode::Backspace => { self.input.backspace(); }
                    KeyCode::Delete => { self.input.delete(); }
                    KeyCode::Left => { self.input.cursor_left(); }
                    KeyCode::Right => { self.input.cursor_right(); }
                    KeyCode::Home => { self.input.cursor_home(); }
                    KeyCode::End => { self.input.cursor_end(); }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::Thinking => {}
            AgentEvent::TextDelta(t) => { self.chat.append_delta(&t); }
            AgentEvent::TextDone { .. } => {}
            AgentEvent::ToolCallStart { name, input, .. } => {
                self.chat.push_tool_start(&name, &input);
                self.status.set(&format!("{} running", name));
            }
            AgentEvent::ToolCallResult { name, output, exit_code, .. } => {
                self.chat.push_tool_result(&name, exit_code, &output);
            }
            AgentEvent::Done { steps, tokens_in, tokens_out } => {
                self.thinking = false;
                self.side.set_entries(vec![
                    ("model".into(), self.cfg.provider.model.clone()),
                    ("tokens".into(), format!("{}/{}", tokens_in, tokens_out)),
                    ("steps".into(), steps.to_string()),
                ]);
                self.status.set(&format!("done · {} steps · {}/{} tokens", steps, tokens_in, tokens_out));
            }
            AgentEvent::Error(e) => { self.thinking = false; self.chat.push_error(&e); self.status.set("error"); }
            AgentEvent::Interrupted(r) => { self.thinking = false; self.status.set(&format!("{}", r)); }
            _ => {}
        }
    }
}