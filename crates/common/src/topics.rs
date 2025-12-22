//! Topic naming utilities per AOR spec

/// Public chat (approved messages only)
pub fn public(room_id: &str) -> String {
    format!("rooms/{}/public", room_id)
}

/// Agent-authored messages awaiting approval
pub fn public_candidates(room_id: &str) -> String {
    format!("rooms/{}/public_candidates", room_id)
}

/// System events, mic grants, revocations, rejections
pub fn control(room_id: &str) -> String {
    format!("rooms/{}/control", room_id)
}

/// Facilitator → agent tasks (authoritative)
pub fn agent_inbox(room_id: &str, agent_id: &str) -> String {
    format!("rooms/{}/agents/{}/inbox", room_id, agent_id)
}

/// Private agent scratch, tool output, memory
pub fn agent_work(room_id: &str, agent_id: &str) -> String {
    format!("rooms/{}/agents/{}/work", room_id, agent_id)
}

/// Agent heartbeat topic
pub fn agent_heartbeat(room_id: &str, agent_id: &str) -> String {
    format!("rooms/{}/agents/{}/heartbeat", room_id, agent_id)
}

/// Agent heartbeat wildcard subscription (all agents)
pub fn all_agent_heartbeats(room_id: &str) -> String {
    format!("rooms/{}/agents/+/heartbeat", room_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_formatting() {
        assert_eq!(public("test"), "rooms/test/public");
        assert_eq!(public_candidates("test"), "rooms/test/public_candidates");
        assert_eq!(control("test"), "rooms/test/control");
        assert_eq!(
            agent_inbox("test", "researcher"),
            "rooms/test/agents/researcher/inbox"
        );
        assert_eq!(
            agent_work("test", "researcher"),
            "rooms/test/agents/researcher/work"
        );
    }
}
