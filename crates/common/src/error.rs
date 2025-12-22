//! Error types for AOR components

/// Common errors across AOR components
#[derive(Debug, Clone)]
pub enum AorError {
    Mqtt(String),
    Validation(String),
    PermissionDenied(String),
}
