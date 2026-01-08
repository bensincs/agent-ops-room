# Development Guide

## Running the System

The Agent Ops Room requires multiple components running simultaneously. Here's how to run everything manually:

### Prerequisites

- **Rust 1.88.0 or later** (upgraded to support ratatui 0.29)
  - Check version: `rustc --version`
  - Update if needed: `rustup update stable`
- Docker Desktop running (for MQTT broker)
- `mosquitto_pub` and `mosquitto_sub` CLI tools (optional, for testing)
- **OpenAI API key** or **Azure OpenAI credentials**

### Start Components (in order)

Open 6 separate terminal windows:

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
# With OpenAI
cargo run --bin facilitator -- \
  --room-id default \
  --openai-api-key "sk-..." \
  --openai-model "gpt-4o-mini"

# With Azure OpenAI
cargo run --bin facilitator -- \
  --room-id default \
  --openai-api-key "YOUR_AZURE_KEY" \
  --openai-base-url "https://YOUR_RESOURCE.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT" \
  --openai-model "gpt-4"
```
The facilitator coordinates the system, analyzes user messages, and delegates tasks to specialist agents.

#### Terminal 4: Specialist Agent (Math Tutor)
```bash
# With OpenAI
cargo run --bin specialist-agent -- \
  --room-id default \
  --agent-id math-agent \
  --openai-api-key "sk-..." \
  --openai-model "gpt-4o-mini"

# With Azure OpenAI
cargo run --bin specialist-agent -- \
  --room-id default \
  --agent-id math-agent \
  --openai-api-key "YOUR_AZURE_KEY" \
  --openai-base-url "https://YOUR_RESOURCE.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT"
```
A specialist agent that handles math-related tasks.

#### Terminal 5: Summarizer (Optional)
```bash
# With OpenAI
cargo run --bin summarizer -- \
  --room-id default \
  --openai-api-key "sk-..." \
  --openai-model "gpt-4o-mini" \
  --summary-interval 3

# With Azure OpenAI
cargo run --bin summarizer -- \
  --room-id default \
  --openai-api-key "YOUR_AZURE_KEY" \
  --openai-base-url "https://YOUR_RESOURCE.openai.azure.com/openai/deployments/YOUR_DEPLOYMENT" \
  --summary-interval 3
```
The summarizer generates concise conversation summaries after every N task completions (default: 3). Summaries appear in the user CLI's dedicated summary panel.

#### Terminal 6: Sink (Optional)
```bash
cargo run --bin sink -- \
  --room-id default \
  --output-file messages.jsonl
