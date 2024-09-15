use std::sync::Arc;

use ratatui::layout::{Offset, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use crate::global_state::GlobalState;
use crate::types::tui_state::TuiState;
use super::Theme;




pub fn draw(
    f: &mut ratatui::Frame,
    global_state: Arc<GlobalState>,
    _tui_state: &mut TuiState,
    area: Rect,
    theme: &Theme,
) {
    let size = area.as_size();
    if size.height < 20 || size.width < 50 {
        return;
    }


    let is_dark_theme = matches!(&theme,Theme::Dark(_));

    use std::collections::HashSet;

    let tcp_total_connections = global_state
        .app_state
        .statistics
        .tunnelled_tcp_connections_per_hostname
        .iter()
        .map(|x| {
            let (_, count) = x.pair();
            count.load(std::sync::atomic::Ordering::SeqCst)
        })
        .sum::<usize>();

    let http_total_connections = global_state
        .app_state
        .statistics
        .terminated_http_connections_per_hostname
        .iter()
        .map(|x| {
            let (_, count) = x.pair();
            count.load(std::sync::atomic::Ordering::SeqCst)
        })
        .sum::<usize>();

    let mut unique_hostnames = HashSet::new();

    for x in global_state
        .app_state
        .statistics
        .tunnelled_tcp_connections_per_hostname
        .iter()
    {
        let (domain_name, _) = x.pair();
        unique_hostnames.insert(domain_name.clone());
    }

    for x in global_state
        .app_state
        .statistics
        .terminated_http_connections_per_hostname
        .iter()
    {
        let (domain_name, _) = x.pair();
        unique_hostnames.insert(domain_name.clone());
    }

    let num_unique_hostnames = unique_hostnames.len();

    let style = if is_dark_theme { Style::new().fg(Color::White) } else { Style::new().fg(Color::Black) };


    let p1 = Paragraph::new(format!(
        "Handled TCP tunnels: {}",
        tcp_total_connections
    )).style(style);
    
    let p2 = Paragraph::new(format!(
        "Terminated HTTP connections: {}",
        http_total_connections
    )).style(style);

    

    let p3 = Paragraph::new(format!("Number of unique hostnames seen: {}", num_unique_hostnames)).style(style);

    f.render_widget(p1, area.offset(Offset { x: 4, y: 1 }));
    f.render_widget(p2, area.offset(Offset { x: 4, y: 2 }));
    f.render_widget(p3, area.offset(Offset { x: 4, y: 3 }));


    // TODO - Use a scrollable table and display all host specific stats
    // TODO - Add data-transfer count for each hostname:
    //         - for tunnelled traffic, we can use the total number of bytes sent/received as observed by the bidirectional copy call.
    //         - for terminated traffic we can most likely add support for getting this data from the ManagedStream implementation


    f.render_widget(
        Paragraph::new(Text::styled("... This page will have more data in the future :-)", Style::default().fg(Color::DarkGray))),
        area.offset(Offset { x: 4, y: 5 })
            
    );

}
