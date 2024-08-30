use std::collections::HashMap;
use std::default;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use ratatui::prelude::Rect;
use utoipa::ToSchema;
use crate::types::tui_state::LogPageState;
use crate::types::tui_state::Page;
use crate::types::tui_state::ThreadsTabState;
use crate::types::tui_state::ConnectionsTabState;
use std::sync::Arc;
use crate::types::proxy_state::*;
use ratatui::widgets::ListState;
use crate::ProcMessage;

#[derive(Debug,PartialEq,Clone,serde::Serialize,ToSchema)]
pub enum ProcState {
    Faulty,
    Stopped,    
    Starting,
    Stopping,
    Running,
    Remote
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
                active_connections: dashmap::DashMap::new(),
                hosted_process_stats: dashmap::DashMap::new(),
                remote_targets_stats: dashmap::DashMap::new(),
                total_request_count: AtomicUsize::new(0),
                
            }),
            exit: AtomicBool::new(false),
            //view_mode: ViewMode::Console,
        };

        
        result
    }


}

