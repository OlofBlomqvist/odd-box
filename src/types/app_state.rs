use std::sync::atomic::AtomicBool;
use utoipa::ToSchema;
use std::sync::Arc;

#[derive(Debug,PartialEq,Clone,serde::Serialize,ToSchema)]
pub enum ProcState {
    Faulty,
    Stopped,    
    Starting,
    Stopping,
    Running,
    Remote,
    DirServer,
    Docker
}

#[derive(Debug)]
pub struct AppState {
    pub enable_global_traffic_inspection: AtomicBool,
    pub exit: AtomicBool,
    pub site_status_map: Arc<dashmap::DashMap<String,ProcState>>,
}

impl AppState {

    pub fn new() -> AppState {
        let result = AppState {
            enable_global_traffic_inspection: AtomicBool::new(false),
            site_status_map: Arc::new(dashmap::DashMap::new()),
            exit: AtomicBool::new(false),
            //view_mode: ViewMode::Console,
        };

        
        result
    }


}

