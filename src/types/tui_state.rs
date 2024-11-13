use std::{default, sync::Arc};

use ratatui::layout::Rect;
use tracing::level_filters::LevelFilter;

use crate::{http_proxy::ProcMessage, tui::scroll_state_wrapper::ScrollStateWrapper};

use super::app_state::ProcState;


#[derive(Debug,Eq,PartialEq,Clone)]
pub enum TuiSiteWindowState {
    Hide,
    Small,
    Medium,
    Large
}
impl TuiSiteWindowState {
    pub fn next(self) -> Self {
        match self {
            Self::Hide => Self::Small,
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Hide
        }
    }
}


#[derive(Debug)]
pub struct TuiState {
    pub site_rects: Vec<(Rect,String)>,
    pub app_window_state : TuiSiteWindowState,
    pub currently_hovered_site: Option<String>,    
    pub current_page : Page,
    pub connections_tab_state: ConnectionsTabState,
    pub threads_tab_state: ThreadsTabState,    
    pub log_tab_stage : LogPageState,
    pub log_level : String
}
impl TuiState {
    pub fn new() -> TuiState {
        TuiState {
            log_level: LevelFilter::current().to_string(),
            current_page: Page::Logs,
            currently_hovered_site: None,
            site_rects: Vec::new(),   
            app_window_state : TuiSiteWindowState::Small,
            connections_tab_state: {
                let mut s = ConnectionsTabState::default();
                s.scroll_state.vertical_scroll = Some(0);
                s
            },
            threads_tab_state: {
                let mut s = ThreadsTabState::default();
                s.scroll_state.vertical_scroll = Some(0);
                s
            },
            log_tab_stage: default::Default::default(),
            
        }
    }
}



impl TuiState {
    

    pub fn sites_handle_mouse_click(&mut self, _column: u16, _row: u16,tx: tokio::sync::broadcast::Sender<ProcMessage>, site_states_map: &Arc<dashmap::DashMap<String, ProcState>>) {
        
        let selected_site = if let Some(s) = &self.currently_hovered_site { s } else { return };
        
        let new_state : Option<bool> =  {

            let mut info = if let Some(v) = site_states_map.get_mut(selected_site) {v} else {return};
            let (_,state) = info.pair_mut();
            match state {
                ProcState::Faulty => {
                    *state = ProcState::Stopped;
                    Some(false)
                },
                ProcState::Stopped =>  {
                    *state = ProcState::Starting;
                    Some(true)
                }
                ProcState::Running =>  {
                    *state = ProcState::Stopping;
                    Some(false)
                }
                _ => None
            }
        };

        if let Some(s) = new_state {
            if s {
                tx.send(ProcMessage::Start(selected_site.to_owned())).expect("should always be able to send internal messages");
            } else {
                tx.send(ProcMessage::Stop(selected_site.to_owned())).expect("should always be able to send internal messages");
            }
        }

    }

    pub fn sites_handle_mouse_hover(&mut self, column: u16, row: u16) {
        let mut highlight : Option<String> = None;
        for (rect,site) in self.site_rects.iter() {
            if rect.left() <= column && rect.right() >= column && row == rect.top() {
                highlight = Some(site.to_string());
                break;
            }
        }
        
        self.currently_hovered_site = highlight;
    }

}




#[derive(Debug,Default)]
pub struct LogPageState {
    pub scroll_state : ScrollStateWrapper
}

#[derive(Debug,Default)]
pub struct ThreadsTabState {
    pub _test : String,
    pub scroll_state : ScrollStateWrapper
}

#[derive(Debug,Default)]
pub struct ConnectionsTabState {
    pub _test : String,
    pub scroll_state : ScrollStateWrapper
}


#[derive(Clone,Debug,Eq,PartialEq)]
pub enum Page {
    Logs,
    Statistics,
    Connections,
    Threads
}

