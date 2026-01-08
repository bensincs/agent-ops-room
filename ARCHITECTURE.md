# Agent Ops Room (AOR) — Architecture

This document describes the **runtime architecture** of Agent Ops Room (AOR).

It explains **components, responsibilities, and data flow**.
Protocol details live in `SPEC.md`.

---

## Architectural Goals

- Clear separation of concerns
- Deterministic moderation
- Minimal shared state
- Easy local and distributed deployment
- No required database

---

## High-Level Components

AOR is composed of **small, single-purpose binaries** that communicate over MQTT.

```
User / UI
   |
   |  HTTP / SSE
   v
UI Bridge
   |
   |  MQTT
   v
+-----------------------+
|   Agent Ops Room      |
|                       |
|  Facilitator  Gateway |
|       |          |    |
|       +----+-----+    |
|            |          |
|        Specialist     |
|          Agents       |
+-----------------------+
```

---

## Component Responsibilities

### UI Bridge
**Purpose:** Interface between humans and MQTT

Responsibilities:
- Streams room events to clients (SSE)
- Accepts user messages via HTTP
- Publishes user chat into MQTT

Non-responsibilities:
- No persistence
- No moderation
- No agent logic

---

### Facilitator
**Purpose:** Coordination and leadership

Responsibilities:
- Observe public room chat
- Interpret user intent
- Assign tasks to agents
- Issue mic grants
- Publish assignment events

Non-responsibilities:
- Does not moderate output
- Does not stream to UIs

---

### Gateway
**Purpose:** Deterministic moderation and enforcement

Responsibilities:
- Validate agent messages
- Enforce mic grants
- Enforce rate limits and schemas
- Republish approved messages

Key property:
- No AI
- Fully deterministic
- Enforceable via ACLs

---

### Specialist Agents
**Purpose:** Perform domain-specific work

Responsibilities:
- Subscribe to room topics
- Maintain local memory
- Execute tasks when assigned
- Publish structured results

Non-responsibilities:
- No direct publishing to public chat
- No coordination authority

---

### Summarizer (Optional)
**Purpose:** Generate and maintain conversation context summaries

Responsibilities:
- Subscribe to `rooms/{roomId}/public` topic
- Track message count and conversation flow
- Generate periodic summaries (e.g., every 50-100 messages)
- **Incremental summarization**: new summary = condense(previous summary + new messages)
- Publish summaries to `rooms/{roomId}/summary`
- Include metadata: `covers_until_ts`, `message_count`, `generated_at`

Context Assembly Strategy:
```
Agent Context = [Latest Summary] + [ALL messages where ts > covers_until_ts]
```

Example:
- Messages 1-100: Summary A covers up to timestamp T1
- Messages 101-200: Summary B = LLM condenses (Summary A + messages 101-200), covers up to T2
- Messages 201-250: Agent receives Summary B + **ALL** messages 201-250 (not a fixed window)
- **Zero message loss**: every message is either in summary or has ts > covers_until_ts

Summary Period Tradeoff:
- **Shorter period** (e.g., every 30 msgs): fresher summaries, more API calls, smaller recent window
- **Longer period** (e.g., every 200 msgs): fewer API calls, larger recent window
- Period should be tuned based on room activity and context window limits

Implementation Approaches:
1. **Facilitator-based** (simple): Facilitator generates summaries as part of coordination
2. **Dedicated component** (scalable): Separate summarizer service per room or pool

Non-responsibilities:
- No moderation or filtering
- No persistence (stateless, regenerates from public topic if needed)
- No task assignment or coordination

Key Benefits:
- **Zero context loss**: every message is either in summary or recent window
- Prevents unbounded context growth in long-running rooms
- Summaries can be re-summarized periodically to stay bounded
- Summary period is tunable based on room activity
- LLMs excel at condensing information while preserving key details

---

## Data Flow (Typical)

### Standard Message Flow
1. User posts message
2. Facilitator assigns task
3. Facilitator grants mic
4. Agent performs work
5. Agent publishes result to staging
6. Gateway validates
7. Result appears publicly
8. UI Bridge streams update

### Summarization Flow (Optional)
1. Public messages accumulate in room
2. Summarizer tracks message count
3. After N messages (e.g., 50-100), summarizer generates **incremental summary**:
   - Retrieves previous summary (if exists)
   - Condenses: previous_summary + new_messages_since_last_summary
   - Updates `covers_until_ts` to latest message timestamp
4. Summary published to `rooms/{roomId}/summary`
5. Agents receive summary update
6. Agents build LLM context: `[Latest Summary] + [ALL messages where ts > covers_until_ts]`
7. **Zero message loss**: every message is either in summary or after covers_until_ts
8. Optionally, summarizer can re-summarize the summary itself to prevent unbounded growth

---

## Security Model

- MQTT ACLs enforce topic permissions
- Gateway is the only publisher to public agent output
- Agents are sandboxed by topic access
- UI Bridge is the only internet-facing component

---

## Scaling Model

- Scale agents horizontally
- Scale UI Bridge separately
- Single gateway per broker (or sharded by room prefix)
- Facilitators can be per-room or pooled

---

## Extensibility

Future components may include:
- **Summarizer** (documented above) — context management for long conversations
- Persistence / replay service
- Policy / moderation agent
- Analytics / metrics exporter
- WebSocket UI bridge

Each extension remains optional.

### Context Management Evolution
As rooms grow, context management becomes critical:
- **Phase 1:** Agents use full public message history (works for ~20-50 messages)
- **Phase 2:** Facilitator generates inline summaries (simple, no new component)
- **Phase 3:** Dedicated Summarizer component (scalable, handles multiple rooms)

The incremental summary approach provides:
- **Zero context loss**: every message is represented (either in summary or ts > covers_until_ts)
- **Bounded context size**: summary + recent unsummarized messages
- **Tunable tradeoff**: summary period balances freshness vs. API costs
- Summaries grow incrementally, can be re-summarized to stay bounded
- Simple implementation: `[Latest Summary] + [ALL messages where ts > covers_until_ts]`
- No fixed window size to configure or manage

---

**AOR is infrastructure, not an application.**
