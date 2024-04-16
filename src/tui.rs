use ratatui::layout::{Alignment, Flex, Margin, Offset, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{BorderType, Cell, List, ListItem, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table};
use tokio::sync::RwLockWriteGuard;
use tokio::task;
use tracing::{Level, Subscriber};
use tracing_subscriber::{Layer, EnvFilter};
use tracing_subscriber::layer::{Context, SubscriberExt};
use std::borrow::BorrowMut;
use std::collections::{HashMap, VecDeque};
use std::io::Stdout;
use crate::types::app_state::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::http_proxy::ProcMessage;

use crate::types::proxy_state::*;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph}
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

#[derive(Clone)]
struct LogMsg {
    msg: String,
    lvl: Level,
    src: String,
    thread: Option<String>
}

struct SharedLogBuffer {
    pub (crate) logs: VecDeque<LogMsg>,
    pub (crate) limit : Option<usize>
}

impl SharedLogBuffer {
    
    fn new() -> Self {
        SharedLogBuffer {
            logs: VecDeque::new(),
            limit: Some(500)
        }
    }

    fn push(&mut self, message: LogMsg) {

        self.logs.push_back(message);
        match self.limit {
            Some(x) => {
                while self.logs.len() > x {
                    self.logs.pop_front();
                }
            },
            None => {},
        }
        
    }


}

struct LogVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for LogVisitor {

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {        
        self.fields.insert(field.name().to_string(), format!("{:?}",value));
    }

}
impl LogVisitor {
    fn new() -> Self {
        LogVisitor {
            fields: HashMap::new(),
        }
    }
    fn result(self) -> String {
        self.fields
            .iter()
            .map(|(_key, value)| format!("{}", value))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

struct TuiLoggerLayer {
    log_buffer: Arc<Mutex<SharedLogBuffer>>,
}

impl<S: Subscriber> Layer<S> for TuiLoggerLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();


        let target = metadata.target();
        
        // Create a visitor to format the fields of the event.
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);
        
        let mut msg =  visitor.result();
        let mut src = String::new();
        


        if msg.starts_with("[") && msg.contains("]") {
            let end = msg.find("]").unwrap_or_default();
            src = msg[1..end].to_string().trim().to_string();
            msg = msg[end+1..].to_string().trim().to_string();
        }

        if src.is_empty() {
            if !target.ends_with("proc_host") {
                src = target.into();
            }
            
        }

        let current_thread = std::thread::current();
        let current_thread_name = current_thread.name().and_then(|x|Some(x.to_string())).unwrap_or(format!("HAH!"));
        let mut skip_src = false;
        let thread_name = if current_thread_name == "tokio-runtime-worker" { 
            skip_src = true;
            Some(src.to_string()) 
        } else { 
            Some(current_thread_name) 
        };

        let log_message = LogMsg {
            thread: thread_name,
            lvl: metadata.level().clone(),
            src: if skip_src { "".into() } else {src},
            msg,
        };

        let mut buffer = self.log_buffer.lock().expect("must always be able to lock log buffer");
        buffer.push(log_message);
    
    }
}


pub (crate) fn init() {
    _ = enable_raw_mode().expect("must be able to enable raw mode");();
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).expect("must always be able to enter alt screen");
}

