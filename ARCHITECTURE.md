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

## Data Flow (Typical)

1. User posts message
2. Facilitator assigns task
3. Facilitator grants mic
4. Agent performs work
5. Agent publishes result to staging
6. Gateway validates
7. Result appears publicly
8. UI Bridge streams update

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
- Persistence / replay service
- Policy / moderation agent
- Analytics / metrics exporter
- WebSocket UI bridge

Each extension remains optional.

---

**AOR is infrastructure, not an application.**
