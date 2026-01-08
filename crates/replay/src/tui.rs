use chrono::{DateTime, Utc};
use common::{Envelope, SenderKind};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use tokio::sync::mpsc;

pub enum TuiCommand {
    Replay(Vec<Envelope>),
}

struct TuiState {
    messages: Vec<Envelope>,
    selected: usize,
    scroll: usize,
    status: String,
}

impl TuiState {
    fn new(messages: Vec<Envelope>) -> Self {
        Self {
            messages,
            selected: 0,
            scroll: 0,
            status: "Press '?' for help".to_string(),
        }
    }

    fn select_next(&mut self) {
        if !self.messages.is_empty() && self.selected < self.messages.len() - 1 {
            self.selected += 1;
            if self.selected >= self.scroll + 10 {
                self.scroll = self.selected - 9;
            }
        }
    }

    fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll {
                self.scroll = self.selected;
            }
        }
    }

    fn select_first(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    fn select_last(&mut self) {
        if !self.messages.is_empty() {
            self.selected = self.messages.len() - 1;
            if self.messages.len() > 10 {
                self.scroll = self.messages.len() - 10;
            }
        }
    }

    fn get_selected(&self) -> Option<&Envelope> {
        self.messages.get(self.selected)
    }
}

pub async fn run_tui(
    replay_tx: mpsc::UnboundedSender<TuiCommand>,
    messages: Vec<Envelope>,
) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = TuiState::new(messages);

    loop {
        terminal.draw(|f| ui(f, &state))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => state.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => state.select_prev(),
                    KeyCode::Char('g') => state.select_first(),
                    KeyCode::Char('G') => state.select_last(),
                    KeyCode::Char('r') => {
                        if let Some(msg) = state.get_selected() {
                            let msg_id = msg.id.clone();
                            let msg_clone = msg.clone();
                            state.status = format!("Replaying message {}", msg_id);
                            let _ = replay_tx.send(TuiCommand::Replay(vec![msg_clone]));
                        }
                    }
                    KeyCode::Char('R') => {
                        let count = state.messages.len();
                        let messages = state.messages.clone();
                        state.status = format!("Replaying all {} messages", count);
                        let _ = replay_tx.send(TuiCommand::Replay(messages));
                    }
                    KeyCode::Char('?') => {
                        state.status =
                            "j/k:nav | r:replay | R:replay all | g/G:top/bottom | q:quit"
                                .to_string();
                    }
                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(format!("Replay - {} messages loaded", state.messages.len()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Archive Browser"),
        );
    f.render_widget(header, chunks[0]);

    // Message list
    let items: Vec<ListItem> = state
        .messages
        .iter()
        .enumerate()
        .skip(state.scroll)
        .take(chunks[1].height as usize)
        .map(|(i, msg)| {
            let ts = DateTime::<Utc>::from_timestamp(msg.ts as i64, 0)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "??:??:??".to_string());

            let sender_color = match msg.from.kind {
                SenderKind::User => Color::Cyan,
                SenderKind::Agent => Color::Green,
                SenderKind::System => Color::Yellow,
            };

            let line = Line::from(vec![
                Span::styled(format!("{:8} ", ts), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:12} ", msg.from.id),
                    Style::default().fg(sender_color),
                ),
                Span::styled(
                    format!("{:?}", msg.message_type),
                    Style::default().fg(Color::White),
                ),
            ]);

            let style = if i == state.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Messages"));
    f.render_widget(list, chunks[1]);

    // Detail view
    if let Some(msg) = state.get_selected() {
        let detail_text = if let Ok(json) = serde_json::to_string_pretty(&msg.payload) {
            format!(
                "ID: {}\nFrom: {} ({})\nType: {:?}\nTimestamp: {}\n\nPayload:\n{}",
                msg.id,
                msg.from.id,
                format!("{:?}", msg.from.kind),
                msg.message_type,
                msg.ts,
                json
            )
        } else {
            "Failed to format message".to_string()
        };

        let detail = Paragraph::new(detail_text)
            .block(Block::default().borders(Borders::ALL).title("Detail"))
            .wrap(Wrap { trim: true });
        f.render_widget(detail, chunks[2]);
    }

    // Status bar
    let status = Paragraph::new(state.status.as_str())
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status, chunks[3]);
}