pub (crate) async fn run(
    filter:EnvFilter,
    shared_state:Arc<tokio::sync::RwLock<AppState>>,
    tx: tokio::sync::broadcast::Sender<ProcMessage>
) {
    
    let log_buffer = Arc::new(Mutex::new(SharedLogBuffer::new()));
    let layer = TuiLoggerLayer { log_buffer: log_buffer.clone() };

    let subscriber = tracing_subscriber::registry()
        .with(filter).with(layer);


    tracing::subscriber::set_global_default(subscriber).expect("Failed to set collector");
  

    let backend = CrosstermBackend::new(std::io::stdout());
    
    let terminal = Terminal::new(backend).expect("must be possible to create terminal");
    
    let terminal = Arc::new(tokio::sync::Mutex::new(terminal));
    
    
    let mut theme = match dark_light::detect() {
        dark_light::Mode::Dark => Theme::Dark(dark_theme()),
        dark_light::Mode::Light => Theme::Light(light_theme()),
        dark_light::Mode::Default => Theme::Dark(dark_theme()),
    };
    let mut count = 0;


    // TUI event loop
    let tui_handle = {
        let terminal = Arc::clone(&terminal);
        let app_state = Arc::clone(&shared_state);
        let tx = tx.clone();
        task::spawn(async move {
            
            let tx = tx.clone();
           
            let mut last_key_time = tokio::time::Instant::now();
            let debounce_duration = Duration::from_millis(100);
            
            //let mut last_toggle : Option<tokio::time::Instant> = None;
            
            loop {

                {
                    let app = app_state.read().await;

                    if app.exit {
                        if app.procs.iter().find(|x|
                               x.1 == &ProcState::Stopping 
                            || x.1 == &ProcState::Running
                            || x.1 == &ProcState::Starting 
                            
                        ).is_none() {
                            break; // nothing is running,stopping or starting.. we can exit now
                        }
                    }
                }

                if count > 100 {
                    theme = match dark_light::detect() {
                        dark_light::Mode::Dark => Theme::Dark(dark_theme()),
                        dark_light::Mode::Light => Theme::Light(light_theme()),
                        dark_light::Mode::Default => Theme::Dark(dark_theme()),
                    };
                    count = 0;
                
                }
                
                // KEEP LOCK SHORT TO AVOID DEADLOCK
                {
                    let mut state = app_state.write().await;
                    
                    let mut terminal = terminal.lock().await;
                    
                    terminal.draw(|f| draw_ui::<CrosstermBackend<Stdout>>(f, &mut state, &log_buffer,&theme))?;
                        
                }

                // }


                // Handle input
                if event::poll(std::time::Duration::from_millis(20))? {
                    let now = tokio::time::Instant::now();
                    let time_since_last_keypress = now.duration_since(last_key_time);
                    
                    // let time_since_last_toggle = if let Some(t) = last_toggle {
                    //     Some(now.duration_since(t))
                    // } else {
                    //     None
                    // };
                    let (current_page,sites_open) = {
                        let guard =  app_state.read().await;
                        (guard.current_page.clone(),guard.show_apps_window)
                    };
                    let evt = event::read()?;
                    match evt {
                        Event::Mouse(mouse) => {
                                if sites_open {
                                    match mouse.kind {
                                        event::MouseEventKind::Moved => {
                                            let mut app = app_state.write().await;
                                            app.sites_handle_mouse_hover(mouse.column,mouse.row)
                                        }
                                        event::MouseEventKind::Down(event::MouseButton::Left) => {
                                            let mut app = app_state.write().await;
                                            app.sites_handle_mouse_click(mouse.column,mouse.row,tx.clone())
                                        }
                                        _ => {}
                                    }
                                }
                                match current_page {
                                    Page::Logs => {
                                        match mouse.kind {
                                            event::MouseEventKind::Drag(event::MouseButton::Left) => {
                                                let mut app = app_state.write().await;
                                                app.logs_handle_mouse_scroll_drag(mouse.column,mouse.row)
                                            }
                                            event::MouseEventKind::Moved => {
                                                let mut app = app_state.write().await;
                                                app.logs_handle_mouse_move(mouse.column,mouse.row)
                                            }
                                            event::MouseEventKind::ScrollDown => {
                                                let mut app = app_state.write().await;
                                                app.logs_tab_scroll_down(Some(10));
                                            },
                                            event::MouseEventKind::ScrollUp => {
                                                let mut app = app_state.write().await;
                                                app.logs_tab_scroll_up(Some(10));
                                            },
                                            _ => {}
                                        }
                                    },
                                    Page::Statistics => {},
                                    Page::Connections => {
                                        match mouse.kind {
                                            event::MouseEventKind::ScrollDown => {
                                                let mut app = app_state.write().await;
                                                app.traf_tab_scroll_down(Some(10));
                                            },
                                            event::MouseEventKind::ScrollUp => {
                                                let mut app = app_state.write().await;
                                                app.traf_tab_scroll_up(Some(10));
                                            },
                                            _ => {}
                                        }
                                    },
                                }
                                
                        }
                        Event::Key(key) => {

                            if time_since_last_keypress >= debounce_duration { 
                                last_key_time = now;
                                
                                match current_page {
                                    Page::Logs => {
                                        match key.code {
                                            KeyCode::Enter => {
                                                let mut app = app_state.write().await;
                                                app.vertical_scroll = None;
                                                let mut buf = log_buffer.lock().expect("must always be able to lock log buffer");
                                                match buf.limit {
                                                    Some(x) => {
                                                        while buf.logs.len() > x {
                                                            buf.logs.pop_front();
                                                        }
                                                    },
                                                    None => {},
                                                }
                                                
                                            }    
                                            KeyCode::Up => {
                                                let mut app = app_state.write().await;
                                                app.logs_tab_scroll_up(Some(1));
                                            }
                                            KeyCode::Down => {
                                                let mut app = app_state.write().await;
                                                app.logs_tab_scroll_down(Some(1));
                                            }
                                            KeyCode::PageUp => {
                                                let mut app = app_state.write().await;
                                                let scroll_count = app.logs_area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    app.logs_tab_scroll_up(Some(scroll_count));
                                                }
                                            }
                                            KeyCode::PageDown => {
                                                let mut app = app_state.write().await;
                                                let scroll_count = app.logs_area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    app.logs_tab_scroll_down(Some(scroll_count));
                                                }
                                            },   
                                            KeyCode::Char('c') => {
                                                let mut app = app_state.write().await;
                                                let mut buf = log_buffer.lock().expect("must always be able to lock log buffer");
                                                app.total_line_count = 0;
                                                app.vertical_scroll = None;
                                                app.scroll_state = app.scroll_state.position(0);
                                                buf.logs.clear();
                                                
                                            }    
                                            _ => {}        
                                        }
                                    }
                                    Page::Statistics => {},
                                    Page::Connections => {
                                        match key.code {
                                            KeyCode::PageUp => {
                                                let mut app = app_state.write().await;
                                                app.traf_tab_scroll_up(Some(10));
                                            }
                                            KeyCode::PageDown => {
                                                let mut app = app_state.write().await;
                                                app.traf_tab_scroll_down(Some(10));
                                                
                                            },
                                            KeyCode::Up => {
                                                let mut app = app_state.write().await;
                                                let scroll_count = app.logs_area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    app.traf_tab_scroll_up(None);
                                                }
                                            }
                                            KeyCode::Down => {
                                                let mut app = app_state.write().await;
                                                app.traf_tab_scroll_down(None);
                                            }
                                            _ => {}
                                        }
                                    },
                                }

                                match key.code {    
                                    KeyCode::Char('1') => {
                                        let mut app = app_state.write().await;
                                        app.current_page = Page::Logs;
                                    }
                                    KeyCode::Char('2') => {
                                        let mut app = app_state.write().await;
                                        app.current_page = Page::Connections;
                                    }
                                    KeyCode::Char('3') => {
                                        let mut app = app_state.write().await;
                                        app.current_page = Page::Statistics;
                                    }
                                    KeyCode::BackTab | KeyCode::Left => {
                                        let mut app = app_state.write().await;
                                        match app.current_page {
                                            Page::Logs => app.current_page = Page::Statistics, 
                                            Page::Statistics => app.current_page = Page::Connections, 
                                            Page::Connections => app.current_page = Page::Logs,
                                        }
                                    }
                                    KeyCode::Tab | KeyCode::Right => {
                                        let mut app = app_state.write().await;
                                        match app.current_page {
                                            Page::Logs => app.current_page = Page::Connections, 
                                            Page::Statistics => app.current_page = Page::Logs, 
                                            Page::Connections => app.current_page = Page::Statistics,
                                        }
                                    }          
                                    KeyCode::Char('z') => {
                                        {
                                            let mut app = app_state.write().await;
                                            for (_,state) in app.procs.iter_mut() {
                                                if let ProcState::Running = state {
                                                    *state = ProcState::Stopping;
                                                }
                                            }
                                        }
                                        tx.clone().send(ProcMessage::StopAll).expect("must always be able to send internal messages");
                                    } 
                                    KeyCode::Char('s') => {
                                        {
                                            let mut app = app_state.write().await;
                                            for (_,state) in app.procs.iter_mut() {
                                                if let ProcState::Stopped = state {
                                                    *state = ProcState::Starting;
                                                }
                                            }
                                        }
                                        tx.clone().send(ProcMessage::StartAll).expect("must always be able to send internal messages");
                                    }    
                                    KeyCode::Char('a') => {
                                        //if let Some(t) = time_since_last_toggle{
                                             //{
                                                let mut app = app_state.write().await;
                                                app.show_apps_window = !app.show_apps_window;
                                                //last_toggle = Some(now)
                                            //}
                                        //}
                                    }                                  
                                    KeyCode::Esc | KeyCode::Char('q')=> {
                                        {
                                            let mut app = app_state.write().await;
                                            app.exit = true;
                                        }
                                        
                                    }
                                    _ => {
    
                                    }
                                }
                                
                                
                            }
                            
                        },
                        _=> {}
                    }
                
                }
            } 
            Result::<(), std::io::Error>::Ok(())
        })
        
    };

    _ = tui_handle.await.ok();

    _ = disable_raw_mode().ok();
    let mut stdout = std::io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture).expect("should always be possible to leave tui");
  
}

