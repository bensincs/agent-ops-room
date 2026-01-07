//! Message envelope and payload types per AOR spec v0.1

use serde::{Deserialize, Serialize};

/// Canonical message envelope - ALL messages use this structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Globally unique message ID
    pub id: String,
    /// Envelope message type
    #[serde(rename = "type")]
    pub message_type: EnvelopeType,
    /// Room identifier
    pub room_id: String,
    /// Sender information
    pub from: Sender,
    /// Unix timestamp (seconds)
    pub ts: u64,
    /// Type-specific payload
    pub payload: Payload,
}

/// Envelope message types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeType {
    Say,
    Task,
    MicGrant,
    MicRevoke,
    Result,
    Reject,
    Heartbeat,
}

/// Sender information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sender {
    /// Sender category
    pub kind: SenderKind,
    /// Sender identifier
    pub id: String,
}

/// Sender categories
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SenderKind {
    User,
    Agent,
    System,
}

// Type aliases for convenience
pub type From = Sender;
pub type FromKind = SenderKind;

/// Type-specific payloads
pub type Payload = serde_json::Value;

/// Free-form human chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SayPayload {
    pub text: String,
}

/// Authoritative instruction to perform work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    pub task_id: String,
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<u64>,
}

/// Permission to speak publicly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicGrantPayload {
    pub task_id: String,
    pub agent_id: String,
    pub max_messages: u32,
    pub allowed_message_types: Vec<ResultMessageType>,
    pub expires_at: u64,
}

/// Structured agent disclosure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultPayload {
    pub task_id: String,
    pub message_type: ResultMessageType,
    pub content: ResultContent,
}

/// Result content types per AOR spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResultContent {
    Ack(AckContent),
    ClarifyingQuestion(ClarifyingQuestionContent),
    Progress(ProgressContent),
    Finding(FindingContent),
    Risk(RiskContent),
    Result(ResultOutcome),
    ArtifactLink(ArtifactLinkContent),
}

/// Ack content: acknowledges task acceptance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckContent {
    pub text: String,
}

/// Clarifying question content: requests user input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarifyingQuestionContent {
    pub question: String,
}

/// Progress content: lightweight status update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressContent {
    pub text: String,
}

/// Finding content: important intermediate discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Risk content: early warning or constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskContent {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mitigation: Option<String>,
}

/// Result content: final output (answer, summary, or conclusion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultOutcome {
    pub text: String,
}

/// Artifact link content: reference to external artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLinkContent {
    pub label: String,
    pub url: String,
}

/// Result message type definitions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultMessageType {
    Ack,
    ClarifyingQuestion,
    Progress,
    Finding,
    Risk,
    Result,
    ArtifactLink,
}

impl std::fmt::Display for ResultMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ResultMessageType::Ack => "ack",
            ResultMessageType::ClarifyingQuestion => "clarifying_question",
            ResultMessageType::Progress => "progress",
            ResultMessageType::Finding => "finding",
            ResultMessageType::Risk => "risk",
            ResultMessageType::Result => "result",
            ResultMessageType::ArtifactLink => "artifact_link",
        };
        write!(f, "{}", s)
    }
}

/// Explain why a message was blocked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectPayload {
    pub message_id: String,
    pub task_id: String,
    pub reason: String,
}

/// Agent heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    /// Timestamp
    pub ts: u64,
    /// Optional agent description (sent every 3rd heartbeat)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Mic revoke payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicRevokePayload {
    pub task_id: String,
    pub agent_id: String,
}
