use std::fmt::{Display, Formatter};
use std::sync::Arc;
use ratatui::layout::{Flex, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, Table};
use crate::global_state::GlobalState;
use crate::types::connection_type::ConnectionType;
use crate::types::proxy_state::*;
use crate::types::tui_state::TuiState;
use ratatui::layout::Constraint;
use super::Theme;

impl Display for ConnectionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::FullTlsTerminatedWithHttpReEncryptedOutgoingTls => {
                write!(f, "Full TLS Terminated: HTTP processed, outgoing re-encrypted with TLS")
            }
            ConnectionType::MixedTerminationWithPlainOutgoing => {
                write!(f, "Mixed Termination: HTTP processed, outgoing is plain")
            }
            ConnectionType::TlsTerminatedWithReEncryptedOutgoingTls => {
                write!(f, "TLS Terminated: Outgoing re-encrypted with TLS")
            }
            ConnectionType::TlsTerminatedWithPlainOutgoing => {
                write!(f, "TLS Terminated: Outgoing is plain")
            }
            ConnectionType::HttpTerminatedWithOutgoingTls => {
                write!(f, "HTTP Terminated: Outgoing encrypted with TLS")
            }
            ConnectionType::HttpTerminatedWithPlainOutgoing => {
                write!(f, "HTTP Terminated: Outgoing is plain")
            }
            ConnectionType::OpaqueTlsForwarding => {
                write!(f, "Opaque TLS Forwarding: End-to-end encrypted")
            }
            ConnectionType::PlainEndToEndForwarding => {
                write!(f, "Plain Forwarding: No termination, data forwarded as plain")
            }
            ConnectionType::OpaqueIncomingTlsPassthroughWithPlainOutgoing => {
                write!(f, "Opaque TLS Passthrough: Forwarded to plain outgoing")
            }
            ConnectionType::OpaqueIncomingTlsPassthroughWithOutgoingTls => {
                write!(f, "Opaque TLS Passthrough: Forwarded to TLS outgoing")
            }
        }
    }
}


pub fn draw(
    f: &mut ratatui::Frame,
    global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    area: Rect,
    theme: &Theme
) {

    let size = area.as_size();
    if size.height < 10 || size.width < 10 {
        return
    }


    let headers = [ "Site", "Source","Destination", "Description"];
    
    let mut rows = global_state.app_state.statistics.active_connections.iter().map(|guard| {
        let (_,active_connection) = guard.pair();
        let src = active_connection.client_addr.clone();
        let backend = match &active_connection.backend {
            Some(b) => format!("{}:{}",b.address,b.port),
            None if active_connection.is_odd_box_admin_api_req => "odd-box".to_string(),
            None => match active_connection {
                ProxyActiveTCPConnection { dir_server: Some(_ds), .. } => format!("odd-box"),
                _ => "<UNKNOWN>".to_string()
            }
        };
        let connection_type = active_connection.get_connection_type().to_string();

        if let Some(t) = &active_connection.target {
            vec![
                t.host_name.clone(),
                src,
                backend,
                connection_type, 
                
            ]
        } else {
            vec![
                match (active_connection.dir_server.as_ref(),active_connection.is_odd_box_admin_api_req) {
                    (Some(ds),_) => format!("{}",ds.host_name.clone()),
                    (None,true) => "odd-box-admin-api".to_string(),
                    _ => "<UNKNOWN>".to_string()
                },
                src,
                backend,
                connection_type, 
            ]
        }        
    }).collect::<Vec<Vec<String>>>();
    
    rows.sort_by_key(|row| row[0].clone());
    
    // let rows : Vec<Vec<String>> = vec![
    //     vec!["QQQQQQQQQQ".to_string(),"BBBBBBB".to_string(),"CCCCCC".to_string(),"DDDDDD".to_string()],
    // ];

    // ====================================================================================================
    let wrapped_line_count = rows.len();    
    tui_state.connections_tab_state.scroll_state.total_rows = wrapped_line_count;
    let area_height = area.height.saturating_sub(0); // header and footer
    tui_state.connections_tab_state.scroll_state.area_height = area_height as usize;
    tui_state.connections_tab_state.scroll_state.area_width = area.width as usize;
    let scroll_pos = { tui_state.connections_tab_state.scroll_state.vertical_scroll };
    let max_scroll_pos = rows.len().saturating_sub(area_height as usize - 1);
    let visible_rows = area.height as usize;
    let start = scroll_pos.unwrap_or(max_scroll_pos);
    let end = std::cmp::min(start + visible_rows, rows.len());
    let display_rows = &rows[start..end];
    // ====================================================================================================

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
    

    tui_state.connections_tab_state.scroll_state.visible_rows = display_rows.iter().len() as usize;
    tui_state.connections_tab_state.scroll_state.total_rows = rows.len();


    
    
    let headers = Row::new(headers
        .iter()
        .map(|&h| Cell::from(h).fg(if is_dark_theme {Color::LightGreen} else {Color::Blue}).underlined().add_modifier(Modifier::BOLD))
    ).height(1);

    let widths = [
        Constraint::Fill(1),
        Constraint::Fill(1), 
        Constraint::Fill(1),         
        Constraint::Fill(4),        
    ];
    
    let table = Table::new(table_rows, widths.clone())
        .header(headers)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .widths(&widths)
        .flex(Flex::Legacy)
        .column_spacing(1);

    f.render_widget(table, area);

    // SCROLLING

    let mut scrollbar = Scrollbar::default()
        .style(Style::default())
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")).thumb_style(Style::new().fg(Color::LightBlue))
        .orientation(ScrollbarOrientation::VerticalRight);

    tui_state.connections_tab_state.scroll_state.area_height = area_height as usize;
    
    if tui_state.connections_tab_state.scroll_state.scroll_bar_hovered {
        scrollbar = scrollbar.thumb_style(Style::default().fg(Color::Yellow).bg(Color::Red));
    }

    let scrollbar_area = Rect::new(area.right() - 1, area.top(), 1, area.height);
    tui_state.connections_tab_state.scroll_state.vertical_scroll_state = 
        tui_state.connections_tab_state.scroll_state.vertical_scroll_state
            .content_length(rows.len().saturating_sub(area_height as usize));
    if scroll_pos.is_none() {
        tui_state.connections_tab_state.scroll_state.vertical_scroll_state = 
            tui_state.connections_tab_state.scroll_state.vertical_scroll_state.position(rows.len().saturating_sub(area_height as usize));
    }

    // RENDER

    f.render_stateful_widget(scrollbar,scrollbar_area, &mut tui_state.connections_tab_state.scroll_state.vertical_scroll_state);

}

