use std::{default, sync::Arc};

use ratatui::layout::Rect;

use crate::{http_proxy::ProcMessage, tui::scroll_state_wrapper::ScrollStateWrapper};

use super::app_state::ProcState;



#[derive(Debug)]
pub struct TuiState {
    pub site_rects: Vec<(Rect,String)>,
    pub show_apps_window : bool,
    pub currently_hovered_site: Option<String>,    
    pub current_page : Page,
    pub connections_tab_state: ConnectionsTabState,
    pub threads_tab_state: ThreadsTabState,    
    pub log_tab_stage : LogPageState,
}
impl TuiState {
    pub fn new() -> TuiState {
        TuiState {
            current_page: Page::Logs,
            currently_hovered_site: None,
            site_rects: Vec::new(),   
            show_apps_window : true,
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
    pub test : String,
    pub scroll_state : ScrollStateWrapper
}

#[derive(Debug,Default)]
pub struct ConnectionsTabState {
    pub test : String,
    pub scroll_state : ScrollStateWrapper
}


#[derive(Clone,Debug,Eq,PartialEq)]
pub enum Page {
    Logs,
    Statistics,
    Connections,
    Threads
}

