# Replay

Interactive TUI component for browsing and replaying archived messages stored by the sink.

## Features

- **Browse archived messages** - View all messages stored in JSONL format
- **Message details** - Inspect individual message payloads
- **Replay messages** - Republish selected or all messages to MQTT
- **Keyboard navigation** - Vim-style navigation (j/k)

## Usage

```bash
cargo run --bin replay -- --room-id default
```

### Keyboard Controls

- `j` or `↓` - Navigate down
- `k` or `↑` - Navigate up
- `g` - Jump to first message
- `G` - Jump to last message
- `r` - Replay selected message
- `R` - Replay all messages
- `?` - Show help
- `q` - Quit

## Configuration

Environment variables:
- `AOR_MQTT_HOST` - MQTT broker host (default: localhost)
- `AOR_MQTT_PORT` - MQTT broker port (default: 1883)
- `AOR_ROOM_ID` - Room ID to replay messages to
- `AOR_REPLAY_FILE` - Input file path (default: messages.jsonl)

## How It Works

1. Loads messages from JSONL file created by sink
2. Displays them in an interactive TUI
3. When replaying, republishes messages to `rooms/{roomId}/public`
4. Messages go through the normal flow (gateway validation, etc.)

## Use Cases

- **Debugging** - Replay specific conversation sequences
- **Testing** - Simulate user interactions
- **Analysis** - Review conversation history
- **Development** - Test agent behavior with real data
