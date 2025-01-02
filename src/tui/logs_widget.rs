use std::borrow::{BorrowMut, Cow};
use std::sync::{Arc, Mutex};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation};
use tracing::Level;
use crate::global_state::GlobalState;
use crate::logging::SharedLogBuffer;
use crate::types::tui_state::TuiState;
use super::{wrap_string, Theme};


fn trim_to_max_chars(s: &str, max_chars: usize) -> String {
    let mut end = s.len(); 
    for (idx, _) in s.char_indices().take(max_chars) {
        end = idx;
    }

    // note : we CANNOT just do this directly without the indices 
    // as we might end up on non-valid utf8 positions like being inside of 'å'
    // which will then PANIC ...
    if s.chars().count() > max_chars {
        format!("{}..", &s[..end]) // <-- ie. this right here, CAN PANIC if using bad index even if inside of the string
    } else {
        s.to_string()
    }
}

pub fn draw(
    f: &mut ratatui::Frame, 
    _global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    log_buffer: &Arc<Mutex<SharedLogBuffer>>,
    area: Rect,
    theme: &Theme
) {
    
    let size = area.as_size();
    if size.height < 10 || size.width < 80 {
        f.render_widget(Paragraph::new("Please stop squeezing me.."),area);
        return
    }

    let mut buffer = log_buffer.lock().expect("locking shared buffer mutex should always work");

    if tui_state.log_tab_stage.scroll_state.vertical_scroll.is_none() && buffer.limit.is_none() {
        let l = buffer.limit.borrow_mut();
        *l = Some(500);
    } else if tui_state.log_tab_stage.scroll_state.vertical_scroll.is_some() && buffer.limit.is_some() {
        let l = buffer.limit.borrow_mut();
        *l = None;
    }


    let max_msg_width = area.width;

    // if we have 0-9 messages, the len will be 1, if we have 10-99 messages, the len will be 2, etc.
    let item_count_len = buffer.logs.len().to_string().len().max(6);
    
    let is_dark_theme = matches!(&theme,Theme::Dark(_));
    
    let s = |level|if is_dark_theme { 
        match level {
            Level::ERROR => Style::default().fg(Color::LightRed),
            Level::TRACE => Style::default().fg(Color::LightGreen),
            Level::DEBUG => Style::default().fg(Color::LightBlue),
            Level::WARN  => Style::default().fg(Color::Yellow),
            Level::INFO  => Style::default().fg(Color::Gray),
        }            
    } else {
        match level {
            Level::ERROR => Style::default().fg(Color::Red),
            Level::TRACE => Style::default().fg(Color::Blue),
            Level::DEBUG => Style::default().fg(Color::Green),
            Level::WARN  => Style::default().fg(Color::Magenta),
            Level::INFO  => Style::default().fg(Color::Black),
        }   
    };

    let thread_name_style = if is_dark_theme { 
        Style::default().fg(Color::DarkGray) 
    } else { 
        Style::default().fg(Color::Black) 
    };
    let fg_s = if is_dark_theme { 
        Style::default().fg(Color::White) 
    } else { 
        Style::default().fg(Color::Black) 
    };

    
    let max_site_len = tui_state.site_rects.iter().map(|x|x.1.len()).max().unwrap_or(0);

    //let mut odd = false;
    
    // let odd_row_bg = if is_dark_theme { Color::from_hsl(15.0, 10.0, 10.0) } else {
    //     Color::Rgb(250,250,250)
    // };

    // let row_bg =  if is_dark_theme { Color::from_hsl(10.0, 10.0, 5.0) } else {
    //     Color::Rgb(235,235,255)
    // };

    let items: Vec<Line> = 
            buffer.logs.iter_mut().enumerate().flat_map(|(i, x)| {
            
            let level = x.lvl;
                    
            let nr_str = format!("{:1$} | ", i + 1, item_count_len);
            let system_message = x.thread.clone().unwrap_or_default().starts_with("odd_box");
            let lvl_str = format!("{:>1$} ", match x.lvl {

                Level::INFO  if system_message => "INF 🧊",
                Level::TRACE if system_message => "TRC 🧊",
                Level::DEBUG if system_message => "DBG 🧊",
                Level::ERROR if system_message => "ERR 🧊",
                Level::WARN  if system_message => "WRN 🧊",

                Level::INFO => "INF 🥝",
                Level::TRACE => "TRC 🥸",
                Level::DEBUG => "DBG 👀",
                Level::ERROR => "ERR 👺",
                Level::WARN => "WRN 🥦",
                
            }, 5);

            let thread_str = if let Some(n) = &x.thread {
                format!("{n}{}  ", " ".repeat(max_site_len.saturating_sub(n.len())))
            } else {
                (" -").into()
            };

            x.msg = trim_to_max_chars(x.msg.trim(),2000);

            let max_width = ((max_msg_width + 2) as usize)
                .saturating_sub(nr_str.len() + lvl_str.len() + thread_str.len());
            //odd = !odd;
            
            // let bg = if odd {
            //     odd_row_bg
            // } else {
            //     row_bg
            // };

            
            let wrapped = wrap_string(&x.msg, max_width);
    
    
            wrapped.into_iter().enumerate().map(|(i, m)| {

                    let m = format!("{m}{}"," ".repeat(max_width-m.len()));

                    let level_span = if i == 0 {
                        ratatui::text::Span::styled(lvl_str.clone(), s(level))
                    } else {
                        ratatui::text::Span::styled(
                            Cow::from(" ".repeat(lvl_str.len() - 1)),
                            Style::default(),
                        )
                    };
                    

                    if i == 0 {
                        Line::from(vec![
                            ratatui::text::Span::styled(nr_str.to_string(), fg_s),
                            level_span,
                            ratatui::text::Span::styled(thread_str.to_string(), thread_name_style),
                            ratatui::text::Span::styled(m.clone(), s(level)),
                        ]) // .bg(bg) **looks a bit strange tbh
                    } else {
                        // we add 2 spaces in thread names 
                        let padding = " ".repeat(lvl_str.len() + thread_str.len() - 2);
                        Line::from(vec![
                            ratatui::text::Span::styled(nr_str.to_string(), fg_s),
                            ratatui::text::Span::styled(padding, Style::default()),
                            ratatui::text::Span::styled("", Style::default()),
                            ratatui::text::Span::styled("", Style::default()),
                            ratatui::text::Span::styled(m.clone(), s(level)),
                        ]) //.bg(bg) **looks a bit strange tbh
                    }

                    
        
                    
                }).collect::<Vec<Line>>()
        }).collect();

    let wrapped_line_count = items.len();
    tui_state.log_tab_stage.scroll_state.total_rows = wrapped_line_count;
    let height_of_logs_area = area.height.saturating_sub(0); // header and footer
    tui_state.log_tab_stage.scroll_state.area_height = height_of_logs_area as usize;
    tui_state.log_tab_stage.scroll_state.area_width = area.width as usize;
    let scroll_pos = { tui_state.log_tab_stage.scroll_state.vertical_scroll };
    let scrollbar_hovered = tui_state.log_tab_stage.scroll_state.scroll_bar_hovered;
    let max_scroll_pos = items.len().saturating_sub(height_of_logs_area as usize);
    let visible_rows = area.height as usize;
    let start = scroll_pos.unwrap_or(max_scroll_pos);
    let end = std::cmp::min(start + visible_rows, items.len());
    if start > items.len() || end > items.len() || start >= end {
        return
    }

    let clamped_items : Vec<Line> = items[start..end].to_vec();

    let paragraph = Paragraph::new(clamped_items);

    let mut scrollbar = Scrollbar::default()
        .style( Style::default())
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")).thumb_style(Style::new().fg(Color::LightBlue));

    if scrollbar_hovered {
        scrollbar = scrollbar.thumb_style(Style::default().fg(Color::Yellow).bg(Color::LightRed));
    }

    let mut scrollbar_state = tui_state.log_tab_stage.scroll_state.vertical_scroll_state.borrow_mut();
    *scrollbar_state = scrollbar_state.content_length(items.len().saturating_sub(height_of_logs_area as usize));

    if scroll_pos.is_none() {
        *scrollbar_state = scrollbar_state.position(items.len().saturating_sub(height_of_logs_area as usize));
    }
   

    f.render_widget(paragraph, area);
    f.render_stateful_widget(scrollbar,area, &mut scrollbar_state);

   

}