// TODO - move all the state types to a separate module

#[derive(Debug,Default)]
pub struct TrafficTabState {
    pub test : String,
    pub vertical_scroll_state: ScrollbarState,
    pub horizontal_scroll_state: ScrollbarState,
    pub vertical_scroll: usize,
    pub horizontal_scroll: usize,    
    pub total_rows : usize,
    pub visible_rows : usize,
    pub area_height : usize
}

#[derive(Clone,Debug,Eq,PartialEq)]
pub enum Page {
    Logs,
    Statistics,
    Connections
}

fn draw_traffic(
    f: &mut ratatui::Frame,
    app_state: &mut RwLockWriteGuard<'_, AppState>,
    area: Rect,
    theme: &Theme
) {
    let headers = [ "Site", "Source", "Target", "Description"];
    
    let rows : Vec<Vec<String>> = app_state.statistics.read().expect("must be able to read stats").active_connections.iter().map(|((src,_id),target)| {
        let typ = match &target.connection_type {
            ProxyActiveConnectionType::TcpTunnelUnencryptedHttp => "UNENCRYPTED TCP TUNNEL".to_string(),
            ProxyActiveConnectionType::TcpTunnelTls => 
                "TLS ENCRYPTED TCP TUNNEL".to_string(),
            ProxyActiveConnectionType::TerminatingHttp { incoming_scheme, incoming_http_version, outgoing_scheme, outgoing_http_version }=> 
                format!("{incoming_scheme}@{incoming_http_version:?} <-TERMINATING_HTTP-> {outgoing_scheme}@{outgoing_http_version:?}"),
            ProxyActiveConnectionType::TerminatingWs { incoming_scheme, incoming_http_version, outgoing_scheme, outgoing_http_version } => 
                format!("{incoming_scheme}@{incoming_http_version:?} <-TERMINATING_WS-> {outgoing_scheme}@{outgoing_http_version:?}"),
        };
        let description = format!("{}",typ);
        vec![
            target.target.host_name.clone(),
            src.to_string(), 
            target.target_addr.clone(), 
            description
        ]
    }).collect();

    
    let state = &mut app_state.traffic_tab_state;

    let header_height = 1;
    let visible_rows = area.height as usize - header_height;

    let start = state.vertical_scroll;
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
    

    state.visible_rows = display_rows.iter().len() as usize;
    state.total_rows = rows.len();

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
    state.area_height = height_of_traf_area as usize;
    
    state.vertical_scroll_state = state.vertical_scroll_state.content_length(rows.len().saturating_sub(height_of_traf_area as usize));
    
    let scrollbar_area = Rect::new(area.right() - 1, area.top(), 1, area.height);

    f.render_stateful_widget(scrollbar,scrollbar_area, &mut state.vertical_scroll_state);

}



