use chrono::{DateTime, Local};
use clap::Parser;
use common::message::{
    Envelope, EnvelopeType, HeartbeatPayload, ResultContent, ResultPayload, SayPayload, Sender,
    SenderKind, SummaryPayload,
};
use common::topics;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use rumqttc::{AsyncClient, Event as MqttEvent, EventLoop, MqttOptions, Packet, QoS};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::error;

#[derive(Parser, Debug)]
#[command(name = "user-cli")]
#[command(about = "Interactive TUI for Agent Ops Room users")]
struct Args {
    /// Room ID to join (optional, will prompt if not provided)
    #[arg(long, env = "ROOM_ID")]
    room_id: Option<String>,

    /// User ID (your username, optional, will prompt if not provided)
    #[arg(long, env = "USER_ID")]
    user_id: Option<String>,

    /// MQTT broker host
    #[arg(long, env = "MQTT_HOST", default_value = "localhost")]
    mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "MQTT_PORT", default_value = "1883")]
    mqtt_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
enum AgentState {
    Idle,
    Working { task_id: String },
    Complete { task_id: String },
}

#[derive(Debug, Clone)]
struct AgentStatus {
    state: AgentState,
    last_updated: u64,
}

#[derive(Debug, Clone)]
struct Message {
    timestamp: DateTime<Local>,
    sender: String,
    sender_kind: SenderKind,
    content: String,
    msg_type: String,
}

struct App {
    room_id: String,
    user_id: String,
    input: String,
    input_cursor: usize,
    messages: Vec<Message>,
    agents: HashMap<String, AgentStatus>,
    scroll_offset: usize,
    show_findings: bool,
    should_quit: bool,
    current_summary: Option<SummaryPayload>,
}

impl App {
    fn new(room_id: String, user_id: String) -> Self {
        Self {
            room_id,
            user_id,
            input: String::new(),
            input_cursor: 0,
            messages: Vec::new(),
            agents: HashMap::new(),
            scroll_offset: 0,
            show_findings: false,
            should_quit: false,
            current_summary: None,
        }
    }

    fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    fn update_agent(&mut self, agent_id: String, state: AgentState, ts: u64) {
        self.agents.insert(
            agent_id,
            AgentStatus {
                state,
                last_updated: ts,
            },
        );
    }

