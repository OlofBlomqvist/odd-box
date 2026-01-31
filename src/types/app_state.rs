use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Clone, serde::Serialize, ToSchema)]
pub enum ProcState {
    Faulty,
    Stopped,
    Starting,
    Stopping,
    Running,
    Remote,
    DirServer,
    Docker,
}

#[derive(Debug)]
pub struct AppState {
    pub enable_global_traffic_inspection: AtomicBool,
    pub exit: AtomicBool,
    pub site_status_map: Arc<dashmap::DashMap<String, ProcState>>,
    pub cruma_assignment: Arc<tokio::sync::RwLock<Option<CrumaAssignedDomain>>>,
}

#[derive(Debug, Clone)]
pub struct CrumaAssignedDomain {
    pub assigned_domain: String,
    pub welcome_message: String,
}

impl AppState {
    pub fn new() -> AppState {
        let result = AppState {
            enable_global_traffic_inspection: AtomicBool::new(false),
            site_status_map: Arc::new(dashmap::DashMap::new()),
            exit: AtomicBool::new(false),
            cruma_assignment: Arc::new(tokio::sync::RwLock::new(None)),
            //view_mode: ViewMode::Console,
        };

        result
    }
}
