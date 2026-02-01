use std::sync::{Weak, atomic::AtomicBool};

use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Debug, Clone, Hash, Default, Serialize, Deserialize)]
pub struct ProcId {
    pub id: String,
}
impl ProcId {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    pub fn from(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[derive(Debug)]
pub struct ProcInfo {
    pub liveness_ptr: Weak<AtomicBool>,
    pub backend_id: String,
    pub pid: Option<String>,
    pub marked_for_removal: bool,
    pub started_at_time_stamp: std::time::SystemTime,
}
