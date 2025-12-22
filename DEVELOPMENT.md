# Development Guide

## Running the System

The Agent Ops Room requires multiple components running simultaneously. Here's how to run everything manually:

### Prerequisites

- Rust toolchain installed
- Docker Desktop running (for MQTT broker)
- `mosquitto_pub` and `mosquitto_sub` CLI tools (optional, for testing)

### Start Components (in order)

Open 5 separate terminal windows:

#### Terminal 1: MQTT Broker
```bash
docker compose up
```
This starts the MQTT broker on `localhost:1883`

#### Terminal 2: Gateway
```bash
cargo run --bin gateway -- --room-id default
```
The gateway moderates messages from specialist agents before they reach the public channel.

#### Terminal 3: Facilitator
```bash
cargo run --bin facilitator -- --room-id default
```
The facilitator coordinates the system, analyzes user messages, and delegates tasks to specialist agents.

#### Terminal 4: Specialist Agent (Math Tutor)
```bash
cargo run --bin specialist-agent -- --room-id default --agent-id math-agent
```
A specialist agent that handles math-related tasks.

#### Terminal 5: User CLI
```bash
cargo run --bin user-cli -- --room-id default --user-id alice
```
Interactive CLI for sending messages and seeing responses.

### Quick Test Scripts

Alternatively, use the provided scripts:

**Start everything:**
```bash
./run-system.sh
```
(macOS only - opens components in separate Terminal windows)

**Send a message manually:**
```bash
./send-message.sh "What is 25 + 17?"
```

**Monitor all room messages:**
```bash
./monitor-room.sh default
```

## Message Flow

1. **User** sends message → `rooms/{room_id}/public`
2. **Facilitator** receives message, analyzes it
3. **If task needed**:
   - Facilitator sends task → `rooms/{room_id}/agent_inbox/{agent_id}`
   - Specialist agent processes task
   - Agent sends result → `rooms/{room_id}/public_candidates`
   - Gateway moderates result → `rooms/{room_id}/public`
4. **If direct reply**:
   - Facilitator responds directly → `rooms/{room_id}/public`

## Environment Variables

All components support:
- `OPENAI_API_KEY` - Required for LLM functionality
- `OPENAI_BASE_URL` - Optional, defaults to OpenAI's API
- `OPENAI_MODEL` - Optional, defaults to `gpt-4o-mini`

Set in your shell or create a `.env` file:
```bash
export OPENAI_API_KEY="sk-..."
export OPENAI_BASE_URL="https://api.openai.com/v1"
export OPENAI_MODEL="gpt-4o-mini"
```

## Building

Build all components:
```bash
cargo build
```

Build specific component:
```bash
cargo build --bin gateway
cargo build --bin facilitator
cargo build --bin specialist-agent
cargo build --bin user-cli
```

## Testing

Run tests:
```bash
cargo test
```

Test with different room IDs:
```bash
# Terminal 1
cargo run --bin gateway -- --room-id test-room

# Terminal 2
cargo run --bin facilitator -- --room-id test-room

# Terminal 3
cargo run --bin specialist-agent -- --room-id test-room --agent-id math-agent

# Terminal 4
cargo run --bin user-cli -- --room-id test-room --user-id bob
```

## Debugging

### View MQTT messages
```bash
mosquitto_sub -h localhost -p 1883 -t "rooms/default/#" -v
```

### Check component logs
Each component outputs logs to stdout. Look for:
- `[ERROR]` - Problems that need attention
- `[WARN]` - Potential issues
- `[INFO]` - Normal operation events
- `[DEBUG]` - Detailed debugging info

### Common Issues

**"Connection refused" on MQTT:**
- Make sure Docker is running
- Check `docker compose up` is running successfully
- Verify MQTT broker is on port 1883

**"No LLM API key" warnings:**
- Set `OPENAI_API_KEY` environment variable
- Check the key is valid and has credits

**Agent not receiving tasks:**
- Verify agent is sending heartbeats (check logs)
- Ensure room_id matches across all components
- Check facilitator logs for agent discovery

## Architecture

See `ARCHITECTURE.md` for system design details.

See `SPEC.md` for the AOR protocol specification.
