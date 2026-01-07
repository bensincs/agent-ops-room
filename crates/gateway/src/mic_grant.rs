//! Mic grant tracking and validation

use common::MicGrantPayload;
use std::collections::HashMap;

/// Tracks active mic grants per agent
#[derive(Debug, Default)]
pub struct MicGrantTracker {
    /// Key: (agent_id, task_id)
    grants: HashMap<(String, String), MicGrant>,
}

#[derive(Debug, Clone)]
pub struct MicGrant {
    #[allow(dead_code)]
    pub task_id: String,
    #[allow(dead_code)]
    pub agent_id: String,
    pub max_messages: u32,
    pub messages_sent: u32,
    pub allowed_message_types: Vec<String>,
    pub expires_at: u64,
}

impl MicGrantTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new mic grant
    pub fn grant(&mut self, payload: MicGrantPayload) {
        let grant = MicGrant {
            task_id: payload.task_id.clone(),
            agent_id: payload.agent_id.clone(),
            max_messages: payload.max_messages,
            messages_sent: 0,
            allowed_message_types: payload
                .allowed_message_types
                .iter()
                .map(|t| format!("{:?}", t).to_lowercase())
                .collect(),
            expires_at: payload.expires_at,
        };

        self.grants
            .insert((payload.agent_id, payload.task_id), grant);
    }

    /// Validate a message against an active mic grant
    pub fn validate(
        &mut self,
        agent_id: &str,
        task_id: &str,
        message_type: &str,
        current_ts: u64,
    ) -> Result<(), ValidationError> {
        let key = (agent_id.to_string(), task_id.to_string());

        let grant = self
            .grants
            .get_mut(&key)
            .ok_or(ValidationError::NoMicGrant)?;

        // Check expiration
        if current_ts > grant.expires_at {
            return Err(ValidationError::MicGrantExpired);
        }

        // Check message type allowed
        if !grant
            .allowed_message_types
            .contains(&message_type.to_string())
        {
            return Err(ValidationError::MessageTypeNotAllowed);
        }

        // Check message count
        if grant.messages_sent >= grant.max_messages {
            return Err(ValidationError::MessageLimitExceeded);
        }

        // Increment counter
        grant.messages_sent += 1;

        Ok(())
    }

    /// Revoke a mic grant (cleanup)
    pub fn revoke(&mut self, agent_id: &str, task_id: &str) {
        self.grants
            .remove(&(agent_id.to_string(), task_id.to_string()));
    }
}

#[derive(Debug)]
pub enum ValidationError {
    NoMicGrant,
    MicGrantExpired,
    MessageTypeNotAllowed,
    MessageLimitExceeded,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::NoMicGrant => write!(f, "no_mic_grant"),
            ValidationError::MicGrantExpired => write!(f, "mic_grant_expired"),
            ValidationError::MessageTypeNotAllowed => write!(f, "message_type_not_allowed"),
            ValidationError::MessageLimitExceeded => write!(f, "message_limit_exceeded"),
        }
    }
}
