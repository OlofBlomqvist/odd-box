use std::fmt::{Display, Formatter};
use std::sync::Arc;
use ratatui::layout::{Flex, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::widgets::{Cell, Row, Scrollbar, ScrollbarOrientation, Table};
use crate::global_state::GlobalState;
use crate::types::connection_type::ConnectionType;
use crate::types::tui_state::TuiState;
use ratatui::layout::Constraint;
use super::Theme;

impl Display for ConnectionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::TlsTerminatedWithOutgoingClearText(target,backend) => write!(f, "HTTPS to HTTP Tunnel -> {}:{} ({})",backend.address,backend.port,target.host_name),
            ConnectionType::TlsTerminatedWithOutgoingTLS(target,backend) => write!(f, "HTTPS Tunnel to {}:{} ({})",backend.address,backend.port,target.host_name),
            ConnectionType::TlsTermination => write!(f, "HTTPS Terminated"),
            ConnectionType::TlsPassthru(target,backend) => write!(f, "TLS Passthru to {}:{} ({})",backend.address,backend.port,target.host_name),
            ConnectionType::HttpPassthru(target,backend) => write!(f, "HTTP Passthru to {}:{} ({})",backend.address,backend.port,target.host_name),
            ConnectionType::HttpTermination => write!(f, "HTTP Terminated"),
            ConnectionType::HttpWithOutgoingTLS(target,backend) => write!(f, "HTTP to HTTPS Tunnel -> {}:{} ({})",backend.address,backend.port,target.host_name),
            ConnectionType::PendingInit => write!(f, "Initializing.."),
            ConnectionType::Invalid(e) => write!(f, "Invalid: {e}"),
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


    let headers = [  "Source", "In/Out","Description"];

    let mut rows = global_state.app_state.statistics.active_connections.iter().map(|guard| {

        let (_,active_connection) = guard.pair();



        let local_info = if let Some(v) = global_state.monitoring_station.tcp_connections.get(&active_connection.connection_key) {
            v.local_process_name_and_pid.clone()
        } else { None };

        let src = if let Some((n,p)) = local_info {
            format!("{} ({} *{}*)",active_connection.client_addr_string.clone(),n,p)
        } else {
            active_connection.client_addr_string.clone()
        };

        let grpc_tag = if active_connection.is_grpc.unwrap_or_default() { "[gRPC] "} else { "" };
        let ws_tag = if active_connection.is_websocket.unwrap_or_default() { "[WS] "} else { "" };
        let connection_type = format!("{grpc_tag}{ws_tag}{}. {}",
            active_connection.get_connection_type(),
            active_connection.resolved_connection_type_description.as_ref().unwrap_or(&String::new())
        );

        let trans = if let Some(c) = global_state.monitoring_station.tcp_connections.get(&active_connection.connection_key) {
            if c.bytes_rec == 0 && c.bytes_sent == 0 {
                format!("-")
            } else {
                format!("{} bytes / {} bytes ", c.bytes_rec,c.bytes_sent)
            }

        } else {
            format!("not tracked")
        };

        vec![
             src,
             trans,
             connection_type,
         ]
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
    // let odd_row_bg = if is_dark_theme { Color::from_hsl(15.0, 10.0, 10.0) } else {
    //     Color::Rgb(250,250,250)
    // };
    // let row_bg =  if is_dark_theme { Color::from_hsl(10.0, 10.0, 5.0) } else {
    //     Color::Rgb(235,235,255)
    // };
    let table_rows : Vec<_> = display_rows.iter().enumerate().map(|(_i,row)| {
        //let is_odd = i % 2 == 0;
        Row::new(row.iter().map(|x|Cell::new(x.to_string()))).height(1 as u16)
            .style(
                Style::new()
                    // .bg(
                    //     if is_odd {
                    //         odd_row_bg
                    //     } else {
                    //         row_bg
                    //     }
                    // )
                    .fg(if is_dark_theme { Color::White } else { Color::Black })
                )
    }).collect();


    tui_state.connections_tab_state.scroll_state.visible_rows = display_rows.iter().len() as usize;
    tui_state.connections_tab_state.scroll_state.total_rows = rows.len();




    let headers = Row::new(headers
        .iter()
        .map(|&h| Cell::from(h).fg(if is_dark_theme {Color::LightGreen} else {Color::Blue}).underlined().add_modifier(Modifier::BOLD))
    ).height(1);

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
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
        scrollbar = scrollbar.thumb_style(Style::default().fg(Color::Yellow).bg(Color::LightRed));
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
