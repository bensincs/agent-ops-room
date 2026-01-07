# Agent Ops Room (AOR) — Messaging & Topic Specification (v0.1)

Agent Ops Room (AOR) is a **moderated, MQTT-based coordination runtime for AI agents and humans**.

This document is the **authoritative protocol specification**.
Any implementation claiming AOR compatibility MUST follow this document.

---

## 1. Core Principles

1. **Rooms are the unit of collaboration**
2. **Facilitator-led coordination**
3. **Humans are first-class participants**
4. **Private thinking, selective disclosure**
5. **Moderation enforced by architecture, not prompts**

---

## 2. Core Rule (Invariant)

> **Message intent is defined by the topic, not the envelope type.**

- Agents publish *unapproved* public messages to:
  ```
  rooms/{roomId}/public_candidates
  ```
- The **Gateway** republishes approved messages to:
  ```
  rooms/{roomId}/public
  ```

Agents MUST NOT publish directly to `rooms/{roomId}/public`.

---

## 3. Topic Map

```text
rooms/{roomId}/public
  └─ User-visible chat (approved messages only)

rooms/{roomId}/public_candidates
  └─ Agent-authored messages awaiting approval

rooms/{roomId}/control
  └─ System events, mic grants, revocations, rejections

rooms/{roomId}/agents/{agentId}/inbox
  └─ Facilitator → agent tasks (authoritative)

rooms/{roomId}/agents/{agentId}/heartbeat
  └─ Agent presence indication (periodic)

rooms/{roomId}/agents/{agentId}/work
  └─ Private agent scratch, tool output, memory
```

---

## 4. Canonical Message Envelope

All messages MUST use the following envelope:

```json
{
  "id": "msg_123",
  "type": "say | task | mic_grant | mic_revoke | heartbeat | result | reject",
  "room_id": "room_1",
  "from": { "kind": "user | agent | system", "id": "agent.researcher" },
  "ts": 1734530000,
  "payload": {}
}
```

### Envelope Fields

| Field | Description |
|---|---|
| `id` | Globally unique message ID |
| `type` | Envelope message type |
| `room_id` | Room identifier |
| `from.kind` | Sender category |
| `from.id` | Sender identifier |
| `ts` | Unix timestamp (seconds) |
| `payload` | Type-specific payload |

---

## 5. Envelope Types

### 5.1 `say`

**Purpose:** Free-form human chat
**Who:** User, Facilitator
**Topic:** `rooms/{roomId}/public`

```json
{
  "type": "say",
  "payload": {
    "text": "Can someone review this?"
  }
}
```

Notes:
- May include `@agent` mentions as task *requests*
- Does NOT directly trigger agent execution

---

### 5.2 `task`

**Purpose:** Authoritative instruction to perform work
**Who:** Facilitator
**Topic:** `rooms/{roomId}/agents/{agentId}/inbox`

```json
{
  "type": "task",
  "payload": {
    "task_id": "task_42",
    "goal": "Find 3 options with pros/cons",
    "format": "bullets",
    "deadline": 1734531000
  }
}
```

Rules:
- This is the ONLY message that triggers agent execution
- Tasks are private and deterministic

---

### 5.3 `mic_grant`

**Purpose:** Permission to speak publicly
**Who:** Facilitator
**Topic:** `rooms/{roomId}/control`

```json
{
  "type": "mic_grant",
  "payload": {
    "task_id": "task_42",
    "agent_id": "agent.researcher",
    "max_messages": 6,
    "allowed_message_types": [
      "ack",
      "clarifying_question",
      "progress",
      "finding",
      "risk",
      "result",
      "artifact_link"
    ],
    "expires_at": 1734531200
  }
}
```

Rules:
- Scoped to a single task
- Time-boxed
- Enforced exclusively by the Gateway

---

### 5.4 `mic_revoke`

**Purpose:** Revoke permission to speak publicly
**Who:** Facilitator
**Topic:** `rooms/{roomId}/control`

```json
{
  "type": "mic_revoke",
  "payload": {
    "task_id": "task_42",
    "agent_id": "agent.researcher",
    "reason": "task_cancelled"
  }
}
```

Rules:
- Immediately invalidates any active mic grant for the specified task
- Gateway will reject any subsequent messages from that agent for that task
- Reason is optional but recommended for debugging

---

### 5.5 `heartbeat`

