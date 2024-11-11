use std::fmt::{Display, Formatter};
use std::sync::Arc;
use ratatui::layout::{Flex, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, Table};
use crate::global_state::GlobalState;
use crate::types::proxy_state::*;
use crate::types::tui_state::TuiState;
use ratatui::layout::Constraint;
use super::Theme;

#[derive(Debug)]
enum ConnectionType {
    /// Full-TLS Terminated Proxy: The incoming connection uses TLS and is terminated. HTTP processing occurs,
    /// and a new outgoing connection is established with re-encrypted TLS.
    FullTlsTerminatedWithHttpReEncryptedOutgoingTls,

    /// Mixed Termination: The incoming connection uses TLS, is terminated, HTTP processing occurs,
    /// and a plain (unencrypted) outgoing connection is established.
    MixedTerminationWithPlainOutgoing,

    /// TLS Terminated Proxy with End-to-End TLS: Incoming connection uses TLS and is terminated,
    /// and a new TLS connection is established with the outgoing target.
    TlsTerminatedWithReEncryptedOutgoingTls,

    /// TLS Terminated Proxy: Incoming connection uses TLS, is terminated, and the outgoing connection is plain (unencrypted).
    TlsTerminatedWithPlainOutgoing,

    /// Opaque HTTP with TLS: The incoming connection is plaintext, HTTP is processed, and the outgoing
    /// connection is encrypted with TLS.
    HttpTerminatedWithOutgoingTls,

    /// Plain HTTP Proxy: The incoming connection is plaintext, HTTP is processed, and the outgoing connection
    /// is also plaintext.
    HttpTerminatedWithPlainOutgoing,

    /// Opaque TLS Forwarding: Incoming TLS traffic is not terminated. Data is forwarded as-is to the outgoing
    /// target, preserving end-to-end encryption without decryption or re-encryption.
    OpaqueTlsForwarding,

    /// Plain End-to-End Forwarding: No TLS or HTTP termination occurs. Data is forwarded in plaintext
    /// from incoming to outgoing without modification.
    PlainEndToEndForwarding,

    /// Opaque Incoming TLS Passthrough with Plain Outgoing: Incoming connection uses TLS but is not terminated,
    /// and data is forwarded as plaintext to the outgoing connection.
    OpaqueIncomingTlsPassthroughWithPlainOutgoing,

    /// Opaque Incoming TLS Passthrough with Re-encrypted TLS: Incoming connection uses TLS but is not terminated,
    /// and data is forwarded as opaque to an outgoing TLS connection.
    OpaqueIncomingTlsPassthroughWithOutgoingTls,
}

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

    let headers = [ "Site", "Endpoints", "Description"];
    
    let rows : Vec<Vec<String>> = global_state.app_state.statistics.active_connections.iter().map(|guard| {
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
        let connection_type = match (
            active_connection.incoming_connection_uses_tls,
            active_connection.tls_terminated,
            active_connection.http_terminated,
            active_connection.outgoing_connection_is_tls,
        ) {
            (false,false,false,false) => ConnectionType::PlainEndToEndForwarding,
            (true, true, true, true) => ConnectionType::FullTlsTerminatedWithHttpReEncryptedOutgoingTls,
            (true, true, true, false) => ConnectionType::MixedTerminationWithPlainOutgoing,
            (true, true, false, true) => ConnectionType::TlsTerminatedWithReEncryptedOutgoingTls,
            (true, true, false, false) => ConnectionType::TlsTerminatedWithPlainOutgoing,
            (true, false, true, true) => ConnectionType::OpaqueIncomingTlsPassthroughWithOutgoingTls,
            (true, false, true, false) => ConnectionType::OpaqueIncomingTlsPassthroughWithPlainOutgoing,
            (true, false, false, true) => ConnectionType::OpaqueIncomingTlsPassthroughWithOutgoingTls,
            (true, false, false, false) => ConnectionType::OpaqueIncomingTlsPassthroughWithPlainOutgoing,
            (false, true, true, true) => ConnectionType::HttpTerminatedWithOutgoingTls,
            (false, true, true, false) => ConnectionType::HttpTerminatedWithPlainOutgoing,
            (false, true, false, true) => ConnectionType::TlsTerminatedWithReEncryptedOutgoingTls,
            (false, true, false, false) => ConnectionType::TlsTerminatedWithPlainOutgoing,
            (false, false, true, true) => ConnectionType::HttpTerminatedWithOutgoingTls,
            (false, false, true, false) => ConnectionType::HttpTerminatedWithPlainOutgoing,
            (false, false, false, true) => ConnectionType::OpaqueTlsForwarding
        }.to_string();


        if let Some(t) = &active_connection.target {
            vec![
                t.host_name.clone(),
                format!("{} <--> {}",src,backend),
                connection_type, 
                
            ]
        } else {
            vec![
                match (active_connection.dir_server.as_ref(),active_connection.is_odd_box_admin_api_req) {
                    (Some(ds),_) => format!("{}",ds.host_name.clone()),
                    (None,true) => "odd-box-admin-api".to_string(),
                    _ => "<UNKNOWN>".to_string()
                },
                format!("{} <--> {}",src,backend),
                connection_type, 
            ]
        }        
    }).collect();

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
        Constraint::Fill(2), 
        Constraint::Fill(3),        
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

