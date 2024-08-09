use std::collections::HashMap;
use ratatui::widgets::ScrollbarState;
use ratatui::prelude::Rect;
use utoipa::ToSchema;
use crate::tui::Page;
use crate::tui::TrafficTabState;
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
pub (crate) struct AppState {
    pub (crate) logs_scroll_bar_hovered : bool,
    pub (crate) last_mouse_down_y_pos : usize,
    pub (crate) dbg : String,
    pub (crate) total_line_count: usize,
    pub (crate) exit: bool,
    pub (crate) site_states_map: HashMap<String,ProcState>,
    pub (crate) vertical_scroll: Option<usize>,
    pub (crate) scroll_state : ScrollbarState,
    pub (crate) show_apps_window : bool,
    pub (crate) logs_area_height:usize,
    pub (crate) logs_area_width:usize,
    pub (crate) site_rects: Vec<(Rect,String)>,
    pub (crate) currently_hovered_site: Option<String>,
    pub (crate) current_page : Page,
    pub (crate) traffic_tab_state: TrafficTabState,
    pub (crate) statistics : Arc<std::sync::RwLock<ProxyStats>>
}

impl AppState {

    pub fn new() -> AppState {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let mut result = AppState {
            logs_scroll_bar_hovered:false,
            last_mouse_down_y_pos: 1,
            dbg: String::new(),
            statistics : Arc::new(std::sync::RwLock::new(ProxyStats { 
                received_tcp_connections: 0, 
                active_connections: HashMap::new()
            })),
            traffic_tab_state: TrafficTabState {
                ..Default::default()
            },
            current_page: Page::Logs,
            currently_hovered_site: None,
            site_rects: vec![],
            total_line_count:0,
            logs_area_height: 5,
            logs_area_width: 5,
            scroll_state: ScrollbarState::new(0),
            vertical_scroll: None,
            exit: false,
            //view_mode: ViewMode::Console,
            site_states_map: HashMap::<String,ProcState>::new(),
            show_apps_window : true 
        };

        
        result
    }



    pub fn sites_handle_mouse_click(&mut self, _column: u16, _row: u16,tx: tokio::sync::broadcast::Sender<ProcMessage>) {
        
        let selected_site = if let Some(x) = &self.currently_hovered_site {x} else { return };
        
        let new_state : Option<bool> =  {

            let (_,state) = if let Some(info) = self.site_states_map.iter_mut().find(|x|x.0==selected_site) {info} else {return};

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
        for (rect,site) in &self.site_rects {
            if rect.left() <= column && rect.right() >= column && row == rect.top() {
                highlight = Some(site.to_string());
                break;
            }
        }
        
        self.currently_hovered_site = highlight;
    }


    pub fn calculate_thumb_size(&self) -> f32 {
        if self.total_line_count <= self.logs_area_height {
            // this is just if we dont need a scrollbar - in which case its just going to be hidden anyway
            self.logs_area_height as f32
        } else {
            let thumb_size = (self.logs_area_height as f64 / self.total_line_count as f64) * self.logs_area_height as f64;
            thumb_size.ceil() as f32
        }
    }

    pub fn logs_handle_mouse_move(&mut self, column: u16, row: u16) {
        let thumb_size = self.calculate_thumb_size().max(1.0);
        let max_scroll = self.total_line_count.saturating_sub(self.logs_area_height);
        let vscroll = self.vertical_scroll.unwrap_or(max_scroll);
        let thumb_position = if self.total_line_count > self.logs_area_height {
            (vscroll as f32 / (self.total_line_count as f32 - self.logs_area_height as f32)) * (self.logs_area_height as f32 - thumb_size)
        } else {
            1.0
        }.max(1.0);
        let horizontal_match = column as usize >= self.logs_area_width - 1 && column as usize <= self.logs_area_width + 1;
        let vertical_match = (row as isize >= thumb_position as isize - 2) && row as usize <= (thumb_position + thumb_size + 1.0) as usize;
        //self.dbg = format!("dragging pos: {row}/{column} - vscroll: {} - tpos: {thumb_position}  | V: {vertical_match}, H: {horizontal_match}",vscroll);
        self.logs_scroll_bar_hovered = horizontal_match && vertical_match;
    }

    pub fn logs_handle_mouse_scroll_drag(&mut self, _column: u16, row: u16) {

        if self.logs_scroll_bar_hovered {

            let max_scroll = self.total_line_count.saturating_sub(self.logs_area_height);

            let vscroll = self.vertical_scroll.unwrap_or(max_scroll);            
        
            self.dbg = format!("WE ARE MOVING TO {} (from: {vscroll}, min: 1, max:{max_scroll}) - last_pos:{}",row,self.last_mouse_down_y_pos);
            
            let click_position = (row as usize).min(self.logs_area_height).max(0);
            let percentage = click_position as f32 / self.logs_area_height as f32;
            let scroll_to = (percentage * self.total_line_count as f32).round() as usize;

            let new_val = scroll_to.min(max_scroll);
            if new_val == max_scroll {
                self.vertical_scroll = None;
                self.scroll_state = self.scroll_state.position(new_val);    
            } else {
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
        } else {

            self.last_mouse_down_y_pos = row as usize;
        }
        

    }
    

    pub fn logs_tab_scroll_up(&mut self, count:Option<usize>) {
        match self.vertical_scroll {
            Some(current) if current > 0 => {
                let new_val = current.saturating_sub(count.unwrap_or(1)).max(0);
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
            None => {
                let max = self.total_line_count.saturating_sub(self.logs_area_height);
                let new_val = max.saturating_sub(count.unwrap_or(1));
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
            _ => {}
        }
    }

    // we usually only call this if app.logs_area_height.saturating_div(2) is greater than 0
    pub fn logs_tab_scroll_down(&mut self, count:Option<usize>) {
        if self.vertical_scroll.is_some() {
            let current = self.vertical_scroll.unwrap_or_default();
            let max = self.total_line_count.saturating_sub(self.logs_area_height).saturating_sub(1);
            if current < max {
                let new_val = current.saturating_add(count.unwrap_or(1)).min(max);
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
                
            }
            else {
                self.vertical_scroll = None;
            }
        }
    }


    pub fn traf_tab_scroll_up(&mut self, count:Option<usize>) {
        
        self.traffic_tab_state.vertical_scroll = 
            self.traffic_tab_state.vertical_scroll.saturating_sub(count.unwrap_or(1));

        self.traffic_tab_state.vertical_scroll_state =
            self.traffic_tab_state.vertical_scroll_state.position(self.traffic_tab_state.vertical_scroll);
    
    }


    pub fn traf_tab_scroll_down(&mut self, count: Option<usize>) {
       
        let current = self.traffic_tab_state.vertical_scroll;
        let max = self.traffic_tab_state.total_rows.saturating_sub(self.traffic_tab_state.area_height);
        if current < max {
            let new_val = current.saturating_add(count.unwrap_or(1)).min(max);
            self.traffic_tab_state.vertical_scroll = new_val;
            self.traffic_tab_state.vertical_scroll_state = self.traffic_tab_state.vertical_scroll_state.position(new_val);
        }
        else {
            self.traffic_tab_state.vertical_scroll = max;
        }
    }
    
    

    // fn toggle_view(&mut self) {
        
        
    //     self.view_mode = match self.view_mode {
    //         ViewMode::Console => {
    //             ViewMode::TUI
    //         },
    //         ViewMode::TUI => {
    //             ViewMode::Console
    //         },
    //     };
    // }
}
