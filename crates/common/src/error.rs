//! Error types for AOR components

/// Common errors across AOR components
#[derive(Debug, Clone)]
pub enum AorError {
    Mqtt(String),
    Validation(String),
    PermissionDenied(String),
    Llm(String),
}

impl std::fmt::Display for AorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AorError::Mqtt(msg) => write!(f, "MQTT error: {}", msg),
            AorError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AorError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            AorError::Llm(msg) => write!(f, "LLM error: {}", msg),
        }
    }
}

impl std::error::Error for AorError {}