    fn move_cursor_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input_cursor += 1;
        }
    }

    fn enter_char(&mut self, c: char) {
        self.input.insert(self.input_cursor, c);
        self.input_cursor += 1;
    }

    fn delete_char(&mut self) {
        if self.input_cursor > 0 {
            self.input.remove(self.input_cursor - 1);
            self.input_cursor -= 1;
        }
    }

    fn delete_char_forward(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input.remove(self.input_cursor);
        }
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
    }

    fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX; // Will be clamped in render
    }

    fn remove_stale_agents(&mut self, timeout_secs: u64) {
        let now = now_secs();
        self.agents
            .retain(|_, status| now - status.last_updated < timeout_secs);
    }

    fn toggle_findings(&mut self) {
        self.show_findings = !self.show_findings;
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Setup terminal for welcome screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Show welcome screen and get room ID and username
    let (room_id, user_id) = if args.room_id.is_some() && args.user_id.is_some() {
        (args.room_id.unwrap(), args.user_id.unwrap())
    } else {
        match show_welcome_screen(&mut terminal, args.room_id, args.user_id).await {
            Ok((room, user)) => (room, user),
            Err(e) => {
                // Restore terminal
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                return Err(e);
            }
        }
    };

    // Set up MQTT
    let mut mqttoptions = MqttOptions::new(
        format!("user-cli-{}", user_id),
        &args.mqtt_host,
        args.mqtt_port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let public_topic = format!("rooms/{}/public", room_id);
    let heartbeat_topic = format!("rooms/{}/agents/+/heartbeat", room_id);
    let summary_topic = topics::summary(&room_id);

    // Subscribe to public channel, agent heartbeats, and summaries
    client.subscribe(&public_topic, QoS::AtLeastOnce).await?;
    client.subscribe(&heartbeat_topic, QoS::AtLeastOnce).await?;
    client.subscribe(&summary_topic, QoS::AtLeastOnce).await?;

    // Create app state
    let app = Arc::new(Mutex::new(App::new(room_id.clone(), user_id.clone())));
    let app_clone = Arc::clone(&app);

    // Spawn MQTT event loop in background
    tokio::spawn(async move {
        handle_mqtt_events(&mut eventloop, app_clone).await;
    });

    // Spawn background task to clean up stale agents
    let app_clone2: Arc<Mutex<App>> = Arc::clone(&app);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let mut app_lock = app_clone2.lock().await;
            app_lock.remove_stale_agents(30); // 30 second timeout
        }
    });

    // Main UI loop
    let res = run_app(&mut terminal, app, client, room_id, user_id).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: Arc<Mutex<App>>,
    client: AsyncClient,
    room_id: String,
    user_id: String,
) -> anyhow::Result<()> {
    loop {
        // Draw UI
        {
            let app_lock = app.lock().await;
            terminal.draw(|f| ui(f, &app_lock))?;
        }

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                let mut app_lock = app.lock().await;

                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.should_quit = true;
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.should_quit = true;
                    }
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.toggle_findings();
                    }
                    KeyCode::Enter => {
                        let msg = app_lock.input.clone();
                        if !msg.is_empty() {
                            send_message(&client, &room_id, &user_id, msg).await?;
                            app_lock.clear_input();
                        }
                    }
                    KeyCode::Char(c) => {
                        app_lock.enter_char(c);
                    }
                    KeyCode::Backspace => {
                        app_lock.delete_char();
                    }
                    KeyCode::Delete => {
                        app_lock.delete_char_forward();
                    }
                    KeyCode::Left => {
                        app_lock.move_cursor_left();
                    }
                    KeyCode::Right => {
                        app_lock.move_cursor_right();
                    }
                    KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.scroll_to_top();
                    }
                    KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.scroll_to_bottom();
                    }
                    KeyCode::Home => {
                        app_lock.input_cursor = 0;
                    }
                    KeyCode::End => {
                        app_lock.input_cursor = app_lock.input.len();
                    }
                    KeyCode::Up => {
                        app_lock.scroll_up();
                    }
                    KeyCode::Down => {
                        app_lock.scroll_down();
                    }
                    KeyCode::PageUp => {
                        for _ in 0..10 {
                            app_lock.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..10 {
                            app_lock.scroll_down();
                        }
                    }
                    _ => {}
                }

                if app_lock.should_quit {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let has_summary = app.current_summary.is_some();

    let chunks = if has_summary {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Length(3),  // Agents status bar
                Constraint::Length(10), // Summary panel (increased from 6)
                Constraint::Min(0),     // Messages
                Constraint::Length(3),  // Input
                Constraint::Length(1),  // Footer
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(3), // Agents status bar
                Constraint::Min(0),    // Messages
                Constraint::Length(3), // Input
                Constraint::Length(1), // Footer
            ])
            .split(f.area())
    };

    // Header
    render_header(f, chunks[0], app);

    // Agents status bar
    render_agents_bar(f, chunks[1], app);

    if has_summary {
        // Summary panel
        render_summary_panel(f, chunks[2], app);
        // Messages
        render_messages(f, chunks[3], app);
        // Input
        render_input(f, chunks[4], app);
        // Footer
        render_footer(f, chunks[5], app);
    } else {
        // Messages
        render_messages(f, chunks[2], app);
        // Input
        render_input(f, chunks[3], app);
        // Footer
        render_footer(f, chunks[4], app);
    }
}

fn render_summary_panel(f: &mut Frame, area: Rect, app: &App) {
    if let Some(summary) = &app.current_summary {
        let covers_until = DateTime::from_timestamp(summary.covers_until_ts as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let title = format!(
            "📊 Summary ({} msgs until {})",
            summary.message_count, covers_until
        );

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Render summary text - no truncation, let it wrap
        let paragraph = Paragraph::new(summary.summary_text.as_str())
            .style(Style::default().fg(Color::White))
            .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(paragraph, inner);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let title = format!("Agent Ops Room: {} | User: {}", app.room_id, app.user_id);
    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn render_agents_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut agent_list: Vec<_> = app.agents.iter().collect();
    agent_list.sort_by_key(|(id, _)| *id);

    let spans: Vec<Span> = if agent_list.is_empty() {
        vec![Span::styled(
            " No agents online",
            Style::default().fg(Color::DarkGray),
        )]
    } else {
        let mut result = vec![Span::raw(" Agents: ")];
        for (i, (id, status)) in agent_list.iter().enumerate() {
            if i > 0 {
                result.push(Span::raw(" │ "));
            }

            let (emoji, color) = match &status.state {
                AgentState::Idle => ("⚪", Color::Gray),
                AgentState::Working { .. } => ("🟡", Color::Yellow),
                AgentState::Complete { .. } => ("🟢", Color::Green),
            };

            result.push(Span::styled(
                format!("{} {}", emoji, id),
                Style::default().fg(color),
            ));
        }
        result
    };

    let agents_bar = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title("Agents"));
    f.render_widget(agents_bar, area);
}

/// Render markdown-like content with basic formatting
fn render_markdown_content(content: &str, base_style: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Simple markdown parsing for code blocks and bold
    let mut in_code_block = false;

    for line in content.lines() {
        if line.trim().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            // Render code with cyan color
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                ),
            ]));
        } else {
            // Parse bold (**text**)
            let mut spans = vec![Span::raw("  ")];
            let mut current = String::new();
            let mut is_bold = false;
            let mut chars = line.chars().peekable();

            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'*') {
                    chars.next(); // consume second *
                    if !current.is_empty() {
                        let style = if is_bold {
                            base_style.add_modifier(Modifier::BOLD)
                        } else {
                            base_style
                        };
                        spans.push(Span::styled(current.clone(), style));
                        current.clear();
                    }
                    is_bold = !is_bold;
                } else {
                    current.push(ch);
                }
            }

            if !current.is_empty() {
                let style = if is_bold {
                    base_style.add_modifier(Modifier::BOLD)
                } else {
                    base_style
                };
                spans.push(Span::styled(current, style));
            }

            lines.push(Line::from(spans));
        }
    }

    lines
}

