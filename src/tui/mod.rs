use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Margin};
use ratatui::style::{Color, Modifier, Style };
use ratatui::text::Line;
use ratatui::widgets::{BorderType, List, ListItem };
use tokio::task;
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use std::io::Stdout;
use crate::global_state::GlobalState;
use crate::logging::SharedLogBuffer;
use crate::logging::LogMsg;
use crate::types::app_state::*;
use crate::types::tui_state::{Page, TuiState};
use std::sync::{Arc, Mutex};
use crate::http_proxy::ProcMessage;

use serde::ser::SerializeStruct;

mod connections_widget;
mod logs_widget;
mod stats_widget;
mod threads_widget;
pub mod scroll_state_wrapper;

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

impl serde::Serialize for LogMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut s = serializer.serialize_struct("LogMsg", 4)?;
        s.serialize_field("msg", &self.msg)?;
        s.serialize_field("lvl", &self.lvl.as_str())?;
        s.serialize_field("src", &self.src)?;
        s.serialize_field("thread", &self.thread.as_ref().unwrap_or(&"".to_string()))?;
        s.end()

    }
}


pub fn init() {
    _ = enable_raw_mode().expect("must be able to enable raw mode");();
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).expect("must always be able to enter alt screen");
}

pub async fn run(
    global_state: Arc<GlobalState>,
    tx: tokio::sync::broadcast::Sender<ProcMessage>,
    trace_msg_broadcaster: tokio::sync::broadcast::Sender<String>,
    reloadable_filter : tracing_subscriber::reload::Layer<EnvFilter, tracing_subscriber::layer::Layered<crate::logging::TuiLoggerLayer, tracing_subscriber::Registry>>,
) {

    
    let log_buffer = Arc::new(Mutex::new(SharedLogBuffer::new()));
    let layer = crate::logging::TuiLoggerLayer { log_buffer: log_buffer.clone(), broadcaster: trace_msg_broadcaster };

    let subscriber = tracing_subscriber::registry()
       .with(layer).with(reloadable_filter);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set collector");
  

    let backend = CrosstermBackend::new(std::io::stdout());
    
    let terminal = Terminal::new(backend).expect("must be possible to create terminal");
    
    let terminal = Arc::new(tokio::sync::Mutex::new(terminal));
    

    let mut manually_selected_theme : Option<dark_light::Mode> = None;
    
    let dark_style = dark_theme();
    let light_style = light_theme();

    let mut theme = match dark_light::detect() {
        dark_light::Mode::Dark => Theme::Dark(dark_style),
        dark_light::Mode::Light => Theme::Light(light_style),
        dark_light::Mode::Default => Theme::Dark(dark_style),
    };

    let mut count = 0;

    let disabled_items : Vec<String> =  global_state.config.read().await.hosted_process.clone().unwrap_or_default().iter_mut().filter_map( |x| 
      if x.auto_start.unwrap_or_default() { 
        Some(x.host_name.clone()) 
      } else {
        None
      }
    ).collect();

    
    // TUI event loop
    let tui_handle = {
        let terminal = Arc::clone(&terminal);
        let state = global_state.clone();
        let tx = tx.clone();
        task::spawn(async move {
            
            let tx = tx.clone();
           
            let mut tui_state = crate::types::tui_state::TuiState::new();

            loop {
                
                // KEEP LOCK SHORT TO AVOID DEADLOCK
                {
                    let mut terminal = terminal.lock().await;
                    terminal.draw(|f| 
                        draw_ui::<CrosstermBackend<Stdout>>(
                            f,
                            global_state.clone(),
                            &mut tui_state,
                            &log_buffer,&theme
                        )
                    )?;
                        
                }
                
            
                if global_state.app_state.exit.load(std::sync::atomic::Ordering::SeqCst) == true {
                    if global_state.app_state.site_status_map.iter().find(|x|
                            x.value() == &ProcState::Stopping 
                        || x.value() == &ProcState::Running
                        || x.value() == &ProcState::Starting 
                        
                    ).is_none() {
                        break; // nothing is running,stopping or starting.. we can exit now
                    }
                }
            

                if count > 100 {
                    theme = match dark_light::detect() {
                        dark_light::Mode::Dark => Theme::Dark(dark_style),
                        dark_light::Mode::Light => Theme::Light(light_style),
                        dark_light::Mode::Default => Theme::Dark(dark_style),
                    };
                    count = 0;
                
                }
            
                if let Ok(true) = event::poll(std::time::Duration::from_millis(100)) {
                    
                
                    let (current_page,sites_open) = {
                        (tui_state.current_page.clone(),tui_state.show_apps_window)
                    };
                    
                    let evt = event::read()?;
                
                    match evt {
                        Event::Key(KeyEvent { 
                            code: crossterm::event::KeyCode::Char(' '),
                            modifiers: KeyModifiers::NONE,
                            kind: _, 
                            state:_ 
                        }) if tui_state.current_page==Page::Logs => {
                            
                            let mut buf = log_buffer.lock().expect("must always be able to lock log buffer");
                            buf.pause = !buf.pause;
                            let paused = buf.pause;
                            buf.logs.push_back(LogMsg {
                                msg: if paused { 
                                    format!("LOGGING PAUSED! PRESS SPACE TO RESUME.") 
                                } else {
                                    format!("LOGGING RESUMED! PRESS SPACE TO PAUSE.") 
                                },
                                lvl: Level::WARN,
                                src: String::from("odd-box tracing"),
                                thread: None,
                            });
                        }
                        Event::Key(KeyEvent { 
                            code: crossterm::event::KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            kind: _, 
                            state:_ 
                        }) => {
                            state.app_state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                            break;
                        },
                        Event::Mouse(mouse) => {
                                
                                if sites_open {
                                    match mouse.kind {
                                        event::MouseEventKind::Moved => {
                                            tui_state.sites_handle_mouse_hover(mouse.column,mouse.row);
                                        }
                                        event::MouseEventKind::Down(event::MouseButton::Left) => {
                                            tui_state.sites_handle_mouse_click(mouse.column,mouse.row,tx.clone(), &state.app_state.site_status_map)
                                        }
                                        _ => {}
                                    }
                                }
                                match current_page {
                                    Page::Statistics => {
                                        
                                    }
                                    Page::Logs => {
                                        match mouse.kind {                                            
                                            event::MouseEventKind::Drag(event::MouseButton::Left) => {                                                
                                                tui_state.log_tab_stage.scroll_state.handle_mouse_drag(mouse.column,mouse.row);
                                            }
                                            event::MouseEventKind::Moved => {
                                                tui_state.log_tab_stage.scroll_state.handle_mouse_move(mouse.column,mouse.row);
                                            }

                                            event::MouseEventKind::ScrollDown => {
                                                tui_state.log_tab_stage.scroll_state.scroll_down(Some(10));
                                            },
                                            event::MouseEventKind::ScrollUp => {
                                                tui_state.log_tab_stage.scroll_state.scroll_up(Some(10));
                                            },
                                            _ => {}
                                        }
                                    },
                                    Page::Threads => {
                                        match mouse.kind {
                                            event::MouseEventKind::Drag(event::MouseButton::Left) => {
                                                tui_state.threads_tab_state.scroll_state.handle_mouse_drag(mouse.column,mouse.row);
                                            }
                                            event::MouseEventKind::Moved => {
                                                tui_state.threads_tab_state.scroll_state.handle_mouse_move(mouse.column,mouse.row);
                                            }
                                            event::MouseEventKind::ScrollDown => {
                                                tui_state.threads_tab_state.scroll_state.scroll_down(Some(10));
                                            },
                                            event::MouseEventKind::ScrollUp => {
                                                tui_state.threads_tab_state.scroll_state.scroll_up(Some(10));
                                            },
                                            _ => {}
                                        }
                                    },
                                    Page::Connections => {
                                        match mouse.kind {
                                            event::MouseEventKind::Drag(event::MouseButton::Left) => {
                                                tui_state.connections_tab_state.scroll_state.handle_mouse_drag(mouse.column,mouse.row);
                                            }
                                            event::MouseEventKind::Moved => {
                                                tui_state.connections_tab_state.scroll_state.handle_mouse_move(mouse.column,mouse.row);
                                            }
                                            event::MouseEventKind::ScrollDown => {
                                                tui_state.connections_tab_state.scroll_state.scroll_down(Some(10));
                                            },
                                            event::MouseEventKind::ScrollUp => {
                                                tui_state.connections_tab_state.scroll_state.scroll_up(Some(10));
                                            },
                                            _ => {}
                                        }
                                    },
                                }
                                
                        }
                        Event::Key(key) => {

                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q')  => {
                                        {
                                            tracing::warn!("User requested exit");
                                            state.app_state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                                            break;
                                        }
                                        
                                    },
                                    KeyCode::Char('t') => {
                                        match manually_selected_theme {
                                            None => {
                                                // when switching from auto to manual theme, we will switch to the opposite of the current theme
                                                match theme {
                                                    Theme::Light(_) => {
                                                        manually_selected_theme = Some(dark_light::Mode::Dark);
                                                        theme = Theme::Dark(dark_style);
                                                    },
                                                    Theme::Dark(_) => {
                                                        manually_selected_theme = Some(dark_light::Mode::Light);
                                                        theme = Theme::Light(light_style);
                                                    },
                                                }
                                            } 
                                            Some(dark_light::Mode::Dark) => {
                                                manually_selected_theme = Some(dark_light::Mode::Light);
                                                theme = Theme::Light(light_style);
                                            },
                                            Some(dark_light::Mode::Light) => {
                                                manually_selected_theme = Some(dark_light::Mode::Dark);
                                                theme = Theme::Dark(dark_style);
                                            },                                                                                        
                                            _ => {},
                                        };
                                    },
                                    _ => {}
                                }
                                match current_page {
                                    Page::Threads => {
                                        match key.code {
                                            KeyCode::PageUp => {
                                                tui_state.threads_tab_state.scroll_state.scroll_up(Some(10));
                                            }
                                            KeyCode::PageDown => {
                                                tui_state.threads_tab_state.scroll_state.scroll_down(Some(10))
                                                
                                            },
                                            KeyCode::Up => {
                                                tui_state.threads_tab_state.scroll_state.scroll_up(None)
                                            }
                                            KeyCode::Down => {
                                                tui_state.threads_tab_state.scroll_state.scroll_down(None)
                                            }
                                            _ => {}
                                        }
                                    },
                                    Page::Logs => {
                                        match key.code {
                                            KeyCode::Enter => {
                                                tui_state.log_tab_stage.scroll_state.vertical_scroll = None;
                                                let mut buf = log_buffer.lock().expect("must always be able to lock log buffer");
                                                // immediate effect instead of waiting for next log item
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
                                                tui_state.log_tab_stage.scroll_state.scroll_up(Some(1));
                                            }
                                            KeyCode::Down => {
                                                tui_state.log_tab_stage.scroll_state.scroll_down(Some(1));
                                            }
                                            KeyCode::PageUp => {
                                                let scroll_count = tui_state.log_tab_stage.scroll_state.area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    tui_state.log_tab_stage.scroll_state.scroll_up(Some(scroll_count));
                                                }
                                            }
                                            KeyCode::PageDown => {
                                                let scroll_count = tui_state.log_tab_stage.scroll_state.area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    tui_state.log_tab_stage.scroll_state.scroll_down(Some(scroll_count));
                                                }
                                            },   
                                            KeyCode::Char('c') => {
                                                let mut buf = log_buffer.lock().expect("must always be able to lock log buffer");
                                                tui_state.log_tab_stage.scroll_state.total_rows = 0;
                                                tui_state.log_tab_stage.scroll_state.vertical_scroll = None;
                                                tui_state.log_tab_stage.scroll_state.vertical_scroll_state = tui_state.log_tab_stage.scroll_state.vertical_scroll_state.position(0);
                                                buf.logs.clear();
                                                
                                            }    
                                            _ => {}        
                                        }
                                    }
                                    Page::Statistics => {},
                                    Page::Connections => {
                                        match key.code {
                                            KeyCode::PageUp => {
                                                tui_state.connections_tab_state.scroll_state.scroll_up(Some(10));
                                            }
                                            KeyCode::PageDown => {
                                                tui_state.connections_tab_state.scroll_state.scroll_down(Some(10));
                                                
                                            },
                                            KeyCode::Up => {
                                                let scroll_count = tui_state.connections_tab_state.scroll_state.area_height.saturating_div(2);
                                                if scroll_count > 0 {
                                                    tui_state.connections_tab_state.scroll_state.scroll_up(None);
                                                }
                                            }
                                            KeyCode::Down => {
                                                tui_state.connections_tab_state.scroll_state.scroll_down(None);
                                            }
                                            _ => {}
                                        }
                                    },
                                }

                                match key.code {    
                                
                                    KeyCode::Char('1') => {
                                        tui_state.current_page = Page::Logs;
                                    }
                                    KeyCode::Char('2') => {
                                        tui_state.current_page = Page::Connections;
                                    }
                                    KeyCode::Char('3') => {
                                        tui_state.current_page = Page::Statistics;
                                    }
                                    KeyCode::Char('4') => {
                                        tui_state.current_page = Page::Threads;
                                    }
                                    KeyCode::BackTab | KeyCode::Left => {
                                        match tui_state.current_page {
                                            Page::Logs => tui_state.current_page = Page::Threads, 
                                            Page::Threads => tui_state.current_page = Page::Statistics, 
                                            Page::Statistics => tui_state.current_page = Page::Connections,
                                            Page::Connections => tui_state.current_page = Page::Logs
                                        }
                                    }
                                    KeyCode::Tab | KeyCode::Right => {
                                        match tui_state.current_page {
                                            Page::Logs => tui_state.current_page = Page::Connections, 
                                            Page::Connections => tui_state.current_page = Page::Statistics, 
                                            Page::Statistics => tui_state.current_page = Page::Threads,
                                            Page::Threads => tui_state.current_page = Page::Logs
                                        }
                                    }          
                                    KeyCode::Char('z') => {
                                        {
                                            for mut guard in global_state.app_state.site_status_map.iter_mut() {
                                                let (_k,state) = guard.pair_mut();
                                                match state {
                                                    ProcState::Faulty =>  *state = ProcState::Stopping,
                                                    ProcState::Running =>  *state = ProcState::Stopping,
                                                    _ => {}
                                                }
                                            }
                                        }
                                        tx.clone().send(ProcMessage::StopAll).expect("must always be able to send internal messages");
                                    } 
                                    KeyCode::Char('s') => {
                                        {
                                            for mut guard in global_state.app_state.site_status_map.iter_mut() {
                                                let (_k,state) = guard.pair_mut();
                                                if disabled_items.contains(_k) {
                                                    continue;
                                                }
                                                if let ProcState::Running = state {
                                                    *state = ProcState::Starting;
                                                }
                                            }
                                        }
                                        tx.clone().send(ProcMessage::StartAll).expect("must always be able to send internal messages");
                                    }    
                                    KeyCode::Char('a') => {
                                        tui_state.show_apps_window = !tui_state.show_apps_window;
                                    }                                  
                                    
                                    _ => {
    
                                    }
                            }
                            

                            // }


                            
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
        //.bg(Color::White) // Background color
        .add_modifier(Modifier::ITALIC) // Text modifier
}

#[derive(Clone)]
pub enum Theme {
    Light(Style),
    Dark(Style)
}

fn draw_ui<B: ratatui::backend::Backend>(
    f: &mut ratatui::Frame, 
    global_state: Arc<GlobalState>,
    tui_state: &mut TuiState,
    log_buffer: &Arc<Mutex<SharedLogBuffer>>, 
    theme: &Theme
) {

    
    let is_dark_theme = matches!(&theme,Theme::Dark(_));
    let theme_style = match theme {
        Theme::Light(s) => s,
        Theme::Dark(s) => s
    };


    let size = f.area();

    if size.height < 10 || size.width < 10 {
        return
    }

    let help_bar_height = 3 as u16;

    let constraints = if tui_state.show_apps_window {
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
            "[4] Threads"
        ]).highlight_style(
            if is_dark_theme {
                Style::new().fg(Color::Cyan)
            } else {
                Style::new().fg(Color::LightRed)
            }
        )
        .select(match tui_state.current_page {
            Page::Logs => 0,
            Page::Connections => 1,
            Page::Statistics => 2,
            Page::Threads => 3
        }) 
        
        .divider(ratatui::text::Span::raw("|"));

    let frame_margin = Margin { horizontal: 1, vertical: 1 };

    match tui_state.current_page {
        Page::Logs => logs_widget::draw(f,global_state.clone(),tui_state,log_buffer,main_area[0].inner(frame_margin),&theme),
        Page::Statistics => stats_widget::draw(f,global_state.clone(),tui_state,main_area[0].inner(frame_margin),&theme),
        Page::Connections => connections_widget::draw(f,global_state.clone(),tui_state,main_area[0].inner(frame_margin),&theme),
        Page::Threads => threads_widget::draw(f,global_state.clone(),tui_state,main_area[0].inner(frame_margin),&theme)
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
    f.render_widget(tabs, main_area[0].inner(Margin { horizontal: 2, vertical: 0 }));

    if tui_state.show_apps_window {

        let sites_area_height = mid_area.height.saturating_sub(2);
        if sites_area_height == 0 {
            return
        }
        let sites_count = global_state.app_state.site_status_map.iter().count();
        let columns_needed = ((sites_count as f32 / sites_area_height as f32).ceil()).max(1.0) as usize;

        let site_columns = Layout::default()
            .direction(Direction::Horizontal)
            .flex(ratatui::layout::Flex::Legacy)
            .constraints(vec![Constraint::Percentage(100 / columns_needed as u16); columns_needed])
            .split(mid_area);

        let mut site_rects = vec![];

        for (col_idx, col) in site_columns.iter().enumerate() {
            
            let mut procly : Vec<(String, ProcState)> = global_state.app_state.site_status_map.iter()
                .map(|x|{
                    let (a,b) = x.pair();
                    (a.to_string(),b.to_owned())
                }).collect();

            // todo -- no clone
            procly.sort_by_key(|k| k.0.clone());
            
            let start_idx = col_idx * sites_area_height as usize;
            let end_idx = ((col_idx + 1) * sites_area_height as usize).min(sites_count);
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
                                Color::Rgb(200, 150, 150)
                            } else {
                                Color::Rgb(200, 150, 150)
                            }
                    ),
                    &ProcState::Starting => Style::default().fg(
                        if is_dark_theme {
                                Color::LightGreen
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
                                Color::Cyan
                            } else {
                                Color::Black
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
                if let Some(hovered) = &tui_state.currently_hovered_site {
                    if hovered == id {
                        id_style = id_style.add_modifier(Modifier::BOLD);
                        s = if is_dark_theme { s.bg(Color::Gray) } else { s.bg(Color::Gray) };
                    }
                }
            
                
                let message = ratatui::text::Span::styled(format!(" {id} "),id_style);

                let status = match state {
                    &ProcState::Running => ratatui::text::Span::styled(format!("{:?}",state),s),
                    &ProcState::Faulty => ratatui::text::Span::styled(format!("{:?} (retrying in 5s)",state),s),
                    &ProcState::Starting => ratatui::text::Span::styled(format!("{:?}",state),s),
                    &ProcState::Stopped => ratatui::text::Span::styled(format!("{:?}",state),s),
                    &ProcState::Stopping => ratatui::text::Span::styled(format!("{:?}..",state),s),
                    &ProcState::Remote => ratatui::text::Span::styled(format!("{:?}",state),s)
                };
        
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

        tui_state.site_rects = site_rects;
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


    if Page::Logs == tui_state.current_page {
        help_bar_text.push(ratatui::text::Span::raw("c: Clear | "));
        help_bar_text.push(ratatui::text::Span::raw("tab: toggle page "));
        if tui_state.log_tab_stage.scroll_state.vertical_scroll.is_some() {
            help_bar_text.push(ratatui::text::Span::raw("| enter: Tail log "));
        }
    } else {
        help_bar_text.push(ratatui::text::Span::raw("| tab: toggle page"));
    }

    if tui_state.current_page == Page::Logs {
        help_bar_text.push(ratatui::text::Span::raw("| space: un/pause logging"));
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