use std::sync::Arc;

use ratatui::layout::{Flex, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, Table};
use crate::global_state::GlobalState;
use crate::types::proxy_state::*;
use crate::types::tui_state::TuiState;
use ratatui::layout::Constraint;
use super::Theme;


pub fn draw(
    f: &mut ratatui::Frame,
    global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    area: Rect,
    theme: &Theme
) {
    let headers = [ "Site", "Source", "Target", "Description"];
    
    let rows : Vec<Vec<String>> = global_state.app_state.statistics.active_connections.iter().map(|guard| {
        let (_,active_connection) = guard.pair();
        let typ = match &active_connection.connection_type {
            ProxyActiveConnectionType::TcpTunnelUnencryptedHttp => "UNENCRYPTED TCP TUNNEL".to_string(),
            ProxyActiveConnectionType::TcpTunnelTls => 
                "TLS ENCRYPTED TCP TUNNEL".to_string(),
            ProxyActiveConnectionType::TerminatingHttp { incoming_scheme, incoming_http_version, outgoing_scheme, outgoing_http_version }=> 
                format!("{incoming_scheme}@{incoming_http_version:?} <-TERMINATING_HTTP-> {outgoing_scheme}@{outgoing_http_version:?}"),
            ProxyActiveConnectionType::TerminatingWs { incoming_scheme, incoming_http_version, outgoing_scheme, outgoing_http_version } => 
                format!("{incoming_scheme}@{incoming_http_version:?} <-TERMINATING_WS-> {outgoing_scheme}@{outgoing_http_version:?}"),
        };
        let description = format!("{}",typ);
        // pub struct ProxyActiveConnection {
        //     pub target_name : String,
        //     pub creation_time : chrono::DateTime<chrono::Local>,
        //     pub description : Option<String>,
        //     pub connection_type : ProxyActiveConnectionType,
        //     pub source_addr: SocketAddr,
        //     pub target_addr: String
        // }
        
        vec![
            active_connection.target_name.clone(),
            active_connection.source_addr.to_string(), 
            active_connection.target_addr.clone(), 
            description
        ]
    }).collect();

    

    let header_height = 1;
    let visible_rows = area.height as usize - header_height;
    let start = tui_state.connections_tab_state.scroll_state.vertical_scroll.unwrap_or_default();
    let end = std::cmp::min(start + visible_rows, rows.len());

    let is_dark_theme = matches!(&theme,Theme::Dark(_));
    
    let display_rows = &rows[start..end];

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

    let widths = [
        Constraint::Fill(1), 
        Constraint::Fill(1), 
        Constraint::Fill(2), 
        Constraint::Fill(4),        
    ];
    
    
    let headers = Row::new(headers
        .iter()
        .map(|&h| Cell::from(h).fg(if is_dark_theme {Color::LightGreen} else {Color::Blue}).underlined().add_modifier(Modifier::BOLD))
    ).height(1);

    
    let table = Table::new(table_rows, widths.clone())
        .header(headers)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .widths(&widths)
        .flex(Flex::Legacy)
        .column_spacing(1);

    f.render_widget(table, area);


    let scrollbar = Scrollbar::default()
        .style(Style::default())
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")).thumb_style(Style::new().fg(Color::LightBlue))
        .orientation(ScrollbarOrientation::VerticalRight);

    let height_of_traf_area = area.height.saturating_sub(2); 
    tui_state.connections_tab_state.scroll_state.area_height = height_of_traf_area as usize;
    
    tui_state.connections_tab_state.scroll_state.vertical_scroll_state = tui_state.connections_tab_state.scroll_state.vertical_scroll_state.content_length(rows.len().saturating_sub(height_of_traf_area as usize));
    
    let scrollbar_area = Rect::new(area.right() - 1, area.top(), 1, area.height);

    f.render_stateful_widget(scrollbar,scrollbar_area, &mut tui_state.connections_tab_state.scroll_state.vertical_scroll_state);

}