fn render_messages(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        let time_str = msg.timestamp.format("%H:%M:%S").to_string();

        let sender_style = match msg.sender_kind {
            SenderKind::User => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            SenderKind::Agent => {
                if msg.sender == "facilitator" {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                }
            }
            SenderKind::System => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        };

        let msg_type_style = Style::default().fg(Color::DarkGray);

        let header_spans = vec![
            Span::styled(time_str, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(&msg.sender, sender_style),
            Span::raw(" "),
            Span::styled(&msg.msg_type, msg_type_style),
        ];

        lines.push(Line::from(header_spans));

        // Redact finding content if show_findings is false
        let content = if msg.msg_type.to_lowercase().contains("finding") && !app.show_findings {
            "[REDACTED - Press Ctrl+F to view findings]".to_string()
        } else {
            msg.content.clone()
        };

        // Add content with markdown rendering
        let content_style = if msg.msg_type.to_lowercase().contains("finding") && !app.show_findings
        {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC)
        } else {
            Style::default()
        };

        // Parse and render markdown-like content
        let content_lines = render_markdown_content(&content, content_style);
        for line in content_lines {
            lines.push(line);
        }
    }

    // Calculate max scroll (total lines - visible lines)
    let total_lines = lines.len();
    let visible_lines = area.height.saturating_sub(2) as usize; // Subtract borders
    let max_scroll = total_lines.saturating_sub(visible_lines);

    let actual_scroll = app.scroll_offset.min(max_scroll);
    let title = if total_lines > visible_lines {
        format!(
            "Messages (line {}/{}, {} msgs)",
            actual_scroll + 1,
            total_lines,
            app.messages.len()
        )
    } else {
        format!("Messages ({} msgs)", app.messages.len())
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((actual_scroll as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, area);

    // Show cursor
    f.set_cursor_position((area.x + app.input_cursor as u16 + 1, area.y + 1));
}

fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let findings_status = if app.show_findings { "ON" } else { "OFF" };
    let footer_text = format!(
        " Enter: Send | ↑↓: Scroll | Ctrl+Home: Top | Ctrl+End: Bottom | Ctrl+F: Findings {} | Ctrl+C/D: Quit ",
        findings_status
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(footer, area);
}

async fn send_message(
    client: &AsyncClient,
    room_id: &str,
    user_id: &str,
    text: String,
) -> anyhow::Result<()> {
    let envelope = Envelope {
        id: format!("user_msg_{}", now_secs()),
        message_type: EnvelopeType::Say,
        room_id: room_id.to_string(),
        from: Sender {
            kind: SenderKind::User,
            id: user_id.to_string(),
        },
        ts: now_secs(),
        payload: serde_json::to_value(SayPayload { text })?,
    };

    let topic = format!("rooms/{}/public", room_id);
    let payload = serde_json::to_string(&envelope)?;

    client
        .publish(topic, QoS::AtLeastOnce, false, payload)
        .await?;

    Ok(())
}

async fn handle_mqtt_events(eventloop: &mut EventLoop, app: Arc<Mutex<App>>) {
    loop {
        match eventloop.poll().await {
            Ok(MqttEvent::Incoming(Packet::Publish(p))) => {
                if let Ok(text) = String::from_utf8(p.payload.to_vec()) {
                    if let Ok(envelope) = serde_json::from_str::<Envelope>(&text) {
                        if p.topic.ends_with("/heartbeat") {
                            process_heartbeat(&envelope, &app).await;
                        } else if p.topic.ends_with("/summary") {
                            process_message(envelope, &app).await;
                        } else {
                            process_message(envelope, &app).await;
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("MQTT error: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn process_heartbeat(envelope: &Envelope, app: &Arc<Mutex<App>>) {
    if envelope.message_type == EnvelopeType::Heartbeat {
        if serde_json::from_value::<HeartbeatPayload>(envelope.payload.clone()).is_ok() {
            let agent_id = envelope.from.id.clone();
            let mut app_lock = app.lock().await;

            // Update or create agent entry
            app_lock
                .agents
                .entry(agent_id)
                .and_modify(|status| {
                    status.last_updated = envelope.ts;
                })
                .or_insert(AgentStatus {
                    state: AgentState::Idle,
                    last_updated: envelope.ts,
                });
        }
    }
}

async fn process_message(envelope: Envelope, app: &Arc<Mutex<App>>) {
    let sender_id = envelope.from.id.clone();
    let sender_kind = envelope.from.kind.clone();
    let timestamp = DateTime::from_timestamp(envelope.ts as i64, 0)
        .map(|dt| dt.with_timezone(&Local))
        .unwrap_or_else(|| Local::now());
    let is_agent = sender_kind == SenderKind::Agent;

    let (msg_type, content) = match envelope.message_type {
        EnvelopeType::Say => {
            if let Ok(say) = serde_json::from_value::<SayPayload>(envelope.payload.clone()) {
                ("Say".to_string(), say.text)
            } else {
                ("Say".to_string(), "[invalid]".to_string())
            }
        }
        EnvelopeType::Summary => {
            if let Ok(summary) = serde_json::from_value::<SummaryPayload>(envelope.payload.clone())
            {
                // Update the current summary in app state
                let mut app_lock = app.lock().await;
                app_lock.current_summary = Some(summary.clone());
                drop(app_lock);
            }
            // Don't add summary to message stream - it will be shown in dedicated panel
            return;
        }
        EnvelopeType::Result => {
            if let Ok(result) = serde_json::from_value::<ResultPayload>(envelope.payload.clone()) {
                let msg_type_str = result.message_type.to_string();

                // Update agent state for ALL agents (including facilitator)
                if is_agent {
                    match msg_type_str.as_str() {
                        "ack" => {
                            let mut app_lock = app.lock().await;
                            // Ensure agent exists (in case heartbeat hasn't arrived yet)
                            app_lock
                                .agents
                                .entry(sender_id.clone())
                                .or_insert(AgentStatus {
                                    state: AgentState::Idle,
                                    last_updated: envelope.ts,
                                });

                            app_lock.update_agent(
                                sender_id.clone(),
                                AgentState::Working {
                                    task_id: result.task_id.clone(),
                                },
                                envelope.ts,
                            );
                        }
                        "result" => {
                            let mut app_lock = app.lock().await;
                            // Ensure agent exists
                            app_lock
                                .agents
                                .entry(sender_id.clone())
                                .or_insert(AgentStatus {
                                    state: AgentState::Idle,
                                    last_updated: envelope.ts,
                                });

                            app_lock.update_agent(
                                sender_id.clone(),
                                AgentState::Complete {
                                    task_id: result.task_id.clone(),
                                },
                                envelope.ts,
                            );
                            drop(app_lock);

                            // Set back to idle after a moment
                            let sender_clone = sender_id.clone();
                            let app_clone: Arc<Mutex<App>> = Arc::clone(app);
                            tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                let mut app_lock = app_clone.lock().await;
                                if let Some(status) = app_lock.agents.get(&sender_clone) {
                                    if matches!(status.state, AgentState::Complete { .. }) {
                                        app_lock.update_agent(
                                            sender_clone,
                                            AgentState::Idle,
                                            now_secs(),
                                        );
                                    }
                                }
                            });
                        }
                        _ => {}
                    }
                }

                let content = extract_result_content(&result.content);
                (format!("Result:{}", msg_type_str), content)
            } else {
                ("Result".to_string(), "[invalid]".to_string())
            }
        }
        _ => {
            let msg_type = format!("{:?}", envelope.message_type);
            let content = serde_json::to_string(&envelope.payload)
                .unwrap_or_else(|_| "[invalid]".to_string());
            (msg_type, content)
        }
    };

    let mut app_lock = app.lock().await;
    app_lock.add_message(Message {
        timestamp,
        sender: sender_id,
        sender_kind,
        content,
        msg_type,
    });
}

fn extract_result_content(content: &ResultContent) -> String {
    match content {
        ResultContent::Ack(ack) => ack.text.clone(),
        ResultContent::ClarifyingQuestion(q) => q.question.clone(),
        ResultContent::Progress(p) => p.text.clone(),
        ResultContent::Finding(f) => {
            if let Some(bullets) = &f.bullets {
                serde_json::to_string(bullets).unwrap_or_else(|_| "[bullets]".to_string())
            } else if let Some(text) = &f.text {
                text.clone()
            } else {
                "[empty finding]".to_string()
            }
        }
        ResultContent::Risk(r) => {
            let mut parts = vec![format!(
                "severity={}",
                r.severity.as_deref().unwrap_or("unknown")
            )];
            parts.push(format!("text={}", r.text));
            if let Some(mitigation) = &r.mitigation {
                parts.push(format!("mitigation={}", mitigation));
            }
            format!("{{{}}}", parts.join(", "))
        }
        ResultContent::Result(res) => res.text.clone(),
        ResultContent::ArtifactLink(link) => {
            format!("{{label={}, url={}}}", link.label, link.url)
        }
    }
}

async fn show_welcome_screen(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    default_room: Option<String>,
    default_user: Option<String>,
) -> anyhow::Result<(String, String)> {
    let mut room_input = default_room.unwrap_or_default();
    let mut user_input = default_user.unwrap_or_default();
    let mut room_cursor = room_input.len();
    let mut user_cursor = user_input.len();
    let mut active_field = 0; // 0 = room, 1 = user

    loop {
        terminal.draw(|f| {
            let size = f.area();

            // Create centered box
            let vertical_margin = size.height / 4;
            let horizontal_margin = size.width / 4;

            let outer_area = Rect {
                x: horizontal_margin,
                y: vertical_margin,
                width: size.width - (horizontal_margin * 2),
                height: size.height - (vertical_margin * 2),
            };

            // Split into sections
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Length(3), // Room input
                    Constraint::Length(3), // User input
                    Constraint::Length(2), // Instructions
                ])
                .split(outer_area);

            // Title
            let title = Paragraph::new("🚀 Agent Ops Room")
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(title, chunks[0]);

            // Room ID input
            let room_style = if active_field == 0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let room_block = Block::default()
                .borders(Borders::ALL)
                .title("Room ID")
                .border_style(if active_field == 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });
            let room_para = Paragraph::new(room_input.as_str())
                .style(room_style)
                .block(room_block);
            f.render_widget(room_para, chunks[1]);

            // User ID input
            let user_style = if active_field == 1 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let user_block = Block::default()
                .borders(Borders::ALL)
                .title("Username")
                .border_style(if active_field == 1 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });
            let user_para = Paragraph::new(user_input.as_str())
                .style(user_style)
                .block(user_block);
            f.render_widget(user_para, chunks[2]);

            // Instructions
            let instructions =
                Paragraph::new("Tab: Switch fields | Enter: Join room | Ctrl+C: Quit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(instructions, chunks[3]);

            // Show cursor in active field
            if active_field == 0 {
                f.set_cursor_position((chunks[1].x + room_cursor as u16 + 1, chunks[1].y + 1));
            } else {
                f.set_cursor_position((chunks[2].x + user_cursor as u16 + 1, chunks[2].y + 1));
            }

            // Outer border
            let outer_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));
            f.render_widget(outer_block, outer_area);
        })?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(anyhow::anyhow!("Cancelled"));
                    }
                    KeyCode::Tab => {
                        active_field = if active_field == 0 { 1 } else { 0 };
                    }
                    KeyCode::Enter => {
                        if !room_input.is_empty() && !user_input.is_empty() {
                            return Ok((room_input, user_input));
                        }
                    }
                    KeyCode::Char(c) => {
                        if active_field == 0 {
                            room_input.insert(room_cursor, c);
                            room_cursor += 1;
                        } else {
                            user_input.insert(user_cursor, c);
                            user_cursor += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if active_field == 0 && room_cursor > 0 {
                            room_input.remove(room_cursor - 1);
                            room_cursor -= 1;
                        } else if active_field == 1 && user_cursor > 0 {
                            user_input.remove(user_cursor - 1);
                            user_cursor -= 1;
                        }
                    }
                    KeyCode::Left => {
                        if active_field == 0 && room_cursor > 0 {
                            room_cursor -= 1;
                        } else if active_field == 1 && user_cursor > 0 {
                            user_cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if active_field == 0 && room_cursor < room_input.len() {
                            room_cursor += 1;
                        } else if active_field == 1 && user_cursor < user_input.len() {
                            user_cursor += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
