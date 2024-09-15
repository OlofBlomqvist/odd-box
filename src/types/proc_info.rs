use std::sync::{atomic::AtomicBool, Weak};

use serde::{Deserialize, Serialize};

use crate::configuration::v2::FullyResolvedInProcessSiteConfig;


#[derive(Eq,PartialEq,Debug,Clone,Hash, Serialize, Deserialize)]
pub struct ProcId { pub id: String }
impl ProcId {
    pub fn new() -> Self {
        Self { id: uuid::Uuid::new_v4().to_string() }
    }
}

#[derive(Debug)]
pub struct ProcInfo {
    pub liveness_ptr : Weak<AtomicBool>,
    pub config : FullyResolvedInProcessSiteConfig,
    pub pid : Option<String>
}

#[derive(Debug)]
pub struct BgTaskInfo {
    pub liveness_ptr : Weak<bool>,
    pub status: String
}