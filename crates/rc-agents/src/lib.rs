use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: Uuid,
    pub title: String,
    pub owner: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamPlan {
    pub team_id: Uuid,
    pub lead: String,
    pub tasks: Vec<AgentTask>,
}

impl TeamPlan {
    pub fn new(lead: impl Into<String>) -> Self {
        Self {
            team_id: Uuid::new_v4(),
            lead: lead.into(),
            tasks: Vec::new(),
        }
    }
}
