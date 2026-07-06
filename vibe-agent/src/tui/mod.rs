use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Terminal,
};
use std::sync::{mpsc, Arc};

use crate::config::Config;
use crate::session::{SessionStore, SqliteStore};
use crate::stream::AgentEvent;
use crate::tools::ToolRegistry;

mod chat;
mod input;
mod side;
mod status;
use chat::ChatPanel;
use input::InputPanel;
use side::SidePanel;
use status::StatusBar;

pub async fn run(cfg: Arc<Config>, tools: Arc<ToolRegistry>, store: Arc<dyn SessionStore>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(cfg.clone());
    let res = app.run_loop(&mut terminal, tools, store).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    res
}

pub struct App {
    pub cfg: Arc<Config>,
    pub chat: ChatPanel,
    pub side: SidePanel,
    pub input: InputPanel,
    pub status: StatusBar,
    pub thinking: bool,
}

impl App {
    pub fn new(cfg: Arc<Config>) -> Self {
        let mut chat = ChatPanel::new();
        chat.push(Line::from(vec![
            Span::styled("vibe-agent ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}", cfg.provider.model), Style::default().fg(Color::White)),
            Span::styled("  |  Esc quit", Style::default().fg(Color::DarkGray)),
        ]));
        let mut side = SidePanel::new("info");
        side.set_entries(vec![
            ("model".into(), cfg.provider.model.clone()),
            ("provider".into(), cfg.provider.base_url.clone()),
            ("steps".into(), cfg.agent.max_steps.unwrap_or(40).to_string()),
        ]);
        Self { cfg, chat, side, input: InputPanel::new(), status: StatusBar::new("ready"), thinking: false }
    }

    async fn run_loop(
        mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        tools: Arc<ToolRegistry>,
        store: Arc<dyn SessionStore>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel::<AgentEvent>();

        loop {
terminal.draw(|f| {
                let area = f.area();
                let main_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)])
                    .split(area);
                let top_row = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                    .split(main_chunks[0]);

                self.chat.render(f, top_row[0]);
                self.side.render(f, top_row[1]);
                self.input.render(f, main_chunks[1]);
                self.status.render(f, main_chunks[2]);
            })?;

            while let Ok(ev) = rx.try_recv() { self.handle_agent_event(ev); }

            if event::poll(std::time::Duration::from_millis(100))? {
                let Event::Key(key) = event::read()? else { continue };
                if key.kind != KeyEventKind::Press { continue; }
                match key.code {
                    KeyCode::Char('/') if !self.input.command_mode => {
                        self.input.toggle_command();
                    }
                    KeyCode::Esc => {
                        if self.input.command_mode { self.input.toggle_command(); }
                        else { break; }
                    }
                    KeyCode::Enter => {
                        if self.input.command_mode {
                            let cmd = self.input.take();
                            self.input.toggle_command();
                            // handle slash commands
                            match cmd.trim() {
                                "/q" | "/quit" | "/exit" => break,
                                "/h" | "/help" => {
                                    self.chat.push_user("/help");
                                    self.chat.append_delta("Commands: /q quit, /h help, /s sessions, /c clear chat, /n new session");
                                }
                                "/c" | "/clear" => {
                                    self.chat = ChatPanel::new();
                                    self.chat.push(Line::from(Span::styled("chat cleared", Style::default().fg(Color::DarkGray))));
                                }
                                "/s" | "/sessions" => {
                                    self.chat.push_user("/sessions");
                                    self.chat.append_delta("session: "); // simplified
                                }
                                _ => {
                                    self.chat.push_user(&cmd);
                                }
                            }
                            continue;
                        }
                        let prompt = self.input.take();
                        if prompt.is_empty() { continue; }
                        self.chat.push_user(&prompt);
                        self.status.set("thinking...");
                        self.thinking = true;

                        let tx2 = tx.clone();
                        let tools2 = tools.clone();
                        let store2 = store.clone();
                        let cfg2 = self.cfg.clone();
                        let sid = uuid::Uuid::new_v4().to_string();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                let _ = crate::agent::run_agent(
                                    &cfg2, &tools2, store2.as_ref(), &prompt, &sid,
                                    &mut |ev| { tx2.send(ev).ok(); },
                                ).await;
                            });
                        });
                    }
                    KeyCode::Char('j') | KeyCode::Down => { self.chat.scroll_down(); }
                    KeyCode::Char('k') | KeyCode::Up => { self.chat.scroll_up(); }
                    KeyCode::Char('g') => { self.chat.scroll_to_bottom(); }
                    KeyCode::Char(' ') => {
                        // toggle expand on the tool line at scroll position
                        let scroll_pos = self.chat.scroll_pos();
                        let idx = self.chat.line_count().saturating_sub(1 + scroll_pos);
                        self.chat.toggle_line(idx);
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

    fn handle_agent_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::Thinking => {}
            AgentEvent::TextDelta(t) => { self.chat.append_delta(&t); }
            AgentEvent::TextDone { .. } => {}
            AgentEvent::ToolCallStart { name, input, .. } => {
                self.chat.push_tool_start(&name, &input);
                self.status.set(&format!("running {}", name));
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
                self.status.set(&format!("done  steps={}  tokens={}/{}", steps, tokens_in, tokens_out));
            }
            AgentEvent::Error(e) => { self.thinking = false; self.chat.push_error(&e); self.status.set("error"); }
            AgentEvent::Interrupted(r) => { self.thinking = false; self.status.set(&format!("interrupted: {}", r)); }
            _ => {}
        }
    }
}