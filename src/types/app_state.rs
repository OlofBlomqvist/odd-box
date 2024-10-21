use std::sync::atomic::AtomicBool;
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
    Dynamic
}

#[derive(Debug)]
pub struct AppState {
    pub exit: AtomicBool,
    pub site_status_map: Arc<dashmap::DashMap<String,ProcState>>,
    pub statistics : Arc<ProxyStats>,
}

impl AppState {

    pub fn new() -> AppState {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let result = AppState {
            site_status_map: Arc::new(dashmap::DashMap::new()),
            statistics : Arc::new(ProxyStats { 
                terminated_http_connections_per_hostname: dashmap::DashMap::new(),
                active_connections: dashmap::DashMap::new(),
                tunnelled_tcp_connections_per_hostname: dashmap::DashMap::new()
                
            }),
            exit: AtomicBool::new(false),
            //view_mode: ViewMode::Console,
        };

        
        result
    }


}