```
The sink stores all public messages to a JSONL file (one JSON per line) for archival, analysis, and auditing purposes. Runs passively without affecting the room.

#### Terminal 7: User CLI
```bash
cargo run --bin user-cli
```
Interactive TUI for sending messages and seeing responses. Will prompt for:
- Room ID (e.g., "default")
- Username (e.g., "alice")

Alternatively, provide via command-line:
```bash
cargo run --bin user-cli -- --room-id default --user-id alice
```

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
3. **Heartbeats**: All agents (including facilitator) send periodic heartbeats → `rooms/{room_id}/agents/{agent_id}/heartbeat`
4. **If task needed**:
   - Facilitator sends task → `rooms/{room_id}/agents/{agent_id}/inbox`
   - Facilitator sends mic grant → `rooms/{room_id}/control`
   - Facilitator sends ack → `rooms/{room_id}/public` (to show it's processing)
   - Specialist agent processes task
   - Agent sends ack → `rooms/{room_id}/public_candidates` → Gateway → `rooms/{room_id}/public`
   - Agent sends result → `rooms/{room_id}/public_candidates` → Gateway → `rooms/{room_id}/public`
   - Facilitator acknowledges completion → `rooms/{room_id}/public`
5. **If direct reply**:
   - Facilitator responds directly → `rooms/{room_id}/public`
6. **Summarization** (if summarizer running):
   - Summarizer tracks all messages on `rooms/{room_id}/public`
   - After N task completions (Result messages), generates summary
   - Publishes summary → `rooms/{room_id}/summary`
   - User CLI displays summary in dedicated panel

## User Interface

The user CLI is a full-screen TUI (Terminal User Interface) built with **ratatui**:

### Features:
- **Welcome Screen**: Prompts for Room ID and Username with Tab to switch fields
- **Agent Status Bar**: Shows online agents with color-coded states:
  - ⚪ Gray = Idle
  - 🟡 Yellow = Working (processing a task)
  - 🟢 Green = Complete (just finished)
- **Summary Panel**: Displays latest conversation summary (when summarizer is running)
  - Shows message count and timestamp coverage
  - Yellow border for visibility
  - Updates automatically as new summaries arrive
- **Message View**: Scrollable chat with word wrapping
  - Users: Green
  - Facilitator: Cyan
  - Agents: Magenta
  - System: Red
- **Input Field**: Type messages and press Enter to send
- **Keyboard Controls**:
  - Enter: Send message
  - ↑↓: Scroll through message history
  - Ctrl+C or Ctrl+D: Quit

### Agent State Transitions:
- Heartbeat received → Idle (⚪)
- Ack message → Working (🟡)
- Result message → Complete (🟢)
- 5 seconds after Complete → Idle (⚪)
- No heartbeat for 30 seconds → Agent removed from UI

## Environment Variables

All LLM-enabled components (facilitator, specialist-agent) support:
- `OPENAI_API_KEY` - **Required** for LLM functionality
- `OPENAI_BASE_URL` - Optional, defaults to OpenAI's API
  - For Azure: `https://<resource>.openai.azure.com/openai/deployments/<deployment>`
- `OPENAI_MODEL` - Optional, defaults to `gpt-4o-mini`

Set in your shell or create a `.env` file:
```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
export OPENAI_BASE_URL="https://api.openai.com/v1"
export OPENAI_MODEL="gpt-4o-mini"

# Azure OpenAI
export OPENAI_API_KEY="your-azure-key"
export OPENAI_BASE_URL="https://your-resource.openai.azure.com/openai/deployments/your-deployment"
export OPENAI_MODEL="gpt-4"
```

All components support:
- `MQTT_HOST` - Default: `localhost`
- `MQTT_PORT` - Default: `1883`
- `ROOM_ID` - Default: varies by component

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
cargo build --bin summarizer
cargo build --bin sink
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

# Terminal 4 (optional)
cargo run --bin summarizer -- --room-id test-room --summary-interval 3

# Terminal 5
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
- Verify MQTT broker is on port 1883: `docker ps`

**"No LLM API key" warnings:**
- Set `OPENAI_API_KEY` environment variable or pass via `--openai-api-key`
- Check the key is valid and has credits
- For Azure: Ensure base URL includes the full deployment path

**Compile errors after git pull:**
- Rust version may be too old: `rustup update stable`
- Clean build artifacts: `cargo clean && cargo build`

**Agent not receiving tasks:**
- Verify agent is sending heartbeats (check facilitator logs for "Active agents:")
- Ensure room_id matches across all components
- Check facilitator logs for agent discovery
- Facilitator should NOT list itself as an available agent

**TUI not displaying correctly:**
- Terminal must support Unicode and colors
- Try resizing the terminal window
- Ensure terminal is at least 80x24 characters

**Agent states not changing colors:**
- Check that agents are sending Result messages with proper message_type
- Facilitator and agents should send acks before processing
**Complete state lasts 5 seconds before returning to Idle

**Summaries not appearing:**
- Ensure summarizer is running with correct room_id
- Check summarizer has valid OpenAI API key
- Summaries trigger after N task completions (Result messages)
- Default interval is 3 completed tasks
- Check summarizer logs for "Reached N completed tasks, generating summary..."

**Summary panel not visible in UI:**
- Summary panel only appears after first summary is generated
- Check user-cli is subscribed to summary topic (no error logs)
- Verify summary messages are being published (check MQTT with mosquitto_sub)

## Architecture

See `ARCHITECTURE.md` for system design details.

See `SPEC.md` for the AOR protocol specification.
