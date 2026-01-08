# Agent Ops Room (AOR)

**Agent Ops Room (AOR)** is a moderated, MQTT-based coordination runtime for AI agents and humans.

AOR lets you run AI agents **like an ops team, not a swarm** — with clear leadership, observable collaboration, and enforced safety boundaries.

---

## What AOR Is

AOR is a **coordination layer**, not an agent framework.

It focuses on:
- how agents collaborate
- how humans stay in the loop
- how public output is controlled and readable

AOR does **not** dictate:
- which LLM you use
- how agents reason internally
- how tools are implemented

---

## The Problem AOR Solves

Most agent systems struggle with:
- agents talking over each other
- unclear ownership of work
- invisible reasoning and decisions
- unsafe or noisy outputs
- humans losing situational awareness

AOR introduces:
- shared rooms
- facilitator-led tasking
- explicit speaking permissions
- architectural enforcement (not prompt discipline)

---

## Core Concepts (User-Level)

### Rooms
A room is a shared collaboration space where:
- users and agents see the same conversation
- tasks are assigned and tracked
- results are published safely

### Facilitator
The facilitator:
- interprets user intent
- assigns tasks to agents
- grants permission to speak publicly
- keeps the room focused

### Specialist Agents
Specialist agents:
- listen continuously
- think privately
- act only when tasked
- selectively disclose useful information

### Summarizer
The summarizer:
- tracks conversation flow
- generates concise summaries after task completions
- helps maintain context as conversations grow
- prevents information loss in long sessions

### Moderation by Design
Agents cannot speak publicly unless:
- they are assigned a task
- they have an active mic grant
- their message passes gateway validation

---

## How AOR Feels to Use

From a user’s perspective:
1. You ask a question in a room
2. The facilitator assigns the right agent
3. You see who is working on what
4. Agents may ask clarifying questions
5. Findings and results appear clearly
6. You remain in control throughout

No hidden background chaos.

---

## What AOR Is Not

- ❌ Not a swarm framework
- ❌ Not an LLM wrapper
- ❌ Not a workflow engine
- ❌ Not a tool runner

AOR is the **operational layer where agents collaborate**.

---

## Open Source Philosophy

AOR is designed to be:
- composable
- inspectable
- replaceable
- boring infrastructure

Small binaries. Clear contracts. MQTT as the backbone.

---

## Project Status

- Early-stage
- Spec-driven
- Implementation in Rust
- **Gateway**: ✅ Implemented and working
- **Facilitator**: ✅ Implemented
- **Specialist Agent**: ✅ Implemented
- **Summarizer**: ✅ Implemented
- **User CLI**: ✅ Implemented

---

## Quick Start

### Prerequisites
- Docker and Docker Compose
- Rust (for running services locally)

### 1. Start Infrastructure

Start the MQTT broker and viewer:

```bash
docker-compose up -d
```

This starts:
- **Mosquitto** (MQTT broker) on port 1883
- **MQTT Explorer** (web UI) on http://localhost:4001

### 2. Run the Gateway

In a new terminal:

```bash
cargo run --bin gateway -- --room-id default
```

Or with custom settings:

```bash
AOR_MQTT_HOST=localhost \
AOR_ROOM_ID=my-room \
cargo run --bin gateway
```

### 3. Run the Facilitator (Optional)

In a new terminal:

```bash
cargo run --bin facilitator -- \
  --room-id default \
  --openai-api-key "your-api-key" \
  --openai-base-url "https://api.openai.com/v1"
```

### 4. Run the Summarizer (Optional)

In a new terminal:

```bash
cargo run --bin summarizer -- \
  --room-id default \
  --openai-api-key "your-api-key" \
  --openai-base-url "https://api.openai.com/v1" \
  --summary-interval 3
```

The summarizer generates concise summaries after every N task completions (default: 3).

### 5. Run a Specialist Agent (Optional)

In a new terminal:

```bash
cargo run --bin specialist-agent -- \
  --room-id default \
  --agent-id cmd-agent \
  --openai-api-key "your-api-key" \
  --openai-base-url "https://api.openai.com/v1"
```

### 6. Run the User CLI (Optional)

In a new terminal:

```bash
cargo run --bin user-cli -- \
  --room-id default \
  --user-id alice
```

The CLI provides an interactive TUI with:
- Real-time message display
- Summary panel (when summarizer is running)
- Agent status tracking
- Message input

### 7. View MQTT Messages

Open http://localhost:4001 in your browser to see the MQTT Explorer.

Connect to:
- **Host**: `host.docker.internal` (on Mac/Windows) or `172.17.0.1` (on Linux)
- **Port**: `1883`

You'll see all topics:
```
rooms/
  └── default/
      ├── public             # Approved messages
      ├── public_candidates  # Agent messages awaiting approval
      ├── control            # Mic grants, rejections, events
      ├── summary            # Conversation summaries
      └── agents/
          └── {agent-id}/
              ├── inbox      # Private tasks for specific agents
              └── heartbeat  # Agent health status
```

### 8. Test the Gateway

You can publish test messages using MQTT Explorer or `mosquitto_pub`:

```bash
# Publish a mic grant
mosquitto_pub -h localhost -p 1883 \
  -t "rooms/default/control" \
  -m '{
    "id": "grant_1",
    "type": "mic_grant",
    "room_id": "default",
    "from": {"kind": "system", "id": "facilitator"},
    "ts": 1734530000,
    "payload": {
      "task_id": "task_1",
      "agent_id": "researcher",
      "max_messages": 3,
      "allowed_message_types": ["ack", "finding", "result"],
      "expires_at": 9999999999
    }
  }'

# Publish an agent result (should be approved)
mosquitto_pub -h localhost -p 1883 \
  -t "rooms/default/public_candidates" \
  -m '{
    "id": "msg_1",
    "type": "result",
    "room_id": "default",
    "from": {"kind": "agent", "id": "researcher"},
    "ts": 1734530001,
    "payload": {
      "task_id": "task_1",
      "message_type": "finding",
      "content": {"text": "Found interesting data"}
    }
  }'
```

Watch the gateway logs and MQTT Explorer to see:
- The message validated
- Republished to `rooms/default/public`
- Or rejected to `rooms/default/control` if invalid

### 9. Stop Everything

```bash
docker-compose down
```

---

## Configuration

All services support configuration via:

**Environment Variables** (recommended):
```bash
AOR_MQTT_HOST=localhost
AOR_MQTT_PORT=1883
AOR_ROOM_ID=my-room
```

**CLI Arguments**:
```bash
cargo run --bin gateway -- \
  --mqtt-host localhost \
  --mqtt-port 1883 \
  --room-id my-room
```

See `--help` for all options:
```bash
cargo run --bin gateway -- --help
```

---

## Documentation

- **SPEC.md** — messaging protocol, topics, schemas
- **ARCHITECTURE.md** — runtime components and responsibilities

---

## License

Apache 2.0 (recommended)
