use ratatui::layout::{Margin, Alignment};
use ratatui::style::{Style, Color, Modifier};
use ratatui::text::Line;
use ratatui::widgets::{ListItem, List, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState, BorderType};
use tokio::sync::MutexGuard;
use tokio::task;
use tracing::{Level, Subscriber};
use tracing_subscriber::{Layer, EnvFilter};
use tracing_subscriber::layer::{Context, SubscriberExt};
use std::borrow::BorrowMut;
use std::collections::{HashMap, VecDeque};
use std::io::Stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::ProcState;

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
    logs: VecDeque<LogMsg>,
    limit : Option<usize>
}

impl SharedLogBuffer {
    
    fn new() -> Self {
        SharedLogBuffer {
            logs: VecDeque::new(),
            limit: Some(1000)
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

    fn get_logs(&self) -> Vec<LogMsg> {
        self.logs.iter().cloned().collect()
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

        let current_thread = std::thread::current();

        let log_message = LogMsg {
            thread: if let Some(n) = current_thread.name() {Some(n.to_owned())}else{None},
            lvl: metadata.level().clone(),
            src,
            msg
        };

        let mut buffer = self.log_buffer.lock().unwrap();
        buffer.push(log_message);
    
    }
}


pub (crate) fn init() {
    _ = enable_raw_mode().unwrap();
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).unwrap();
}

pub (crate) async fn run(
    filter:EnvFilter,
    shared_state:Arc<tokio::sync::Mutex<AppState>>,
    tx: tokio::sync::broadcast::Sender<(String, bool)>
) {
    
    let log_buffer = Arc::new(Mutex::new(SharedLogBuffer::new()));
    let layer = TuiLoggerLayer { log_buffer: log_buffer.clone() };

    let subscriber = tracing_subscriber::registry().with(filter).with(layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set collector");
  

    let backend = CrosstermBackend::new(std::io::stdout());
    
    let terminal = Terminal::new(backend).unwrap();
    
    let terminal = Arc::new(tokio::sync::Mutex::new(terminal));
    
    // TUI event loop
    let tui_handle = {
        let terminal = Arc::clone(&terminal);
        let app_state = Arc::clone(&shared_state);
        let tx = tx.clone();
        task::spawn(async move {
            
            let tx = tx.clone();
            
            let mut last_key_time = tokio::time::Instant::now();
            let debounce_duration = Duration::from_millis(100);
            
            let toggle_debounce: Duration = Duration::from_millis(250);
            let mut last_toggle : Option<tokio::time::Instant> = None;

            loop {

                {
                    let app = app_state.lock().await;

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

                // let view_mode = {
                //     app_state.lock().await.view_mode.clone()
                // };

                // match view_mode {
                //     ViewMode::Console => {
                //         let mut state = app_state.lock().await;
                        
                //         let mut terminal = terminal.lock().await;
                       
                //         terminal.draw(|f| draw_ui::<CrosstermBackend<Stdout>>(f, &mut state, &log_buffer))?;
                //     },
                //     ViewMode::TUI => 

                    // KEEP LOCK SHORT TO AVOID DEADLOCK
                    {
                        let mut state = app_state.lock().await;
                        
                        let mut terminal = terminal.lock().await;
                        terminal.draw(|f| draw_ui::<CrosstermBackend<Stdout>>(f, &mut state, &log_buffer))?;
                            
                    }

                // }

                
                // Handle input
                if event::poll(std::time::Duration::from_millis(100))? {
                    let now = tokio::time::Instant::now();
                    let time_since_last_keypress = now.duration_since(last_key_time);
                    let time_since_last_toggle = if let Some(t) = last_toggle {
                        Some(now.duration_since(t))
                    } else {
                        None
                    };
                    match event::read()? {
                        Event::Mouse(mouse) => {
                            if time_since_last_keypress >= debounce_duration {
                                match mouse.kind {
                                    event::MouseEventKind::ScrollDown => {
                                        let mut app = app_state.lock().await;
                                        app.scroll_down(Some(10));
                                    },
                                    event::MouseEventKind::ScrollUp => {
                                        let mut app = app_state.lock().await;
                                        app.scroll_up(Some(10));
                                    },
                                    _ => {}
                                }
                            }
                        }
                        Event::Key(key) => {
                            if time_since_last_keypress >= debounce_duration { 
                            
                                match key.code {
                                    // todo
                                    // KeyCode::Char('x') => {
                                    //     let mut app = app_state.lock().await;
                                    //     app.toggle_view()
                                    // },
                                    KeyCode::Up => {
                                        let mut app = app_state.lock().await;
                                        app.scroll_up(None);
                                    }
                                    KeyCode::Down => {
                                        let mut app = app_state.lock().await;
                                        app.scroll_down(None);
                                    }
                                    KeyCode::PageUp => {
                                        let mut app = app_state.lock().await;
                                        let scroll_count = app.logs_area_height.saturating_div(2);
                                        if scroll_count > 0 {
                                            app.scroll_up(Some(scroll_count));
                                        }
                                    }
                                    KeyCode::PageDown => {
                                        let mut app = app_state.lock().await;
                                        let scroll_count = app.logs_area_height.saturating_div(2);
                                        if scroll_count > 0 {
                                            app.scroll_down(Some(scroll_count));
                                        }
                                    }                                    
                                    KeyCode::Char('z') => {
                                        tx.clone().send(("all".to_owned(),false)).unwrap();
                                    } 
                                    KeyCode::Char('s') => {
                                        tx.clone().send(("all".to_owned(),true)).unwrap();
                                    }    
                                    KeyCode::Char('a') => {
                                        if let Some(t) = time_since_last_toggle{
                                            if t < toggle_debounce {
                                                continue
                                            }
                                        }
                                        let mut app = app_state.lock().await;
                                        app.show_apps_window = !app.show_apps_window;
                                        last_toggle = Some(now)
                                    }  
                                    KeyCode::Enter => {
                                        let mut app = app_state.lock().await;
                                        app.vertical_scroll = None;
                                    }    
                                    KeyCode::Char('l') => {
                                        let mut app = app_state.lock().await;
                                        let mut buf = log_buffer.lock().unwrap();
                                        app.line_count = 0;
                                        app.vertical_scroll = None;
                                        app.scroll_state.position(0);
                                        buf.logs.clear();
                                        
                                    }                                      
                                    KeyCode::Esc | KeyCode::Char('q')=> {
                                        tx.clone().send(("all".to_owned(),false)).unwrap();
                                        let mut app = app_state.lock().await;
                                        app.exit = true;
                                    }
                                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                                        tx.clone().send(("all".to_owned(),false)).unwrap();
                                        let mut app = app_state.lock().await;
                                        app.exit = true;
                                    }
                                    _ => {
    
                                    }
                                }
                                last_key_time = now;
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
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture).unwrap();
  
}





pub (crate) struct AppState {
    line_count: usize,
    exit: bool,
    //view_mode: ViewMode,
    pub procs: HashMap<String,ProcState>,
    vertical_scroll: Option<usize>,
    scroll_state : ScrollbarState,
    show_apps_window : bool,
    logs_area_height:usize
}

impl AppState {
    pub fn new() -> AppState {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        AppState {
            line_count:0,
            logs_area_height: 5,
            scroll_state: ScrollbarState::new(0),
            vertical_scroll: None,
            exit: false,
            //view_mode: ViewMode::Console,
            procs: HashMap::<String,ProcState>::new(),
            show_apps_window : false
        }
    }
    pub fn scroll_down(&mut self, count:Option<usize>) {
        if self.vertical_scroll.is_some() {
            let current = self.vertical_scroll.unwrap_or_default();
            let max = self.line_count.saturating_sub(self.logs_area_height).saturating_sub(1);
            if current < max {
                let new_val = current.saturating_add(count.unwrap_or(1)).min(max);
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
            else {
                self.vertical_scroll = None;
            }
        }
    }
    pub fn scroll_up(&mut self, count:Option<usize>) {
        let msg_count = self.line_count;
        match self.vertical_scroll {
            Some(current) if current > 0 => {
                let new_val = current.saturating_sub(count.unwrap_or(1)).max(0);
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
            None => {
                let max = self.line_count.saturating_sub(self.logs_area_height);
                let new_val = max.saturating_sub(count.unwrap_or(1));
                self.vertical_scroll = Some(new_val);
                self.scroll_state = self.scroll_state.position(new_val);
            }
            _ => {}
        }
    }

    // fn toggle_view(&mut self) {
        
        
    //     self.view_mode = match self.view_mode {
    //         ViewMode::Console => {
    //             ViewMode::TUI
    //         },
    //         ViewMode::TUI => {
    //             ViewMode::Console
    //         },
    //     };
    // }
}

fn draw_ui<B: ratatui::backend::Backend>(f: &mut ratatui::Frame, app_state: &mut MutexGuard<'_, AppState>,log_buffer: &Arc<Mutex<SharedLogBuffer>>) {
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Block, Borders, Paragraph}
    };

    let size = f.size();
    if size.height < 10 || size.width < 10 {
        return
    }

    let help_bar_height = 3 as u16;
    
    let constraints = if app_state.show_apps_window {
        vec![
            Constraint::Percentage(70),
            Constraint::Percentage(30 - (help_bar_height * 100 / size.height)), 
            Constraint::Length(help_bar_height),
        ]
    } else {
        vec![
            Constraint::Min(0),
            Constraint::Length(help_bar_height),
        ]
    };
   
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(size);

    let mut buffer = log_buffer.lock().unwrap();
    let logs = buffer.get_logs();


    if app_state.vertical_scroll.is_none() && buffer.limit.is_none() {
        let l = buffer.limit.borrow_mut();
        *l = Some(1000);
    } else if app_state.vertical_scroll.is_some() && buffer.limit.is_some() {
        let l = buffer.limit.borrow_mut();
        *l = None;
    }

    let max_msg_width = chunks[0].inner(&Margin::default()).width;

    let item_count = logs.len().to_string().len().max(6);
    let items: Vec<Line> = logs.iter().enumerate().flat_map(|(i,x)|{
        
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

        if x.msg.len() > max_width as usize {
            
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
            
        }

        
    }).collect();

    let wrapped_line_count = items.len();
    app_state.line_count = wrapped_line_count;

    let area = chunks[0];
    
    let height_of_logs_area = area.height.saturating_sub(2); // header and footer
    app_state.logs_area_height = height_of_logs_area as usize;
    let scroll_pos = { app_state.vertical_scroll };


    let mut scrollbar_state = app_state.scroll_state.borrow_mut();
   
    let max = items.len().saturating_sub(height_of_logs_area as usize) ;
    let paragraph = Paragraph::new(items.clone())
        .scroll((scroll_pos.unwrap_or(max) as u16,0))
        .block(
                Block::new()
                .border_style(Style::default().fg(Color::DarkGray))
                .border_type(BorderType::Rounded)
                .borders(Borders::ALL)
            .title(" Logs ")
            .title_alignment(Alignment::Left)
            .title_style(Style::default().fg(Color::Cyan))
        );
        
    let scrollbar = Scrollbar::default()
        .style(Style::default().fg(Color::LightBlue))
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    *scrollbar_state = scrollbar_state.content_length(items.len().saturating_sub(height_of_logs_area as usize));

    if scroll_pos.is_none() {
        *scrollbar_state = scrollbar_state.position(items.len().saturating_sub(height_of_logs_area as usize));
    }

    f.render_widget(paragraph, area);
    f.render_stateful_widget(scrollbar,
        area.inner(&Margin {
            vertical: 1,
            horizontal: 0,
        }), 
       &mut scrollbar_state);

   
    
    if app_state.show_apps_window {

        let sites_area_height = chunks[1].height.saturating_sub(2);
        if sites_area_height == 0 {
            return
        }
        let sites_count = app_state.procs.len() as u16;
        let columns_needed = (sites_count as f32 / sites_area_height as f32).ceil() as usize;

        let site_columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(100 / columns_needed as u16); columns_needed])
            .split(chunks[1]);

        for (col_idx, col) in site_columns.iter().enumerate() {
            let procly : Vec<(&String, &ProcState)> = app_state.procs.iter().collect();
            let start_idx = col_idx * sites_area_height as usize;
            let end_idx = ((col_idx + 1) * sites_area_height as usize).min(app_state.procs.len());
            let items: Vec<ListItem> = procly[start_idx..end_idx].iter().map(|(id, state)| {
                

                let s = match state {
                    &ProcState::Running => Style::default().fg(Color::LightGreen),
                    &ProcState::Faulty => Style::default().fg(Color::Red),
                    &ProcState::Starting => Style::default().fg(Color::Green),
                    &ProcState::Stopped => Style::default().fg(Color::DarkGray),
                    &ProcState::Stopping => Style::default().fg(Color::Yellow)
                };
                
                let message = ratatui::text::Span::styled(format!(" {id} "),Style::default());

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
                        .title_style(Style::default().fg(Color::Cyan))
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">> ");

            f.render_widget(sites_list, *col);
        }
        
    }
      
    let help_bar_chunk = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3) 
        ])
        .split(chunks.last().unwrap().clone()); 


    
    let mut help_bar_text = vec![
        ratatui::text::Span::raw("q: Quit | "),
        ratatui::text::Span::raw("a: Toggle Sites | "),
    ];

    if app_state.procs.iter().all(|x|x.1==&ProcState::Stopped) {
        help_bar_text.push(ratatui::text::Span::raw("s: Start all sites | "))
    }
    else if  app_state.procs.iter().all(|x: (&String, &ProcState)|x.1==&ProcState::Running) {
        help_bar_text.push(ratatui::text::Span::raw("z: Stop all sites | "));
    }
    else {
        help_bar_text.push(ratatui::text::Span::raw("s: Start all sites | "));
        help_bar_text.push(ratatui::text::Span::raw("z: Stop all sites | "));
    }

    help_bar_text.push(ratatui::text::Span::raw("↑/↓: Scroll | "));
    help_bar_text.push(ratatui::text::Span::raw("PgUp/PgDn: Page Scroll "));

    if app_state.vertical_scroll.is_some() {
        help_bar_text.push(ratatui::text::Span::raw("| enter: Tail log "));
    }

    
    let help_bar = Paragraph::new(Line::from(help_bar_text))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" ODD-BOX ").title_style(Style::default().fg(Color::LightYellow)));

    f.render_widget(help_bar, help_bar_chunk[1]);


}

// #[derive(Clone)]
// enum ViewMode {
//     Console,
//     TUI,
// }

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