use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncAction {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub change_id: Uuid,
    pub device_id: String,
    pub entity_type: String, // "customer", "invoice", "delivery_order", "journal_entry", "account"
    pub entity_id: String,
    pub action: SyncAction,
    pub payload_json: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBundle {
    pub bundle_id: Uuid,
    pub source_device_id: String,
    pub created_at_ms: i64,
    pub changes: Vec<ChangeRecord>,
}

impl SyncBundle {
    pub fn new(source_device_id: String, changes: Vec<ChangeRecord>) -> Self {
        Self {
            bundle_id: Uuid::new_v4(),
            source_device_id,
            created_at_ms: Utc::now().timestamp_millis(),
            changes,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
