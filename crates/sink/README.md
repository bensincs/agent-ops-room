# Sink

**Purpose:** Message persistence

The Sink component stores all public room messages to a file for archival and analysis purposes.

## Responsibilities

- Subscribe to `rooms/{roomId}/public` topic
- Write each message to a JSONL file (one JSON object per line)
- Flush writes immediately to ensure data persistence
- Log message metadata (id, sender, type)

## Non-responsibilities

- No message filtering or moderation
- No compression or rotation (handled externally if needed)
- No querying or search capabilities

## Configuration

The sink uses environment variables or CLI arguments:

- `AOR_MQTT_HOST` - MQTT broker host (default: `localhost`)
- `AOR_MQTT_PORT` - MQTT broker port (default: `1883`)
- `AOR_ROOM_ID` - Room ID to subscribe to (default: `default`)
- `AOR_SINK_FILE` - Output file path (default: `messages.jsonl`)
- `AOR_SINK_APPEND` - Append to existing file (default: `true`)

## Usage

```bash
# Basic usage - append to messages.jsonl
cargo run --bin sink

# Custom output file
cargo run --bin sink -- --output-file /path/to/archive.jsonl

# Start fresh (truncate existing file)
cargo run --bin sink -- --append false

# Using environment variables
export AOR_ROOM_ID=production
export AOR_SINK_FILE=/var/log/aor/messages.jsonl
cargo run --bin sink
```

## Output Format

Messages are written in JSONL format (JSON Lines), where each line is a complete JSON envelope:

```jsonl
{"id":"msg_1","message_type":"Say","room_id":"default","from":{"kind":"User","id":"alice"},"ts":1234567890,"payload":{"text":"Hello"}}
{"id":"msg_2","message_type":"Result","room_id":"default","from":{"kind":"Agent","id":"researcher"},"ts":1234567891,"payload":{"task_id":"task_1","message_type":"Result","content":"Found 3 results"}}
```

This format is:
- Easy to parse line-by-line
- Streamable (can process while writing)
- Grep-friendly for quick searches
- Compatible with many log analysis tools

## Integration

The sink is a passive observer and can be:
- Started/stopped independently without affecting the room
- Run multiple times with different filters (future enhancement)
- Used for backup, auditing, or replay scenarios

## Future Enhancements

Potential additions:
- Filter by message type or sender
- Automatic file rotation based on size/time
- Compression (gzip)
- Multiple output formats (CSV, Parquet)
- Replay capability (publish stored messages back to MQTT)
