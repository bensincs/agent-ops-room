//! Common types and utilities shared across Agent Ops Room components
//!
//! This crate contains:
//! - Message envelope and payload types per AOR spec v0.1
//! - Topic naming conventions
//! - Shared errors
//! - LLM client utilities
//! - Message history/memory for conversation context

pub mod error;
#[cfg(feature = "llm")]
pub mod llm;
#[cfg(feature = "llm")]
pub mod memory;
pub mod message;
pub mod topics;

// Re-export commonly used types
pub use error::AorError;
#[cfg(feature = "llm")]
pub use llm::{
    ChatMessage, ChatRequest, ChatResponse, Choice, FunctionCall, FunctionDefinition, LlmClient,
    ResponseMessage, Tool, ToolCall,
};
#[cfg(feature = "llm")]
pub use memory::MessageHistory;
pub use message::{
    Envelope, EnvelopeType, MicGrantPayload, MicRevokePayload, Payload, RejectPayload,
    ResultMessageType, ResultPayload, SayPayload, Sender, SenderKind, TaskPayload,
};
