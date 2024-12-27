use std::sync::Arc;
use ratatui::layout::{ Constraint, Flex, Rect};
use ratatui::style::{Color, Modifier, Style, Styled, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{ Cell, Row, Scrollbar, ScrollbarOrientation, Table};
use crate::global_state::GlobalState;
use crate::types::tui_state::TuiState;
use super::Theme;

fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();

    let days = total_secs / 86400;          // 1 day = 86_400 seconds
    let hours = (total_secs % 86400) / 3600; 
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    
    if days > 0 {
        format!("{} days, {} hours, {} minutes", days, hours, minutes)
    } else if hours > 0 {
        format!("{} hours, {} minutes", hours, minutes)
    } else if minutes > 0 {
        format!("{} minutes, {} seconds", minutes,seconds)
    } else {
        format!("{} seconds", seconds)
    }
}

pub fn draw(
    f: &mut ratatui::Frame,
    _global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    area: Rect,
    theme: &Theme
) {

    let size = area.as_size();
    if size.height < 10 || size.width < 10 {
        return
    }

    let headers = [ "Task","Uptime","Child PID", "Current Status"];
    
    let mut rows : Vec<Vec<String>> =  crate::PROC_THREAD_MAP.iter().map(|guard| {
        let (_thread_id, thread_info) = guard.pair();
        let uptime = 
            if thread_info.pid.is_some() {
                if let Ok(d) = thread_info.started_at_time_stamp.elapsed() {
                    format_duration(d)
                } else {
                    format!("-")
                }
            } else {
                format!("-")
            };
        vec![
            format!("[PROC_HOST] {}",thread_info.config.host_name),
            uptime,
            format!("{}",thread_info.pid.as_ref().map_or(String::new(),|x|x.to_string())),
            format!("selected port: {:?}", thread_info.config.active_port)
        ]
    }).chain(crate::BG_WORKER_THREAD_MAP.iter().map(|guard|{
        let (thread_id, thread_info) = guard.pair();
        let is_dead = thread_info.liveness_ptr.upgrade().is_none();
        vec![
            format!("[BG_WORKER] {}",thread_id),
            format!("-"),
            if is_dead {"This task has exited.".into()} else { format!("Status: {}", thread_info.status) },
            format!("-"),
        ]
    })).collect();
    rows.sort_by_key(|x|x[0].to_string());

    // =======================================================================================
    let wrapped_line_count = rows.len();    
    tui_state.threads_tab_state.scroll_state.total_rows = wrapped_line_count;
    let height_of_threads_area = area.height.saturating_sub(0); // header and footer
    tui_state.threads_tab_state.scroll_state.area_height = height_of_threads_area as usize;
    tui_state.threads_tab_state.scroll_state.area_width = area.width as usize;
    let scroll_pos = { tui_state.threads_tab_state.scroll_state.vertical_scroll };
    let max_scroll_pos = rows.len().saturating_sub(height_of_threads_area as usize - 1);
    let visible_rows = area.height as usize;
    let start = scroll_pos.unwrap_or(max_scroll_pos);
    let end = std::cmp::min(start + visible_rows, rows.len());
    let display_rows = &rows[start..end];
    // =======================================================================================

    let is_dark_theme = matches!(&theme,Theme::Dark(_));
    let odd_row_bg = if is_dark_theme { Color::from_hsl(15.0, 10.0, 10.0) } else {
        Color::Rgb(250,250,250)
    };
    let row_bg =  if is_dark_theme { Color::from_hsl(10.0, 10.0, 5.0) } else {
        Color::Rgb(235,235,255)
    };
    let table_rows : Vec<_> = display_rows.iter().enumerate().map(|(i,row)| {
        
        let is_odd = i % 2 == 0;


        Row::new(row.iter().map(|x|Cell::new(x.to_string()))).height(1 as u16)
            .style(
                Style::new()
                    .bg(
                        if is_odd {
                            odd_row_bg
                        } else {
                            row_bg
                        }
                    ).fg(if is_dark_theme { Color::White } else { Color::Black })
                )
    }).collect();
    

    tui_state.threads_tab_state.scroll_state.visible_rows = display_rows.iter().len() as usize;
    tui_state.threads_tab_state.scroll_state.total_rows = rows.len();

    let widths = [
        Constraint::Min(10), 
        Constraint::Min(10), 
        Constraint::Min(10), 
        Constraint::Min(10),   
    ];
    
    
    let headers = Row::new(headers
        .iter()
        .map(|&h| Cell::from(h).fg(if is_dark_theme {Color::LightGreen} else {Color::Blue}).underlined().add_modifier(Modifier::BOLD))
    ).height(1);

    
    let table = Table::new(table_rows, widths.clone())
        .header(headers)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .widths(&widths)
        .flex(Flex::SpaceBetween)
        .column_spacing(1);

    f.render_widget(table, area);

    // ======================== SCROLL SECTION ======================================================== 

    let mut scrollbar = Scrollbar::default()
        .style(Style::default())
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")).thumb_style(Style::new().fg(Color::LightBlue))
        .orientation(ScrollbarOrientation::VerticalRight);

    tui_state.threads_tab_state.scroll_state.area_height = height_of_threads_area as usize;
    
    if tui_state.threads_tab_state.scroll_state.scroll_bar_hovered {
        scrollbar = scrollbar.thumb_style(Style::default().fg(Color::Yellow).bg(Color::LightRed));
    }

    let scrollbar_area = Rect::new(area.right() - 1, area.top(), 1, area.height);
    tui_state.threads_tab_state.scroll_state.vertical_scroll_state = 
        tui_state.threads_tab_state.scroll_state.vertical_scroll_state
            .content_length(rows.len().saturating_sub(height_of_threads_area as usize));
    if scroll_pos.is_none() {
        tui_state.threads_tab_state.scroll_state.vertical_scroll_state = 
            tui_state.threads_tab_state.scroll_state.vertical_scroll_state.position(rows.len().saturating_sub(height_of_threads_area as usize));
    }

    // ======================== RENDER ================================================================

    f.render_stateful_widget(scrollbar,scrollbar_area, &mut tui_state.threads_tab_state.scroll_state.vertical_scroll_state);

    
}