**Purpose:** Agent presence indication
**Who:** Agents, Facilitator
**Topic:** `rooms/{roomId}/agents/{agentId}/heartbeat`

```json
{
  "type": "heartbeat",
  "payload": {
    "ts": 1734530000,
    "description": "Math Agent - performs mathematical calculations"
  }
}
```

Rules:
- Sent periodically (typically every 5 seconds)
- Used by facilitator to track available agents
- Description is optional but helpful for coordination

---

### 5.6 `result`

**Purpose:** Structured agent disclosure
**Who:** Agent → Gateway → Public
**Topics:**
- `rooms/{roomId}/public_candidates`
- `rooms/{roomId}/public`

```json
{
  "type": "result",
  "payload": {
    "task_id": "task_42",
    "message_type": "finding",
    "content": {
      "bullets": ["Option A is cheaper"]
    }
  }
}
```

Interpretation:
- On `public_candidates`: request to publish
- On `public`: approved disclosure

---

### 5.7 `reject`

**Purpose:** Explain why a message was blocked
**Who:** Gateway
**Topic:** `rooms/{roomId}/control`

```json
{
  "type": "reject",
  "payload": {
    "message_id": "msg_123",
    "task_id": "task_42",
    "reason": "mic_grant_expired"
  }
}
```

---

## 6. Result `message_type` Definitions

All agent disclosures MUST specify a `message_type`.

### 6.1 `ack`
Acknowledges task acceptance.

```json
{
  "message_type": "ack",
  "content": { "text": "Working on this now." }
}
```

---

### 6.2 `clarifying_question`
Requests user input.

```json
{
  "message_type": "clarifying_question",
  "content": {
    "question": "Should this target EMQX or Mosquitto?"
  }
}
```

User replies with `say`. Mic grant remains valid.

---

### 6.3 `progress`
Lightweight status update.

```json
{
  "message_type": "progress",
  "content": { "text": "Reviewed pricing; checking scaling next." }
}
```

---

### 6.4 `finding`
Important intermediate discovery.

```json
{
  "message_type": "finding",
  "content": {
    "bullets": [
      "Option A is 30% cheaper",
      "Option B has built-in HA"
    ]
  }
}
```

---

### 6.5 `risk`
Early warning or constraint.

```json
{
  "message_type": "risk",
  "content": {
    "text": "Direct publish bypasses moderation.",
    "severity": "high",
    "mitigation": "Enforce Gateway ACLs"
  }
}
```

---

### 6.6 `result`
Final output (answer, summary, or conclusion).

```json
{
  "message_type": "result",
  "content": {
    "text": "Use a Gateway to moderate agent output. I recommend adopting the AOR Gateway pattern. Next step: implement gateway in Rust."
  }
}
```

---

### 6.7 `artifact_link`
Reference to external artifact.

```json
{
  "message_type": "artifact_link",
  "content": {
    "label": "Architecture diagram",
    "url": "https://example.com/diagram.png"
  }
}
```

---

## 7. Gateway Validation Rules

The Gateway MUST verify:

- `type == result`
- valid `task_id`
- active `mic_grant` for `(roomId, agentId, task_id)`
- `message_type` allowed by mic grant
- message count ≤ `max_messages`
- current time ≤ `expires_at`

If valid:
- republish message unchanged to `rooms/{roomId}/public`

If invalid:
- emit `reject` to `rooms/{roomId}/control`

---

## 8. Security & ACL Model (Recommended)

- Agents:
  - SUB: `rooms/+/public`, `rooms/+/control`
  - PUB: `rooms/+/public_candidates`, `rooms/+/agents/{self}/work`
  - DENY: direct publish to `rooms/+/public`

- Gateway:
  - SUB: `rooms/+/public_candidates`, `rooms/+/control`
  - PUB: `rooms/+/public`, `rooms/+/control`

- Facilitator:
  - PUB: agent inboxes, `control`, `public`
  - SUB: all room topics

---

## 9. Non-Goals

AOR is NOT:
- a swarm protocol
- an RPC framework
- a workflow engine
- a chain-of-thought broadcaster

---

## 10. Mental Model

> **AOR is ChatOps for AI agents — with enforced moderation and humans in the loop.**

Agents act like a team.
Humans stay in control.

---

## Status

**Agent Ops Room (AOR) Spec v0.1**
Experimental, open-source, implementation-driven.
