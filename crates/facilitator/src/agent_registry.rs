//! Tracks available agents via heartbeats

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub last_heartbeat: u64,
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct AgentRegistry {
    agents: HashMap<String, AgentInfo>,
    timeout_secs: u64,
}

impl AgentRegistry {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            agents: HashMap::new(),
            timeout_secs,
        }
    }

    pub fn update_agent(&mut self, agent_id: String, description: Option<String>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let _is_new = !self.agents.contains_key(&agent_id);

        if let Some(info) = self.agents.get_mut(&agent_id) {
            info.last_heartbeat = now;
            if description.is_some() {
                info.description = description;
            }
            debug!("Heartbeat from: {}", agent_id);
        } else {
            self.agents.insert(
                agent_id.clone(),
                AgentInfo {
                    last_heartbeat: now,
                    description,
                },
            );
            info!("Agent registered: {}", agent_id);
        }
    }

    pub fn get_active_agents(&self) -> Vec<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.agents
            .iter()
            .filter(|(_, info)| now.saturating_sub(info.last_heartbeat) <= self.timeout_secs)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn get_agent_info(&self, agent_id: &str) -> Option<&AgentInfo> {
        self.agents.get(agent_id)
    }

    pub fn get_active_agents_with_descriptions(&self) -> Vec<(String, Option<String>)> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.agents
            .iter()
            .filter(|(_, info)| now.saturating_sub(info.last_heartbeat) <= self.timeout_secs)
            .map(|(id, info)| (id.clone(), info.description.clone()))
            .collect()
    }
}
