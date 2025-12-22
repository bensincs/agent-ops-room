//! Message validation logic

use crate::mic_grant::{MicGrantTracker, ValidationError};
use common::{Envelope, EnvelopeType};

/// Validate a candidate message for publication
pub fn validate_message(
    envelope: &Envelope,
    tracker: &mut MicGrantTracker,
    current_ts: u64,
) -> Result<(), ValidationError> {
    // Must be a result message
    if envelope.message_type != EnvelopeType::Result {
        return Err(ValidationError::MessageTypeNotAllowed);
    }

    // Extract result payload
    let result_payload = serde_json::from_value::<common::ResultPayload>(envelope.payload.clone())
        .map_err(|_| ValidationError::MessageTypeNotAllowed)?;

    // Validate against mic grant
    tracker.validate(
        &envelope.from.id,
        &result_payload.task_id,
        &result_payload.message_type.to_string(),
        current_ts,
    )
}
