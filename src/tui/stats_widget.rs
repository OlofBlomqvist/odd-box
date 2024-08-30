use std::sync::Arc;

use ratatui::layout::{Offset, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::widgets::Paragraph;
use tokio::sync::RwLockWriteGuard;
use crate::global_state::GlobalState;
use crate::types::app_state::*;
use crate::types::tui_state::TuiState;
use super::Theme;




pub fn draw(
    f: &mut ratatui::Frame, 
    global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    area: Rect,
    _theme: &Theme
) {

    let total_received_tcp_connections = global_state.request_count.load(std::sync::atomic::Ordering::Relaxed);

    let p = Paragraph::new(format!("Total received TCP connections: {total_received_tcp_connections}"));
    let p2 = Paragraph::new(format!("..More to come on this page at some point! :D")).fg(Color::DarkGray);
    
    f.render_widget(p, area.offset(Offset{x:4,y:2}));
    f.render_widget(p2, area.offset(Offset{x:4,y:4}));
}