fn draw_stats(
    f: &mut ratatui::Frame, 
    app_state: &mut RwLockWriteGuard<'_, AppState>,
    area: Rect,
    _theme: &Theme
) {

    let total_received_tcp_connections = {
        let guard = app_state.statistics.read().expect("must always be able to read statistics");
        guard.received_tcp_connections
    };

    let p = Paragraph::new(format!("Total received TCP connections: {total_received_tcp_connections}"));
    let p2 = Paragraph::new(format!("..More to come on this page at some point! :D")).fg(Color::DarkGray);
    
    f.render_widget(p, area.offset(Offset{x:4,y:2}));
    f.render_widget(p2, area.offset(Offset{x:4,y:4}));
}

fn draw_logs(
    f: &mut ratatui::Frame, 
    app_state: &mut RwLockWriteGuard<'_, AppState>,
    log_buffer: &Arc<Mutex<SharedLogBuffer>>,
    area: Rect,
    _theme: &Theme
) {

    {
        let mut buffer = log_buffer.lock().expect("locking shared buffer mutex should always work");

        if app_state.vertical_scroll.is_none() && buffer.limit.is_none() {
            let l = buffer.limit.borrow_mut();
            *l = Some(500);
        } else if app_state.vertical_scroll.is_some() && buffer.limit.is_some() {
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
        
        // todo - theme

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

        // todo: theme

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
    app_state.total_line_count = wrapped_line_count;
    
    let height_of_logs_area = area.height.saturating_sub(0); // header and footer
    app_state.logs_area_height = height_of_logs_area as usize;
    app_state.logs_area_width = area.width as usize;
    
    let scroll_pos = { app_state.vertical_scroll };

    let scrollbar_hovered = app_state.logs_scroll_bar_hovered;
    let mut scrollbar_state = app_state.scroll_state.borrow_mut();
   
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



/// Returns a `Style` configured for a dark theme.
fn dark_theme() -> Style {
    Style::default()
        .fg(Color::White) // Text color
        //.bg(Color::Black) // Background color
        .add_modifier(Modifier::BOLD) // Text modifier
}

/// Returns a `Style` configured for a light theme.
fn light_theme() -> Style {
    Style::default()
        .fg(Color::Black) // Text color
       // .bg(Color::White) // Background color
        .add_modifier(Modifier::ITALIC) // Text modifier
}


#[derive(Clone)]
pub enum Theme {
    Light(Style),
    Dark(Style)
}

fn draw_ui<B: ratatui::backend::Backend>(f: &mut ratatui::Frame, app_state: &mut RwLockWriteGuard<'_, AppState>,log_buffer: &Arc<Mutex<SharedLogBuffer>>, theme: &Theme) {
    
    let is_dark_theme = matches!(&theme,Theme::Dark(_));
    let theme_style = match theme {
        Theme::Light(s) => s,
        Theme::Dark(s) => s
    };


    let size = f.size();
    if size.height < 10 || size.width < 10 {
        return
    }

    let help_bar_height = 3 as u16;



    let constraints = if app_state.show_apps_window {
        vec![
            Constraint::Percentage(70), // MAIN SECTION
            Constraint::Min(0),  // SITES SECTION
            Constraint::Length(help_bar_height), // QUICK BAR
        ]
    } else {
        vec![
            Constraint::Min(1),  // MAIN SECTION
            Constraint::Max(0),
            Constraint::Length(help_bar_height),  // QUICK BAR
        ]
    };

    let vertical = Layout::vertical(constraints);
    let [top_area, mid_area, bot_area] = vertical.areas(size);

    //et x = format!("Logs {:?}",app_state.vertical_scroll);
    
    let main_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0) 
        ])
        .split(top_area.clone()); 

    // let totrows = app_state.traffic_tab_state.total_rows;
    // let traheight = size.height;
    
    let tabs =  ratatui::widgets::Tabs::new(
        vec![
            "[1] Logs", 
            "[2] Connections",
            "[3] Stats",
        ]).highlight_style(
            if is_dark_theme {
                Style::new().fg(Color::Cyan)
            } else {
                Style::new().fg(Color::LightRed)
            }
        )
        .select(match app_state.current_page {
            Page::Logs => 0,
            Page::Connections => 1,
            Page::Statistics => 2,
        }) 
        
        .divider(ratatui::text::Span::raw("|"));

    let frame_margin = &Margin { horizontal: 1, vertical: 1 };

    match &app_state.current_page {
        Page::Logs => draw_logs(f,app_state,log_buffer,main_area[0].inner(frame_margin),&theme),
        Page::Statistics => draw_stats(f,app_state,main_area[0].inner(frame_margin),&theme),
        Page::Connections => draw_traffic(f,app_state,main_area[0].inner(frame_margin),&theme),
    }

    let frame = 
        Block::new()
        .border_style(
            if matches!(&theme,Theme::Dark(_)) {
                Style::new().fg(Color::DarkGray)
            } else {
                Style::new().fg(Color::DarkGray)
            }
            
        )
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL);

    f.render_widget(frame, main_area[0]);
    

    // render the tab bar on top of the tab content
    f.render_widget(tabs, main_area[0].inner(&Margin { horizontal: 2, vertical: 0 }));


    if app_state.show_apps_window {

        let sites_area_height = mid_area.height.saturating_sub(2);
        if sites_area_height == 0 {
            return
        }
        let sites_count = app_state.procs.len() as u16;
        let columns_needed = ((sites_count as f32 / sites_area_height as f32).ceil()).max(1.0) as usize;

        let site_columns = Layout::default()
            .direction(Direction::Horizontal)
            .flex(ratatui::layout::Flex::Legacy)
            .constraints(vec![Constraint::Percentage(100 / columns_needed as u16); columns_needed])
            .split(mid_area);

        let mut site_rects = vec![];

        for (col_idx, col) in site_columns.iter().enumerate() {
            
            let mut procly : Vec<(&String, &ProcState)> = app_state.procs.iter().collect();
            procly.sort_by_key(|k| k.0);
            
            let start_idx = col_idx * sites_area_height as usize;
            let end_idx = ((col_idx + 1) * sites_area_height as usize).min(app_state.procs.len());
            let items: Vec<ListItem> = procly[start_idx..end_idx].iter().enumerate().map(|(index,(id, state))| {
                
                let item_rect = ratatui::layout::Rect {
                    x: col.x,
                    y: col.y + index as u16 + 1,  // Assuming each ListItem is one line high
                    width: col.width,
                    height: 1,  // Assuming each ListItem is one line high
                };

                site_rects.push((item_rect,id.to_string()));

                let mut s = match state {
                    &ProcState::Running => Style::default().fg(
                        if is_dark_theme {
                            Color::LightGreen
                        } else {
                            Color::Green
                        }
                    ),
                    &ProcState::Faulty => Style::default().fg(
                        if is_dark_theme {
                                Color::Red
                            } else {
                                Color::Red
                            }
                    ),
                    &ProcState::Starting => Style::default().fg(
                        if is_dark_theme {
                                Color::Green
                            } else {
                                Color::Green
                            }
                    ),
                    &ProcState::Stopped => Style::default().fg(
                        if is_dark_theme{
                                Color::DarkGray
                            } else {
                                Color::DarkGray
                            }
                    ),
                    &ProcState::Stopping => Style::default().fg(
                        if is_dark_theme {
                                Color::Black
                            } else {
                                Color::Yellow
                            }
                    ),
                    &ProcState::Remote => Style::default().fg(
                        if is_dark_theme {
                                Color::Blue
                            } else {
                                Color::Blue
                            }
                    ),
                };

                let mut id_style = theme_style.clone();
                if let Some(hovered) = &app_state.currently_hovered_site && hovered == *id {
                    id_style = id_style.add_modifier(Modifier::BOLD);
                    s = if is_dark_theme { s.bg(Color::Gray) } else { s.bg(Color::Gray)  };
                }
                
                let message = ratatui::text::Span::styled(format!(" {id} "),id_style);

                let status = ratatui::text::Span::styled(format!("{:?}",state),s);
        
                ListItem::new(Line::from(vec![
                    message,
                    status
                ]))
                
            }).collect();

            let sites_list = List::new(items)
                .block( 
                    Block::new()
                        .border_style(Style::default().fg(Color::DarkGray))
                        .border_type(BorderType::Rounded)
                        .borders(Borders::ALL)
                        .title(" Sites ").title_alignment(Alignment::Left)
                        .title_style(
                            if is_dark_theme {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default().fg(Color::Blue)
                            }
                        )
                )
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">> ");
            
            f.render_widget(sites_list, *col);
        }
        app_state.site_rects = site_rects;
    }

    
      
    let help_bar_chunk = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3) 
        ])
        .split(bot_area.clone()); 


    
    let mut help_bar_text = vec![
        ratatui::text::Span::raw("q: Quit | "),
        ratatui::text::Span::raw("a: Toggle Sites | "),
    ];


    help_bar_text.push(ratatui::text::Span::raw("s: Start all | "));
    help_bar_text.push(ratatui::text::Span::raw("z: Stop all | "));

    help_bar_text.push(ratatui::text::Span::raw("↑/↓: Scroll | "));
    help_bar_text.push(ratatui::text::Span::raw("PgUp/PgDn Scroll "));


    if Page::Logs == app_state.current_page {
        help_bar_text.push(ratatui::text::Span::raw("c: Clear | "));
        help_bar_text.push(ratatui::text::Span::raw("tab: toggle page "));
        if app_state.vertical_scroll.is_some() {
            help_bar_text.push(ratatui::text::Span::raw("| enter: Tail log "));
        }
    } else {
        help_bar_text.push(ratatui::text::Span::raw("| tab: toggle page"));
    }


    // // DEBUG
    // help_bar_text.push(ratatui::text::Span::raw(format!("| DBG: {}", 
    //     app_state.dbg
    // )));
    let current_version = self_update::cargo_crate_version!();
    let help_bar = Paragraph::new(Line::from(help_bar_text))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" ODD-BOX v{current_version}")).title_style(
            if is_dark_theme {
                Style::default().fg(Color::LightYellow)
            } else {
                Style::default().fg(Color::Black)
            }
        ));

    f.render_widget(help_bar, help_bar_chunk[1]);


}

fn wrap_string(input: &str, max_length: usize) -> Vec<String> {

    let words = input.split_whitespace();
    let mut wrapped_lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        // Check if adding the next word exceeds the max_length
        if current_line.len() + word.len() + 1 > max_length {
            // Add the current line to the vector and start a new line
            wrapped_lines.push(current_line);
            current_line = String::new();
        }

        // If the line is not empty, add a space before the next word
        if !current_line.is_empty() {
            current_line.push(' ');
        }

        // Add the word to the current line
        current_line.push_str(word);
    }

    // Add the last line if it's not empty
    if !current_line.is_empty() {
        wrapped_lines.push(current_line);
    }

    wrapped_lines
}