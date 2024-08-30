use std::borrow::BorrowMut;
use std::sync::{Arc, Mutex};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation};
use tokio::sync::RwLockWriteGuard;
use tracing::Level;
use crate::global_state::GlobalState;
use crate::logging::SharedLogBuffer;
use crate::types::app_state::*;
use crate::types::tui_state::TuiState;
use super::{wrap_string, Theme};


pub fn draw(
    f: &mut ratatui::Frame, 
    mut global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    log_buffer: &Arc<Mutex<SharedLogBuffer>>,
    area: Rect,
    _theme: &Theme
) {

    {
        let mut buffer = log_buffer.lock().expect("locking shared buffer mutex should always work");
        if tui_state.log_tab_stage.scroll_state.vertical_scroll.is_none() && buffer.limit.is_none() {
            let l = buffer.limit.borrow_mut();
            *l = Some(500);
        } else if tui_state.log_tab_stage.scroll_state.vertical_scroll.is_some() && buffer.limit.is_some() {
            let l = buffer.limit.borrow_mut();
            *l = None;
        }
    }


     let buffer = log_buffer.lock().expect("locking shared buffer mutex should always work");

    let max_msg_width = area.width;

    let item_count = buffer.logs.len().to_string().len().max(6);

    // we do this recalculation on each render in case of window-resize and such
    // we should move so that this is done ONCE per log message and not for each log message ever on each render.
    let items: Vec<Line> = buffer.logs.iter().enumerate().flat_map(|(i,x)|{
        
        let level = x.lvl;
        
        let s = match level {
            Level::ERROR => Style::default().fg(Color::Red),
            Level::TRACE => Style::default().fg(Color::Gray),
            Level::DEBUG => Style::default().fg(Color::Magenta),
            Level::WARN => Style::default().fg(Color::Yellow),
            Level::INFO => Style::default().fg(Color::Blue)
        };

        let nr_str = format!("{:1$} | ",i+1, item_count);
        let lvl_str = format!("{:>1$} ",x.lvl.as_str(),5);
        let thread_str = if let Some(n) = &x.thread {format!("{n} ")} else { format!("") };

        let number = ratatui::text::Span::styled(nr_str.clone(),Style::default().fg(Color::DarkGray));
        let level = ratatui::text::Span::styled(lvl_str.clone(),s);
        let thread_name = ratatui::text::Span::styled(thread_str.clone(),Style::default().fg(Color::DarkGray));

        // if x.msg is wider than the available width, we need to split the message in multiple lines..
        let max_width = (max_msg_width as usize).saturating_sub(8).saturating_sub(nr_str.len() + lvl_str.len() + thread_str.len());

        let l = if x.msg.len() > max_width as usize {
            
            wrap_string(x.msg.as_str(), max_width as usize)
            .into_iter().enumerate()
            .map(|(i,m)| 
                Line::from(
                    vec![
                        number.clone(),
                        if i == 0 { level.clone() } else { 
                            ratatui::text::Span::styled(" ".repeat(level.clone().content.len()).to_string() ,Style::default())
                        },
                        thread_name.clone(),
                        ratatui::text::Span::styled(m,Style::default())
                    ]
                )
            ).collect::<Vec<Line>>()

            
        } else {
            let message = ratatui::text::Span::styled(format!("{} {}",x.src.clone(),x.msg),Style::default());
            vec![Line::from(vec![number,level,thread_name,message])]
            
        };
        
        l

        
    }).collect();

    let wrapped_line_count = items.len();

    tui_state.log_tab_stage.scroll_state.total_rows = wrapped_line_count;
    
    let height_of_logs_area = area.height.saturating_sub(0); // header and footer
    tui_state.log_tab_stage.scroll_state.area_height = height_of_logs_area as usize;
    tui_state.log_tab_stage.scroll_state.area_width = area.width as usize;
    
    let scroll_pos = { tui_state.log_tab_stage.scroll_state.vertical_scroll };

    let scrollbar_hovered = tui_state.log_tab_stage.scroll_state.scroll_bar_hovered;
    let mut scrollbar_state = tui_state.log_tab_stage.scroll_state.vertical_scroll_state.borrow_mut();
   
    let max_scroll_pos = items.len().saturating_sub(height_of_logs_area as usize);
    
    //let clamped_scroll_pos = scroll_pos.unwrap_or(max_scroll_pos).min(max_scroll_pos) as u16;
   
    let visible_rows = area.height as usize; // Adjust as needed based on your UI

    let start = scroll_pos.unwrap_or(max_scroll_pos);
    let end = std::cmp::min(start + visible_rows, items.len());


    if start > items.len() || end > items.len() || start >= end {
        return
    }

    let display_rows = &items[start..end];


    let clamped_items : Vec<Line> = display_rows.iter().map(|x| {
        x.clone()
    }).collect();

    let paragraph = Paragraph::new(clamped_items);

    let mut scrollbar = Scrollbar::default()
        .style( Style::default())
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")).thumb_style(Style::new().fg(Color::LightBlue));

    if scrollbar_hovered {
        scrollbar = scrollbar.thumb_style(Style::default().fg(Color::Yellow).bg(Color::Red));
    }

    *scrollbar_state = scrollbar_state.content_length(items.len().saturating_sub(height_of_logs_area as usize));

    if scroll_pos.is_none() {
        *scrollbar_state = scrollbar_state.position(items.len().saturating_sub(height_of_logs_area as usize));
    }
   

    f.render_widget(paragraph, area);
    f.render_stateful_widget(scrollbar,area, &mut scrollbar_state);

   

}

