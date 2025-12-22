use chrono::Utc;
use clap::Parser;
use common::message::{
    Envelope, EnvelopeType, ResultContent, ResultMessageType, ResultPayload, SayPayload, Sender,
    SenderKind,
};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "user-cli")]
#[command(about = "Interactive CLI for Agent Ops Room users")]
struct Args {
    /// Room ID to join
    #[arg(long, env = "ROOM_ID", default_value = "default")]
    room_id: String,

    /// User ID (your username)
    #[arg(long, env = "USER_ID", default_value = "alice")]
    user_id: String,

    /// MQTT broker host
    #[arg(long, env = "MQTT_HOST", default_value = "localhost")]
    mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "MQTT_PORT", default_value = "1883")]
    mqtt_port: u16,
}

fn now_secs() -> u64 {
    Utc::now().timestamp() as u64
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    info!(
        "🧑 Starting user CLI for room '{}' as user '{}'",
        args.room_id, args.user_id
    );

    // Set up MQTT
    let mut mqttoptions = MqttOptions::new(
        format!("user-cli-{}", args.user_id),
        &args.mqtt_host,
        args.mqtt_port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let public_topic = format!("rooms/{}/public", args.room_id);

    // Subscribe to public channel
    client
        .subscribe(&public_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to public topic");

    info!("📡 Connected to room: {}", args.room_id);
    info!("👤 Posting as user: {}", args.user_id);
    info!("💬 Subscribed to: {}", public_topic);
    info!("");
    info!("Type your messages and press Enter. Type 'exit' or 'quit' to leave.");
    info!("");

    // Clone for background task
    let client_clone = client.clone();
    let room_id_clone = args.room_id.clone();
    let user_id_clone = args.user_id.clone();

    // Spawn MQTT event loop in background
    tokio::spawn(async move {
        handle_mqtt_events(&mut eventloop).await;
    });

    // Interactive input loop
    let mut rl = DefaultEditor::new()?;
    loop {
        let readline = rl.readline("You> ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "exit" || trimmed == "quit" {
                    info!("👋 Goodbye!");
                    break;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);

                // Send message
                if let Err(e) = send_message(
                    &client_clone,
                    &room_id_clone,
                    &user_id_clone,
                    trimmed.to_string(),
                )
                .await
                {
                    error!("Failed to send message: {}", e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                info!("👋 Interrupted, goodbye!");
                break;
            }
            Err(ReadlineError::Eof) => {
                info!("👋 EOF, goodbye!");
                break;
            }
            Err(err) => {
                error!("Error reading line: {:?}", err);
                break;
            }
        }
    }

    Ok(())
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

async fn handle_mqtt_events(eventloop: &mut EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if let Ok(text) = String::from_utf8(p.payload.to_vec()) {
                    match serde_json::from_str::<Envelope>(&text) {
                        Ok(envelope) => {
                            display_message(&envelope);
                        }
                        Err(e) => {
                            warn!("Failed to parse message: {}", e);
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

fn display_message(envelope: &Envelope) {
    let sender_id = &envelope.from.id;

    // Clear the prompt line and move to beginning
    print!("\r\x1b[K");

    // Format: from | EnvelopeType | ResultMessageType? | content
    match envelope.message_type {
        EnvelopeType::Say => {
            if let Ok(say) = serde_json::from_value::<SayPayload>(envelope.payload.clone()) {
                println!("{} | Say | {}", sender_id, say.text);
            }
        }
        EnvelopeType::Result => {
            if let Ok(result) = serde_json::from_value::<ResultPayload>(envelope.payload.clone()) {
                let msg_type = result.message_type.to_string();

                // Extract content as string or JSON
                let content = match result.content {
                    ResultContent::Ack(ack) => ack.text,
                    ResultContent::ClarifyingQuestion(q) => q.question,
                    ResultContent::Progress(p) => p.text,
                    ResultContent::Finding(f) => {
                        if let Some(bullets) = f.bullets {
                            serde_json::to_string(&bullets)
                                .unwrap_or_else(|_| "[bullets]".to_string())
                        } else if let Some(text) = f.text {
                            text
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
                        if let Some(mitigation) = r.mitigation {
                            parts.push(format!("mitigation={}", mitigation));
                        }
                        format!("{{{}}}", parts.join(", "))
                    }
                    ResultContent::Result(res) => res.text,
                    ResultContent::ArtifactLink(link) => {
                        format!("{{label={}, url={}}}", link.label, link.url)
                    }
                };

                println!("{} | Result | {} | {}", sender_id, msg_type, content);
            } else {
                // Fallback to raw JSON
                let content = serde_json::to_string(&envelope.payload)
                    .unwrap_or_else(|_| "[invalid]".to_string());
                println!("{} | Result | {} | {}", sender_id, "Unknown", content);
            }
        }
        _ => {
            // Show other message types as well
            let msg_type = format!("{:?}", envelope.message_type);
            let content = serde_json::to_string(&envelope.payload)
                .unwrap_or_else(|_| "[invalid]".to_string());
            println!("{} | {} | {}", sender_id, msg_type, content);
        }
    }

    // Flush output to ensure message is displayed immediately
    let _ = io::stdout().flush();
}
