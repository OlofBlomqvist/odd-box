use std::sync::atomic::{AtomicBool, AtomicUsize};
use utoipa::ToSchema;
use std::sync::Arc;
use crate::types::proxy_state::*;
use ratatui::widgets::ListState;

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
    pub statistics : Arc<ProxyLiveStats>,
}

impl AppState {

    pub fn new() -> AppState {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let result = AppState {
            enable_global_traffic_inspection: AtomicBool::new(false),
            site_status_map: Arc::new(dashmap::DashMap::new()),
            statistics : Arc::new(ProxyLiveStats { 
                total_accepted_tcp_connections: AtomicUsize::new(0),
                lb_access_count_per_hostname: dashmap::DashMap::new(),
                active_connections: dashmap::DashMap::new()
                
            }),
            exit: AtomicBool::new(false),
            //view_mode: ViewMode::Console,
        };

        
        result
    }


}

