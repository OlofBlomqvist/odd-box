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


pub fn draw(
    f: &mut ratatui::Frame, 
    _global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    log_buffer: &Arc<Mutex<SharedLogBuffer>>,
    area: Rect,
    theme: &Theme
) {
    
    let size = area.as_size();
    if size.height < 10 || size.width < 10 {
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
            Level::ERROR => Style::default().fg(Color::Red),
            Level::TRACE => Style::default().fg(Color::Gray),
            Level::DEBUG => Style::default().fg(Color::Magenta),
            Level::WARN  => Style::default().fg(Color::Yellow),
            Level::INFO  => Style::default().fg(Color::Blue),
        }            
    } else {
        match level {
            Level::ERROR => Style::default().fg(Color::Red),
            Level::TRACE => Style::default().fg(Color::Black),
            Level::DEBUG => Style::default().fg(Color::Black),
            Level::WARN  => Style::default().fg(Color::Magenta),
            Level::INFO  => Style::default().fg(Color::Blue),
        }   
    };

    let thread_name_style = if is_dark_theme { Style::default().fg(Color::DarkGray) } else { Style::default().fg(Color::Black) };
    let fg_s = if is_dark_theme { Style::default().fg(Color::White) } else { Style::default().fg(Color::Black) };

    
    let max_site_len = tui_state.site_rects.iter().map(|x|x.1.len()).max().unwrap_or(0);

    let items: Vec<Line> = 
            buffer.logs.iter_mut().enumerate().flat_map(|(i, x)| {
            
            let level = x.lvl;
            
            // TODO -- long lines get messed up
        
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
                format!("{n}{}", " ".repeat(max_site_len.saturating_sub(n.len())))
            } else {
                (" -").into()
            };

            x.msg = x.msg.trim().to_string();

            let max_width = (max_msg_width as usize)
                .saturating_sub(8)
                .saturating_sub(nr_str.len() + lvl_str.len() + thread_str.len());
            
            if x.msg.len() > max_width {
                wrap_string(x.msg.as_str(), max_width)
                    .into_iter().enumerate().map(|(i, m)| {
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
                            ])
                        } else {
                            // we add 2 spaces in thread names 
                            let padding = " ".repeat(lvl_str.len() + thread_str.len() -2);
                            Line::from(vec![
                                ratatui::text::Span::styled(nr_str.to_string(), fg_s),
                                ratatui::text::Span::styled(padding, Style::default()),
                                ratatui::text::Span::styled("", Style::default()),
                                ratatui::text::Span::styled("", Style::default()),
                                ratatui::text::Span::styled(m.clone(), s(level)),
                            ])
                        }
            
                       
                    }).collect::<Vec<Line>>()
            } else {
                let message = ratatui::text::Span::styled(
                    format!("{}{}", &x.src, &x.msg),
                    s(level),
                );
        
                vec![Line::from(vec![
                    ratatui::text::Span::styled(nr_str, fg_s),
                    ratatui::text::Span::styled(lvl_str, s(level)),
                    ratatui::text::Span::styled(thread_str, thread_name_style),
                    message,
                ])]
            }
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

    let mut scrollbar_state = tui_state.log_tab_stage.scroll_state.vertical_scroll_state.borrow_mut();
    *scrollbar_state = scrollbar_state.content_length(items.len().saturating_sub(height_of_logs_area as usize));

    if scroll_pos.is_none() {
        *scrollbar_state = scrollbar_state.position(items.len().saturating_sub(height_of_logs_area as usize));
    }
   

    f.render_widget(paragraph, area);
    f.render_stateful_widget(scrollbar,area, &mut scrollbar_state);

   

}

