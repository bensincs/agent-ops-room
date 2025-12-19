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

---

## Documentation

- **SPEC.md** — messaging protocol, topics, schemas
- **ARCHITECTURE.md** — runtime components and responsibilities

---

## License

Apache 2.0 (recommended)
