use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute, terminal,
};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
    Table, TableState, Wrap,
};
use ratatui::{Frame, Terminal};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::configuration::LogLevel;
use crate::global_state::GlobalState;
use crate::global_state::ProcState;

pub fn init() {
    println!("Starting odd-box TUI (press q or Ctrl+C to exit)...");
}

fn muted_color(light_theme: bool) -> Color {
    if light_theme {
        Color::DarkGray
    } else {
        Color::Gray
    }
}

fn info_color(light_theme: bool) -> Color {
    if light_theme {
        Color::Blue
    } else {
        Color::LightBlue
    }
}

fn timestamp_color(light_theme: bool) -> Color {
    if light_theme {
        Color::Blue
    } else {
        Color::Cyan
    }
}

fn unused_color(light_theme: bool) -> Color {
    if light_theme {
        Color::Yellow
    } else {
        Color::LightYellow
    }
}

fn row_highlight_bg(light_theme: bool) -> Color {
    if light_theme {
        Color::Rgb(220, 226, 234)
    } else {
        Color::DarkGray
    }
}

fn fmt_state(state: ProcState, light_theme: bool) -> (&'static str, Color) {
    match state {
        ProcState::Running => ("running", Color::Green),
        ProcState::Starting => ("starting", Color::Yellow),
        ProcState::Stopping => ("stopping", Color::Yellow),
        ProcState::Stopped => ("stopped", muted_color(light_theme)),
        ProcState::Faulty => ("faulty", Color::Red),
        ProcState::Remote => ("remote", Color::Cyan),
        ProcState::DirServer => ("dir", info_color(light_theme)),
        ProcState::Docker => ("docker", Color::Magenta),
    }
}

fn fmt_level(level: Level, light_theme: bool) -> (&'static str, Color) {
    match level {
        Level::TRACE => ("TRC", muted_color(light_theme)),
        Level::DEBUG => ("DBG", Color::Blue),
        Level::INFO => ("INF", Color::Green),
        Level::WARN => ("WRN", Color::Yellow),
        Level::ERROR => ("ERR", Color::Red),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TuiPage {
    Sites,
    Docker,
    Logs,
    Traffic,
}

const TUI_HEADER_HEIGHT: u16 = 6;
const TUI_FOOTER_HEIGHT: u16 = 2;

struct Snapshot {
    version: String,
    cruma_enabled: bool,
    cruma_fqdn: String,
    cruma_motd: String,
    listen_ports: String,
    processes: Vec<(String, ProcState, String, Vec<String>, String, bool)>,
    remotes: Vec<(String, ProcState, String, String)>,
    statics: Vec<(String, ProcState, String)>,
    docker: Vec<(String, ProcState, String, String)>,
    docker_discovered: Vec<DockerRow>,
    routes: Vec<(String, String, bool, bool)>,
}

#[derive(Clone)]
struct DockerRow {
    container_name: String,
    image: String,
    runtime: String,
    state: String,
    marked: bool,
    detail: String,
    is_routed: bool,
}

fn format_listen_ports(http: Option<u16>, https: Option<u16>) -> String {
    match (http, https) {
        (Some(h), Some(s)) => format!("http:{}  https:{}", h, s),
        (Some(h), None) => format!("http:{}", h),
        (None, Some(s)) => format!("https:{}", s),
        (None, None) => "none".to_string(),
    }
}

async fn build_snapshot(global_state: &GlobalState) -> Snapshot {
    let snapshot = global_state.process_registry.snapshot();
    let cfg = global_state.config.load_full();
    let cruma_assignment = global_state.cruma_assignment.load_full();

    let version = env!("CARGO_PKG_VERSION").to_string();

    let cruma_enabled = cfg.cruma.as_ref().and_then(|c| c.mode()).is_some();
    let (cruma_fqdn, cruma_motd) = if let Some(assignment) = cruma_assignment.as_ref() {
        (
            assignment.assigned_domain.clone(),
            assignment.welcome_message.clone(),
        )
    } else {
        ("pending assignment".to_string(), "-".to_string())
    };
    let http_port = cfg.frontends.http.as_ref().map(|http| http.port);
    let https_port = cfg.frontends.https.as_ref().map(|https| https.port);
    let listen_ports = format_listen_ports(http_port, https_port);

    let processes = cfg
        .hosted_processes
        .iter()
        .map(|kv| {
            let backend_id = kv.key().clone();
            let proc = kv.value();
            let handle = snapshot.get(&backend_id);
            let proc_state = handle.map(|h| h.state());
            let bin = proc_state
                .as_ref()
                .and_then(|s| s.resolved_bin.clone())
                .unwrap_or_else(|| proc.bin.clone());
            let args = proc_state
                .as_ref()
                .and_then(|s| s.resolved_args.clone())
                .unwrap_or_else(|| proc.args.clone());
            let state = proc_state
                .as_ref()
                .map(|s| s.proc_state.clone())
                .unwrap_or(ProcState::Stopped);
            let port = handle
                .and_then(|h| h.active_port())
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            let exclude_from_start_all = proc.exclude_from_start_all;
            (backend_id, state, bin, args, port, exclude_from_start_all)
        })
        .collect();

    let remotes = cfg
        .remote_sites
        .iter()
        .map(|kv| {
            let backend_id = kv.key().clone();
            let state = snapshot.state_of(&backend_id).unwrap_or(ProcState::Remote);
            let remote = kv.value();
            let (detail, port) = if remote.endpoints.len() == 1 {
                let ep = &remote.endpoints[0];
                let scheme = if remote.https { "https" } else { "http" };
                (format!("{} ({})", ep.addr, scheme), ep.port.to_string())
            } else {
                let ports: Vec<String> = remote
                    .endpoints
                    .iter()
                    .map(|ep| ep.port.to_string())
                    .collect();
                (
                    format!("{} endpoints", remote.endpoints.len()),
                    ports.join(","),
                )
            };
            (backend_id, state, detail, port)
        })
        .collect();

    let statics = cfg
        .static_sites
        .iter()
        .map(|kv| {
            let backend_id = kv.key().clone();
            let state = snapshot
                .state_of(&backend_id)
                .unwrap_or(ProcState::DirServer);
            (backend_id, state, kv.value().dir.clone())
        })
        .collect();

    let docker = cfg
        .docker_containers
        .iter()
        .map(|kv| {
            let cont = kv.value().clone();
            let host = cont.generate_host_name();
            let state = snapshot.state_of(&host).unwrap_or(ProcState::Docker);
            let port = if cont.port == 0 {
                "-".to_string()
            } else {
                cont.port.to_string()
            };
            (host, state, cont.image_name, port)
        })
        .collect();
    let docker_discovered = global_state
        .docker_discovery
        .load_full()
        .iter()
        .map(|cont| {
            let derived_host = format!("{}.odd-box.localhost", cont.container_name);
            let effective_host_name = cont
                .host_name_label
                .clone()
                .unwrap_or_else(|| derived_host.clone());
            let is_routed = cfg.docker_containers.contains_key(&effective_host_name);
            let display_host = cont
                .host_name_label
                .clone()
                .unwrap_or_else(|| derived_host.clone());
            let port_detail = cont
                .odd_box_port
                .or(cont.inferred_private_port)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            DockerRow {
                container_name: cont.container_name.clone(),
                image: cont.image_name.clone(),
                runtime: cont.runtime.clone(),
                state: cont.state.clone(),
                marked: cont.label_odd_box_marked,
                detail: format!("host: {} · port: {}", display_host, port_detail),
                is_routed,
            }
        })
        .collect();

    let mut routes: Vec<(String, String, bool, bool)> = Vec::new();
    if let Some(http) = &cfg.frontends.http {
        for (host, target) in &http.routes {
            routes.push((
                host.clone(),
                target.backend_id().to_string(),
                false,
                target.enable_cruma(),
            ));
        }
    }
    if let Some(https) = &cfg.frontends.https {
        if let Some(crate::configuration::HttpsRoutes::Explicit(explicit)) = &https.routes {
            for (host, target) in explicit {
                if !routes.iter().any(|(h, _, _, _)| h == host) {
                    routes.push((
                        host.clone(),
                        target.backend_id().to_string(),
                        true,
                        target.enable_cruma(),
                    ));
                }
            }
        }
    }

    Snapshot {
        version,
        cruma_enabled,
        cruma_fqdn,
        cruma_motd,
        listen_ports,
        processes,
        remotes,
        statics,
        docker,
        docker_discovered,
        routes,
    }
}

fn draw_ui(
    f: &mut Frame<'_>,
    data: &Snapshot,
    page: TuiPage,
    light_theme: bool,
    log_entries: &VecDeque<LogLine>,
    log_scroll: usize,
    log_tail: bool,
    log_show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    confirm_quit: bool,
    hovered_row: Option<usize>,
    capture_snapshot: &cruma_proxy_lib::proxying::capture_store::CaptureSnapshot,
    traffic_inspection_enabled: bool,
    http_capture_store: &Arc<cruma_proxy_lib::proxying::capture_store::HttpCaptureStore>,
) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TUI_HEADER_HEIGHT),
            Constraint::Min(0),
            Constraint::Length(TUI_FOOTER_HEIGHT),
        ])
        .split(f.area());

    let header_block = Block::default().borders(Borders::ALL);
    let header_inner = header_block.inner(root[0]);
    f.render_widget(header_block, root[0]);
    let header_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(header_inner);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("ODD-BOX v{}", data.version),
            Style::default().bold(),
        ),
        // Span::raw("  "),
        // Span::raw(match page {
        //     TuiPage::Sites => "TUI (sites)",
        //     TuiPage::Docker => "TUI (docker)",
        //     TuiPage::Logs => "TUI (logs)",
        // }),
    ]));

    let status = if data.cruma_enabled {
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Cruma FQDN: ",
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::raw(&data.cruma_fqdn),
            ]),
            Line::from(vec![
                Span::styled(
                    "Cruma MOTD: ",
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::raw(&data.cruma_motd),
            ]),
            Line::from(vec![
                Span::styled("Listen: ", Style::default().fg(muted_color(light_theme))),
                Span::raw(&data.listen_ports),
            ]),
        ])
    } else {
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Cruma Ingress: ",
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::raw("Disabled"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Listen: ", Style::default().fg(muted_color(light_theme))),
                Span::raw(&data.listen_ports),
            ]),
        ])
    };

    f.render_widget(header, header_rows[0]);
    f.render_widget(status, header_rows[1]);

    match page {
        TuiPage::Sites => {
            let (table, mut state, total, start, visible, scroll_area) =
                build_flat_table(data, root[1], hovered_row, light_theme);
            f.render_stateful_widget(table, root[1], &mut state);

            if total > visible {
                let content_len = total.saturating_sub(visible).saturating_add(1).max(1);
                let mut state = ScrollbarState::new(content_len)
                    .position(start.min(content_len.saturating_sub(1)))
                    .viewport_content_length(visible.max(1));
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                f.render_stateful_widget(scrollbar, scroll_area, &mut state);
            }
        }
        TuiPage::Docker => {
            let (table, mut state, total, start, visible, scroll_area) =
                build_docker_table(data, root[1], hovered_row, light_theme);
            f.render_stateful_widget(table, root[1], &mut state);

            if total > visible {
                let content_len = total.saturating_sub(visible).saturating_add(1).max(1);
                let mut state = ScrollbarState::new(content_len)
                    .position(start.min(content_len.saturating_sub(1)))
                    .viewport_content_length(visible.max(1));
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                f.render_stateful_widget(scrollbar, scroll_area, &mut state);
            }
        }
        TuiPage::Logs => {
            let (view, total, start, visible, scroll_area) = build_log_view(
                log_entries,
                log_scroll,
                root[1],
                log_show_timestamp,
                log_level_filter,
                light_theme,
            );
            f.render_widget(view, root[1]);
            if total > visible {
                let content_len = total.saturating_sub(visible).saturating_add(1).max(1);
                let mut state = ScrollbarState::new(content_len)
                    .position(start.min(content_len.saturating_sub(1)))
                    .viewport_content_length(visible.max(1));
                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                f.render_stateful_widget(scrollbar, scroll_area, &mut state);
            }
        }
        TuiPage::Traffic => {
            let detailed = TRAFFIC_DETAILED_MODE.load(std::sync::atomic::Ordering::Relaxed);
            let zoom = TRAFFIC_ZOOM_MODE.load(std::sync::atomic::Ordering::Relaxed);
            if detailed || zoom {
                let (view, total, start, visible, scroll_area) = build_traffic_detailed_view(
                    capture_snapshot,
                    traffic_inspection_enabled,
                    root[1],
                    light_theme,
                    http_capture_store,
                    zoom,
                );
                f.render_widget(view, root[1]);
                if total > visible {
                    let content_len = total.saturating_sub(visible).saturating_add(1).max(1);
                    let mut state = ScrollbarState::new(content_len)
                        .position(start.min(content_len.saturating_sub(1)))
                        .viewport_content_length(visible.max(1));
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                    f.render_stateful_widget(scrollbar, scroll_area, &mut state);
                }
            } else {
                let (table, mut tbl_state, total, start, visible, scroll_area) =
                    build_traffic_table(
                        capture_snapshot,
                        traffic_inspection_enabled,
                        root[1],
                        light_theme,
                    );
                f.render_stateful_widget(table, root[1], &mut tbl_state);

                if total > visible {
                    let content_len = total.saturating_sub(visible).saturating_add(1).max(1);
                    let mut state = ScrollbarState::new(content_len)
                        .position(start.min(content_len.saturating_sub(1)))
                        .viewport_content_length(visible.max(1));
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                    f.render_stateful_widget(scrollbar, scroll_area, &mut state);
                }
            }
        }
    }

    let mut footer_spans = vec![
        Span::styled("↑/↓", Style::default().fg(muted_color(light_theme))),
        Span::raw(" scroll  "),
        Span::styled("tab", Style::default().fg(muted_color(light_theme))),
        Span::raw(" switch  "),
    ];
    match page {
        TuiPage::Sites => footer_spans.extend([
            Span::styled("s", Style::default().fg(muted_color(light_theme))),
            Span::raw(" (start all)  "),
            Span::styled("x", Style::default().fg(muted_color(light_theme))),
            Span::raw(" (stop all)  "),
            Span::styled("p", Style::default().fg(muted_color(light_theme))),
            Span::raw(
                if SHOW_FULL_PATH.load(std::sync::atomic::Ordering::Relaxed) {
                    " (short paths)  "
                } else {
                    " (full paths)  "
                },
            ),
        ]),
        TuiPage::Docker => {}
        TuiPage::Traffic => {
            let detailed = TRAFFIC_DETAILED_MODE.load(std::sync::atomic::Ordering::Relaxed);
            let zoom = TRAFFIC_ZOOM_MODE.load(std::sync::atomic::Ordering::Relaxed);
            footer_spans.extend([
                Span::styled("i", Style::default().fg(muted_color(light_theme))),
                Span::raw(if traffic_inspection_enabled {
                    " (capture:on)  "
                } else {
                    " (capture:off)  "
                }),
                Span::styled("d", Style::default().fg(muted_color(light_theme))),
                Span::raw(if zoom {
                    " (zoom)  "
                } else if detailed {
                    " (detailed)  "
                } else {
                    " (simple)  "
                }),
            ]);
            if detailed {
                footer_spans.extend([
                    Span::styled("z", Style::default().fg(muted_color(light_theme))),
                    Span::raw(if zoom {
                        " (zoom:on)  "
                    } else {
                        " (zoom:off)  "
                    }),
                ]);
            }
            footer_spans.extend([
                Span::styled("c", Style::default().fg(muted_color(light_theme))),
                Span::raw(" (clear)  "),
            ]);
        }
        TuiPage::Logs => footer_spans.extend([
            Span::styled("f", Style::default().fg(muted_color(light_theme))),
            Span::raw(if log_tail {
                " (tail:on)  "
            } else {
                " (tail:off)  "
            }),
            Span::styled("t", Style::default().fg(muted_color(light_theme))),
            Span::raw(if log_show_timestamp {
                " (timestamps:on)  "
            } else {
                " (timestamps:off)  "
            }),
            Span::styled("l", Style::default().fg(muted_color(light_theme))),
            Span::raw(format!(" (log-lvl:{})  ", log_level_filter.label())),
            Span::styled("c", Style::default().fg(muted_color(light_theme))),
            Span::raw(" (clear)  "),
        ]),
    }
    if page == TuiPage::Docker {
        footer_spans.extend([
            Span::styled("docker/podman", Style::default().fg(muted_color(light_theme))),
            Span::raw(" set labels: odd_box=true odd_box_port=<container-port> [odd_box_host_name=<host>]  "),
        ]);
    }
    footer_spans.extend([
        Span::styled("q", Style::default().fg(muted_color(light_theme))),
        Span::raw(" quit"),
    ]);

    let footer =
        Paragraph::new(Line::from(footer_spans)).block(Block::default().borders(Borders::TOP));
    f.render_widget(footer, root[2]);

    if confirm_quit {
        let area = f.area();
        let modal_width = 36;
        let modal_height = 7;
        let modal = Rect {
            x: area.x + area.width.saturating_sub(modal_width) / 2,
            y: area.y + area.height.saturating_sub(modal_height) / 2,
            width: modal_width,
            height: modal_height,
        };

        let backdrop = Block::default().style(Style::default().bg(Color::DarkGray));
        let top = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: modal.y.saturating_sub(area.y),
        };
        let bottom = Rect {
            x: area.x,
            y: modal.y + modal.height,
            width: area.width,
            height: area
                .y
                .saturating_add(area.height)
                .saturating_sub(modal.y + modal.height),
        };
        let left = Rect {
            x: area.x,
            y: modal.y,
            width: modal.x.saturating_sub(area.x),
            height: modal.height,
        };
        let right = Rect {
            x: modal.x + modal.width,
            y: modal.y,
            width: area
                .x
                .saturating_add(area.width)
                .saturating_sub(modal.x + modal.width),
            height: modal.height,
        };
        for rect in [top, bottom, left, right] {
            if rect.width > 0 && rect.height > 0 {
                f.render_widget(backdrop.clone(), rect);
            }
        }

        let modal_block = Block::default()
            .borders(Borders::ALL)
            .title("Quit?")
            .style(Style::default().bg(Color::White).fg(Color::Black));
        let content = Paragraph::new(vec![
            Line::from("Really quit the TUI?"),
            Line::from(""),
            Line::from("y = yes   n = no"),
        ])
        .block(modal_block)
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::White).fg(Color::Black));
        let modal_bg = Paragraph::new("")
            .style(Style::default().bg(Color::White))
            .alignment(Alignment::Center);
        f.render_widget(Clear, modal);
        f.render_widget(modal_bg, modal);
        f.render_widget(content, modal);
    }
}

struct RowData {
    kind: &'static str,
    name: String,
    process_id: Option<String>,
    state: ProcState,
    state_label: Option<&'static str>,
    state_color: Option<Color>,
    detail: String,
    port: String,
    muted: bool,
    alert: bool,
}

fn build_rows(data: &Snapshot, light_theme: bool) -> Vec<RowData> {
    let mut rows = Vec::new();
    let mut backend_ids: Vec<String> = Vec::new();
    backend_ids.extend(data.processes.iter().map(|(n, _, _, _, _, _)| n.clone()));
    backend_ids.extend(data.remotes.iter().map(|(n, _, _, _)| n.clone()));
    backend_ids.extend(data.statics.iter().map(|(n, _, _)| n.clone()));
    backend_ids.extend(data.docker.iter().map(|(n, _, _, _)| n.clone()));

    let process_ids: std::collections::HashSet<String> = data
        .processes
        .iter()
        .map(|(n, _, _, _, _, _)| n.clone())
        .collect();

    let mut backend_state: std::collections::BTreeMap<String, ProcState> =
        std::collections::BTreeMap::new();
    let mut backend_kind: std::collections::BTreeMap<String, &'static str> =
        std::collections::BTreeMap::new();
    for (name, state, _, _, _, _) in &data.processes {
        backend_state.insert(name.clone(), state.clone());
        backend_kind.insert(name.clone(), "process");
    }
    for (name, state, _, _) in &data.remotes {
        backend_state.insert(name.clone(), state.clone());
        backend_kind.insert(name.clone(), "remote");
    }
    for (name, state, _) in &data.statics {
        backend_state.insert(name.clone(), state.clone());
        backend_kind.insert(name.clone(), "static");
    }
    for (name, state, _, _) in &data.docker {
        backend_state.insert(name.clone(), state.clone());
        backend_kind.insert(name.clone(), "docker");
    }

    let mut routes_by_backend: std::collections::BTreeMap<String, Vec<(String, bool, bool)>> =
        std::collections::BTreeMap::new();
    for (host, backend, https_only, enable_cruma) in &data.routes {
        routes_by_backend.entry(backend.clone()).or_default().push((
            host.clone(),
            *https_only,
            *enable_cruma,
        ));
    }

    for (host, backend, https_only, enable_cruma) in &data.routes {
        let missing = !backend_ids.contains(backend);
        let route_count = routes_by_backend.get(backend).map(|r| r.len()).unwrap_or(0);
        if !missing && route_count == 1 {
            continue;
        }
        let backend_state = backend_state.get(backend).cloned();
        let backend_ok = matches!(
            backend_state,
            Some(ProcState::Running | ProcState::Remote | ProcState::DirServer | ProcState::Docker)
        );
        let state = if missing {
            ProcState::Faulty
        } else if backend_ok {
            ProcState::Running
        } else {
            ProcState::Faulty
        };
        let backend_kind = backend_kind.get(backend).copied().unwrap_or("unknown");
        let mut detail = if *https_only {
            format!("{} ({}, https-only)", backend, backend_kind)
        } else {
            format!("{} ({})", backend, backend_kind)
        };
        if *enable_cruma {
            detail.push_str(" [cruma]");
        }
        if missing {
            detail.push_str(" (missing backend)");
        } else if !backend_ok {
            detail.push_str(" (backend not running)");
        }
        let process_id = if process_ids.contains(backend) {
            Some(backend.clone())
        } else {
            None
        };
        rows.push(RowData {
            kind: "route",
            name: host.clone(),
            process_id,
            state,
            state_label: None,
            state_color: None,
            detail,
            port: String::new(),
            muted: false,
            alert: missing || !backend_ok,
        });
    }
    for (name, state, bin, args, port, _) in &data.processes {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let is_unused = !routes_by_backend.contains_key(name);
        let display_bin = if SHOW_FULL_PATH.load(std::sync::atomic::Ordering::Relaxed) {
            bin.clone()
        } else {
            std::path::Path::new(bin.as_str())
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| bin.clone())
        };
        let bin_and_args = if args.is_empty() {
            display_bin
        } else {
            format!("{} {}", display_bin, args.join(" "))
        };
        let (display_name, suffix) = if let Some((host, https_only, enable_cruma)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            let cruma_tag = if enable_cruma { " [cruma]" } else { "" };
            (host, format!("{}{}{}", bin_and_args, marker, cruma_tag))
        } else {
            (name.clone(), format!("{} (no frontend)", bin_and_args))
        };
        rows.push(RowData {
            kind: "process",
            name: display_name,
            process_id: Some(name.clone()),
            state: state.clone(),
            state_label: is_unused.then_some("unused"),
            state_color: is_unused.then_some(unused_color(light_theme)),
            detail: suffix,
            port: port.clone(),
            muted: false,
            alert: false,
        });
    }
    for (name, state, detail, remote_port) in &data.remotes {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let is_unused = !routes_by_backend.contains_key(name);
        let (display_name, suffix, muted, alert) =
            if let Some((host, https_only, enable_cruma)) = combined {
                let marker = if https_only { " (https-only)" } else { "" };
                let cruma_tag = if enable_cruma { " [cruma]" } else { "" };
                (
                    host,
                    format!("{}{}{} · {}", name, marker, cruma_tag, detail),
                    false,
                    false,
                )
            } else {
                (
                    name.clone(),
                    format!("{} (no frontend)", detail),
                    false,
                    false,
                )
            };
        rows.push(RowData {
            kind: "remote",
            name: display_name,
            process_id: None,
            state: state.clone(),
            state_label: is_unused.then_some("unused"),
            state_color: is_unused.then_some(unused_color(light_theme)),
            detail: suffix,
            port: remote_port.clone(),
            muted,
            alert,
        });
    }
    for (name, state, dir) in &data.statics {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let is_unused = !routes_by_backend.contains_key(name);
        let (display_name, suffix, muted, alert) =
            if let Some((host, https_only, enable_cruma)) = combined {
                let marker = if https_only { " (https-only)" } else { "" };
                let cruma_tag = if enable_cruma { " [cruma]" } else { "" };
                (
                    host,
                    format!("{}{}{} · {}", name, marker, cruma_tag, dir),
                    false,
                    false,
                )
            } else {
                (name.clone(), format!("{} (no frontend)", dir), false, false)
            };
        rows.push(RowData {
            kind: "static",
            name: display_name,
            process_id: None,
            state: state.clone(),
            state_label: is_unused.then_some("unused"),
            state_color: is_unused.then_some(unused_color(light_theme)),
            detail: suffix,
            port: data.listen_ports.clone(),
            muted,
            alert,
        });
    }
    for (name, state, image, docker_port) in &data.docker {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let is_unused = !routes_by_backend.contains_key(name);
        let (display_name, suffix) = if let Some((host, https_only, enable_cruma)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            let cruma_tag = if enable_cruma { " [cruma]" } else { "" };
            (host, format!("{}{}{} · {}", name, marker, cruma_tag, image))
        } else {
            (name.clone(), format!("{image} (no frontend)"))
        };
        rows.push(RowData {
            kind: "docker",
            name: display_name,
            process_id: None,
            state: state.clone(),
            state_label: is_unused.then_some("unused"),
            state_color: is_unused.then_some(unused_color(light_theme)),
            detail: suffix,
            port: docker_port.clone(),
            muted: false,
            alert: false,
        });
    }
    rows.sort_by_cached_key(|row| row.name.to_ascii_lowercase());
    rows
}

fn build_flat_table<'a>(
    data: &'a Snapshot,
    area: ratatui::layout::Rect,
    hovered_row: Option<usize>,
    light_theme: bool,
) -> (
    Table<'a>,
    TableState,
    usize,
    usize,
    usize,
    ratatui::layout::Rect,
) {
    let rows = build_rows(data, light_theme);
    let total = rows.len();
    let visible = area.height.saturating_sub(3) as usize;
    let start = TUI_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let max_start = total.saturating_sub(visible);
    let start = start.min(max_start);
    TUI_SCROLL.store(start, std::sync::atomic::Ordering::Relaxed);
    let end = (start + visible).min(total);

    let rows_vec: Vec<Row> = rows[start..end]
        .iter()
        .map(|row| {
            let (default_label, default_color) = fmt_state(row.state.clone(), light_theme);
            let label = row.state_label.unwrap_or(default_label);
            let color = row.state_color.unwrap_or(default_color);
            let mut name_cell = Cell::from(row.name.clone());
            let mut detail_cell = Cell::from(row.detail.clone());
            let mut kind_cell = Cell::from(row.kind);
            let mut port_cell = Cell::from(row.port.clone());
            if row.alert {
                let alert_style = Style::default().fg(Color::Red);
                name_cell = name_cell.style(alert_style);
                detail_cell = detail_cell.style(alert_style);
                kind_cell = kind_cell.style(alert_style);
                port_cell = port_cell.style(alert_style);
            }
            let mut table_row = Row::new(vec![
                kind_cell,
                Cell::from(label).style(Style::default().fg(color)),
                name_cell,
                detail_cell,
                port_cell,
            ]);
            if row.muted {
                table_row = table_row.style(Style::default().fg(muted_color(light_theme)));
            }
            table_row
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Type"),
        Cell::from("State"),
        Cell::from("Name"),
        Cell::from("Detail"),
        Cell::from("Port"),
    ])
    .style(Style::default().fg(muted_color(light_theme)));

    // Account for table borders and inter-column spacing so we can size columns
    // using the actual render width.
    let inner_width = area.width.saturating_sub(2);
    let spacing = 4; // 5 columns => 4 gaps when column_spacing(1)
    let table_width = inner_width.saturating_sub(spacing);

    let fixed_left = 7 + 8; // Type + State
    let available_after_left = table_width.saturating_sub(fixed_left);

    let mut port_width = rows
        .iter()
        .map(|r| r.port.chars().count() as u16)
        .max()
        .unwrap_or(4)
        .max(4)
        .saturating_add(1)
        .clamp(8, 24);

    // Keep "Name" from greedily consuming space; let "Detail" take the remainder.
    let min_detail = 16;
    let max_name = 24;
    let min_name = 10;

    let name_budget = available_after_left
        .saturating_sub(port_width)
        .saturating_sub(min_detail);
    let name_width = if name_budget < min_name {
        name_budget
    } else {
        name_budget.min(max_name)
    };

    // If space is tight, shrink port first down to a safe minimum before clipping detail.
    let need_for_detail = fixed_left
        .saturating_add(name_width)
        .saturating_add(port_width)
        .saturating_add(min_detail);
    if table_width < need_for_detail {
        let shortage = need_for_detail - table_width;
        port_width = port_width.saturating_sub(shortage).max(8);
    }

    let table = Table::new(
        rows_vec,
        [
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Length(name_width),
            Constraint::Fill(1),
            Constraint::Length(port_width),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(row_highlight_bg(light_theme)))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Sites ({} total)", total)),
    )
    .column_spacing(1);

    let scroll_area = ratatui::layout::Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y + 1,
        width: 1,
        height: area.height.saturating_sub(2),
    };

    let mut state = TableState::default();
    if let Some(row) = hovered_row {
        if row >= start && row < end {
            state.select(Some(row - start));
        }
    }

    (table, state, total, start, visible, scroll_area)
}

fn build_docker_table<'a>(
    data: &'a Snapshot,
    area: ratatui::layout::Rect,
    hovered_row: Option<usize>,
    light_theme: bool,
) -> (
    Table<'a>,
    TableState,
    usize,
    usize,
    usize,
    ratatui::layout::Rect,
) {
    let rows = sorted_docker_rows(data);

    let total = rows.len();
    let visible = area.height.saturating_sub(3) as usize;
    let start = TUI_DOCKER_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let max_start = total.saturating_sub(visible);
    let start = start.min(max_start);
    TUI_DOCKER_SCROLL.store(start, std::sync::atomic::Ordering::Relaxed);
    let end = (start + visible).min(total);

    let rows_vec: Vec<Row> = rows[start..end]
        .iter()
        .map(|row| {
            let name = &row.container_name;
            let image = &row.image;
            let runtime = &row.runtime;
            let state = &row.state;
            let marked = row.marked;
            let detail = &row.detail;
            let is_routed = row.is_routed;
            let state_color = if state.eq_ignore_ascii_case("running") {
                Color::Green
            } else if state.eq_ignore_ascii_case("exited")
                || state.eq_ignore_ascii_case("dead")
                || state.eq_ignore_ascii_case("failed")
            {
                Color::Red
            } else {
                muted_color(light_theme)
            };
            let state_label = state.clone();
            let mark_label = if marked { "yes" } else { "no" };
            let mark_color = if marked {
                Color::Cyan
            } else {
                muted_color(light_theme)
            };
            let route_label = if is_routed {
                "active"
            } else if marked {
                "marked"
            } else {
                "-"
            };
            let route_color = if is_routed {
                Color::Green
            } else if marked {
                Color::Yellow
            } else {
                muted_color(light_theme)
            };
            Row::new(vec![
                Cell::from(name.clone()),
                Cell::from(state_label).style(Style::default().fg(state_color)),
                Cell::from(mark_label).style(Style::default().fg(mark_color)),
                Cell::from(route_label).style(Style::default().fg(route_color)),
                Cell::from(format!("{} · runtime: {} · {}", image, runtime, detail)),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("State"),
        Cell::from("Marked"),
        Cell::from("Route"),
        Cell::from("Detail"),
    ])
    .style(Style::default().fg(muted_color(light_theme)));

    let table = Table::new(
        rows_vec,
        [
            Constraint::Percentage(24),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(row_highlight_bg(light_theme)))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Docker ({} total)", total)),
    )
    .column_spacing(1);

    let scroll_area = ratatui::layout::Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y + 1,
        width: 1,
        height: area.height.saturating_sub(2),
    };

    let mut state = TableState::default();
    if let Some(row) = hovered_row {
        if row >= start && row < end {
            state.select(Some(row - start));
        }
    }

    (table, state, total, start, visible, scroll_area)
}

fn hit_test_site_row(area: Rect, mouse_y: u16, total: usize) -> Option<usize> {
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if inner.height < 2 {
        return None;
    }
    let row_y_start = inner.y.saturating_add(1);
    if mouse_y < row_y_start || mouse_y >= inner.y.saturating_add(inner.height) {
        return None;
    }

    let visible = inner.height.saturating_sub(1) as usize;
    let start = TUI_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let row_in_view = mouse_y.saturating_sub(row_y_start) as usize;
    if row_in_view >= visible {
        return None;
    }

    let row_index = start.saturating_add(row_in_view);
    if row_index >= total {
        return None;
    }

    Some(row_index)
}

fn sorted_docker_rows(data: &Snapshot) -> Vec<DockerRow> {
    let mut rows = data.docker_discovered.clone();
    rows.sort_by_cached_key(|row| row.container_name.to_ascii_lowercase());
    rows
}

fn hit_test_docker_row(area: Rect, mouse_y: u16, total: usize) -> Option<usize> {
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    if inner.height < 2 {
        return None;
    }
    let row_y_start = inner.y.saturating_add(1);
    if mouse_y < row_y_start || mouse_y >= inner.y.saturating_add(inner.height) {
        return None;
    }

    let visible = inner.height.saturating_sub(1) as usize;
    let start = TUI_DOCKER_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let row_in_view = mouse_y.saturating_sub(row_y_start) as usize;
    if row_in_view >= visible {
        return None;
    }

    let row_index = start.saturating_add(row_in_view);
    if row_index >= total {
        return None;
    }

    Some(row_index)
}

fn scroll_area_for_content(area: Rect) -> Rect {
    ratatui::layout::Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y + 1,
        width: 1,
        height: area.height.saturating_sub(2),
    }
}

fn scroll_pos_from_mouse(scroll_area: Rect, mouse_row: u16, max_start: usize) -> usize {
    if max_start == 0 || scroll_area.height <= 1 {
        return 0;
    }
    let rel = mouse_row.saturating_sub(scroll_area.y) as usize;
    let height = scroll_area.height.saturating_sub(1) as usize;
    let clamped = rel.min(height);
    (clamped * max_start) / height
}

static TUI_SCROLL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TUI_DOCKER_SCROLL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TUI_LOG_SCROLL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TUI_TRAFFIC_SCROLL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static SHOW_FULL_PATH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TRAFFIC_DETAILED_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static TRAFFIC_ZOOM_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Cached total line count from the last detailed/zoom render pass.
/// Used by mouse scroll handlers so they don't need to recompute it.
static TRAFFIC_TOTAL_LINES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// ─── Traffic inspection: detailed / zoom view helpers ─────────────────────

fn traffic_truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else if max <= 1 {
        "…".into()
    } else {
        let mut truncated: String = input.chars().take(max - 1).collect();
        truncated.push('…');
        truncated
    }
}

/// Replace control characters that would corrupt the terminal with safe
/// visual substitutes. Keeps `\n`, `\r`, `\t` (handled by the TUI layout),
/// replaces everything else < 0x20 and DEL (0x7f) with the Unicode
/// "Control Pictures" block (U+2400–U+2421) or a `·` placeholder.
fn sanitize_for_tui(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' | '\r' | '\t' => out.push(ch),
            // ASCII control 0x00–0x1F → Unicode Control Pictures U+2400–U+241F
            c if (c as u32) < 0x20 => {
                out.push(char::from_u32(0x2400 + c as u32).unwrap_or('.'));
            }
            '\x7f' => out.push('␡'),
            // BOM / zero-width chars / other invisible Unicode
            '\u{feff}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{fffe}' => out.push('·'),
            c => out.push(c),
        }
    }
    out
}

/// Try to interpret `bytes` as UTF-8 text and return a sanitized preview.
/// Returns `None` when the content is not valid UTF-8 (i.e. binary data).
/// Callers should fall back to a simple byte-count label when this returns `None`.
fn body_preview_string(bytes: &[u8], max_len: usize) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let sanitized = sanitize_for_tui(text);
    if sanitized.len() <= max_len {
        Some(sanitized)
    } else {
        let mut preview: String = sanitized.chars().take(max_len.saturating_sub(1)).collect();
        preview.push('…');
        Some(preview)
    }
}

fn find_header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

fn try_decompress(bytes: &[u8], content_encoding: Option<&str>) -> Vec<u8> {
    let encoding = match content_encoding {
        Some(e) => e.trim().to_ascii_lowercase(),
        None => return try_decompress_magic(bytes),
    };
    match encoding.as_str() {
        "gzip" | "x-gzip" => decompress_gzip(bytes).unwrap_or_else(|| bytes.to_vec()),
        "deflate" => decompress_deflate(bytes).unwrap_or_else(|| bytes.to_vec()),
        "identity" | "" => bytes.to_vec(),
        _ => bytes.to_vec(),
    }
}

fn try_decompress_magic(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        decompress_gzip(bytes).unwrap_or_else(|| bytes.to_vec())
    } else {
        bytes.to_vec()
    }
}

fn decompress_gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn decompress_deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn read_body_preview(
    capture_store: &Arc<cruma_proxy_lib::proxying::capture_store::HttpCaptureStore>,
    req_id: u64,
    is_request: bool,
    content_encoding: Option<&str>,
) -> Option<String> {
    let captured = capture_store.body_bytes(req_id)?;
    let raw = if is_request {
        captured.req_body.as_ref()?
    } else {
        captured.resp_body.as_ref()?
    };
    if raw.is_empty() {
        return None;
    }
    let bytes = try_decompress(raw, content_encoding);
    body_preview_string(&bytes, 4096)
}

fn format_headers_preview(headers: &[(String, String)], max_headers: usize) -> String {
    let preview: Vec<String> = headers
        .iter()
        .take(max_headers)
        .map(|(k, v)| {
            format!(
                "{}: {}",
                sanitize_for_tui(k),
                traffic_truncate(&sanitize_for_tui(v), 40)
            )
        })
        .collect();
    let mut result = preview.join(", ");
    if headers.len() > max_headers {
        result.push_str(&format!(" (+{} more)", headers.len() - max_headers));
    }
    result
}

fn traffic_kind_badge(kind: &cruma_proxy_lib::proxying::HttpRequestKind) -> (&'static str, Color) {
    use cruma_proxy_lib::proxying::HttpRequestKind;
    match kind {
        HttpRequestKind::Regular => ("HTTP", Color::DarkGray),
        HttpRequestKind::SSE => ("SSE", Color::LightGreen),
        HttpRequestKind::WebSocket => ("WS", Color::LightMagenta),
        HttpRequestKind::H2cUpgrade => ("H2C", Color::LightCyan),
    }
}

fn is_streaming(entry: &cruma_proxy_lib::proxying::capture_store::CapturedExchange) -> bool {
    use cruma_proxy_lib::proxying::HttpRequestKind;
    entry.is_inflight
        && matches!(
            entry.kind,
            HttpRequestKind::SSE | HttpRequestKind::WebSocket
        )
}

fn format_traffic_request_detailed<'a>(
    entry: &cruma_proxy_lib::proxying::capture_store::CapturedExchange,
    capture_store: &Arc<cruma_proxy_lib::proxying::capture_store::HttpCaptureStore>,
    light_theme: bool,
) -> Vec<Line<'a>> {
    use cruma_proxy_lib::proxying::HttpRequestKind;
    let mut lines = Vec::new();

    // Line 1: Request line with response status
    let mut request_line = vec![Span::styled(
        entry
            .client_addr
            .as_ref()
            .map(|s| format!("[{}] ", sanitize_for_tui(s)))
            .unwrap_or_else(|| "[?] ".to_string()),
        Style::default().fg(muted_color(light_theme)),
    )];

    if entry.kind != HttpRequestKind::Regular {
        let (badge_text, badge_color) = traffic_kind_badge(&entry.kind);
        request_line.push(Span::styled(
            format!("[{badge_text}] "),
            Style::default().fg(badge_color).bold(),
        ));
    }

    if let Some(ref ver) = entry.http_version {
        request_line.push(Span::styled(
            format!("{} ", sanitize_for_tui(ver)),
            Style::default().fg(muted_color(light_theme)),
        ));
    }

    request_line.extend([
        Span::styled(
            format!("→ {}", sanitize_for_tui(&entry.method)),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            sanitize_for_tui(&entry.host.clone().unwrap_or_else(|| "<no-host>".into())),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(sanitize_for_tui(&entry.path)),
    ]);

    request_line.push(Span::raw("  "));
    if is_streaming(entry) {
        let stream_label = match entry.kind {
            HttpRequestKind::SSE => "⇣ streaming (SSE)",
            HttpRequestKind::WebSocket => "⇅ open (WS)",
            _ => "⇣ streaming",
        };
        request_line.push(Span::styled(
            stream_label,
            Style::default().fg(Color::LightGreen).bold(),
        ));
        if let Some(status) = entry.status {
            request_line.push(Span::styled(
                format!(" {status}"),
                Style::default().fg(Color::Green),
            ));
        }
    } else {
        match (entry.status, entry.duration_ms) {
            (Some(status), Some(duration)) => {
                let status_style = if status >= 200 && status < 300 {
                    Style::default().fg(Color::Green)
                } else if status >= 400 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                request_line.push(Span::styled(
                    format!("← {} ({} ms)", status, duration),
                    status_style.bold(),
                ));
            }
            _ => {
                request_line.push(Span::styled(
                    "↻ pending...",
                    Style::default().fg(muted_color(light_theme)),
                ));
            }
        }
    }
    lines.push(Line::from(request_line));

    // Request headers preview
    if let Some(headers) = &entry.req_headers {
        if !headers.is_empty() {
            let header_preview = format_headers_preview(headers, 3);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Req Headers: ",
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::styled(header_preview, Style::default().fg(Color::Blue)),
            ]));
        }
    }

    // Request body preview
    if entry.req_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if entry.req_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = entry
            .req_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        if let Some(preview) = read_body_preview(
            capture_store,
            entry.req_id,
            true,
            content_encoding.as_deref(),
        ) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Req Body: ", Style::default().fg(muted_color(light_theme))),
                Span::styled(
                    format!(
                        "({} bytes{}) ",
                        entry.req_body_size.unwrap_or(0),
                        truncated_marker
                    ),
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::styled(
                    traffic_truncate(&preview, 200),
                    Style::default().fg(Color::Blue),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Req Body: ", Style::default().fg(muted_color(light_theme))),
                Span::styled(
                    format!(
                        "[binary data, {} bytes{}]",
                        entry.req_body_size.unwrap_or(0),
                        truncated_marker
                    ),
                    Style::default().fg(muted_color(light_theme)),
                ),
            ]));
        }
    }

    // Response headers preview
    if let Some(headers) = &entry.resp_headers {
        if !headers.is_empty() {
            let header_preview = format_headers_preview(headers, 3);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Resp Headers: ",
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::styled(header_preview, Style::default().fg(Color::Magenta)),
            ]));
        }
    }

    // Response body preview
    if entry.resp_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if entry.resp_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = entry
            .resp_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        if let Some(preview) = read_body_preview(
            capture_store,
            entry.req_id,
            false,
            content_encoding.as_deref(),
        ) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Resp Body: ", Style::default().fg(muted_color(light_theme))),
                Span::styled(
                    format!(
                        "({} bytes{}) ",
                        entry.resp_body_size.unwrap_or(0),
                        truncated_marker
                    ),
                    Style::default().fg(muted_color(light_theme)),
                ),
                Span::styled(
                    traffic_truncate(&preview, 200),
                    Style::default().fg(Color::Magenta),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Resp Body: ", Style::default().fg(muted_color(light_theme))),
                Span::styled(
                    format!(
                        "[binary data, {} bytes{}]",
                        entry.resp_body_size.unwrap_or(0),
                        truncated_marker
                    ),
                    Style::default().fg(muted_color(light_theme)),
                ),
            ]));
        }
    }

    // WebSocket messages (compact)
    if entry.kind == HttpRequestKind::WebSocket {
        if let Some(snap) = capture_store.ws_messages(entry.req_id) {
            use cruma_proxy_lib::proxying::ws_capture::{WsDirection, WsMessageKind};
            let total = snap.total_message_count;
            let shown = snap.messages.len();
            let c2o = snap.total_client_to_origin;
            let o2c = snap.total_origin_to_client;
            let summary = if shown as u64 == total {
                format!("{total} messages  ↑ {c2o} B  ↓ {o2c} B")
            } else {
                format!("{shown}/{total} messages (oldest evicted)  ↑ {c2o} B  ↓ {o2c} B")
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "WS Messages: ",
                    Style::default().fg(Color::LightMagenta).bold(),
                ),
                Span::styled(summary, Style::default().fg(muted_color(light_theme))),
            ]));
            let display_count = snap.messages.len().min(8);
            let start = snap.messages.len().saturating_sub(display_count);
            for msg in &snap.messages[start..] {
                let (arrow, arrow_color) = match msg.direction {
                    WsDirection::ClientToOrigin => ("↑", Color::Cyan),
                    WsDirection::OriginToClient => ("↓", Color::LightMagenta),
                };
                let kind_label = match msg.kind {
                    WsMessageKind::Text => "text",
                    WsMessageKind::Binary => "bin",
                    WsMessageKind::Ping => "ping",
                    WsMessageKind::Pong => "pong",
                    WsMessageKind::Close => "close",
                };
                let payload_preview = if msg.kind == WsMessageKind::Text {
                    match std::str::from_utf8(&msg.payload) {
                        Ok(s) => traffic_truncate(&sanitize_for_tui(s), 120),
                        Err(_) => format!("[binary, {} B]", msg.original_len),
                    }
                } else if msg.payload.is_empty() {
                    String::new()
                } else {
                    format!("[binary, {} B]", msg.original_len)
                };
                let size_info = if msg.original_len != msg.payload.len() {
                    format!("{} B (truncated)", msg.original_len)
                } else {
                    format!("{} B", msg.original_len)
                };
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(arrow, Style::default().fg(arrow_color).bold()),
                    Span::styled(
                        format!(" [{kind_label}] "),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                    Span::styled(
                        format!("({size_info}) "),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                    Span::styled(payload_preview, Style::default().fg(arrow_color)),
                ]));
            }
            if start > 0 {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("… {start} older messages not shown"),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                ]));
            }
        }
    }

    // SSE events (compact)
    if entry.kind == HttpRequestKind::SSE {
        if let Some(snap) = capture_store.sse_events(entry.req_id) {
            let total = snap.total_event_count;
            let shown = snap.events.len();
            let total_bytes = snap.total_bytes;
            let summary = if shown as u64 == total {
                format!("{total} events  ↓ {total_bytes} B")
            } else {
                format!("{shown}/{total} events (oldest evicted)  ↓ {total_bytes} B")
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "SSE Events: ",
                    Style::default().fg(Color::LightGreen).bold(),
                ),
                Span::styled(summary, Style::default().fg(muted_color(light_theme))),
            ]));
            let display_count = snap.events.len().min(8);
            let start = snap.events.len().saturating_sub(display_count);
            for evt in &snap.events[start..] {
                let event_type = sanitize_for_tui(evt.event_type.as_deref().unwrap_or("message"));
                let is_comment_only = evt.data.is_empty() && !evt.comments.is_empty();
                let label = if is_comment_only {
                    "comment".to_string()
                } else {
                    event_type
                };
                let payload_preview = if is_comment_only {
                    traffic_truncate(
                        &sanitize_for_tui(evt.comments.first().map(|s| s.as_str()).unwrap_or("")),
                        120,
                    )
                } else {
                    traffic_truncate(&sanitize_for_tui(&evt.data), 120)
                };
                let size_info = if evt.truncated {
                    format!("{} B (truncated)", evt.original_data_len)
                } else if !evt.data.is_empty() {
                    format!("{} B", evt.data.len())
                } else {
                    String::new()
                };
                let mut spans = vec![
                    Span::raw("    "),
                    Span::styled("↓", Style::default().fg(Color::LightGreen).bold()),
                    Span::styled(
                        format!(" [{label}] "),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                ];
                if let Some(id) = &evt.id {
                    spans.push(Span::styled(
                        format!("id={} ", sanitize_for_tui(id)),
                        Style::default().fg(muted_color(light_theme)),
                    ));
                }
                if !size_info.is_empty() {
                    spans.push(Span::styled(
                        format!("({size_info}) "),
                        Style::default().fg(muted_color(light_theme)),
                    ));
                }
                spans.push(Span::styled(
                    payload_preview,
                    Style::default().fg(Color::LightGreen),
                ));
                lines.push(Line::from(spans));
            }
            if start > 0 {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("… {start} older events not shown"),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                ]));
            }
        }
    }

    // Blank separator
    lines.push(Line::from(""));
    lines
}

fn format_traffic_request_zoom<'a>(
    entry: &cruma_proxy_lib::proxying::capture_store::CapturedExchange,
    capture_store: &Arc<cruma_proxy_lib::proxying::capture_store::HttpCaptureStore>,
    light_theme: bool,
) -> Vec<Line<'a>> {
    use cruma_proxy_lib::proxying::HttpRequestKind;
    let mut lines = Vec::new();

    // Line 1: Request line with response status
    let mut request_line = vec![Span::styled(
        entry
            .client_addr
            .as_ref()
            .map(|s| format!("[{}] ", sanitize_for_tui(s)))
            .unwrap_or_else(|| "[?] ".to_string()),
        Style::default().fg(muted_color(light_theme)),
    )];

    if entry.kind != HttpRequestKind::Regular {
        let (badge_text, badge_color) = traffic_kind_badge(&entry.kind);
        request_line.push(Span::styled(
            format!("[{badge_text}] "),
            Style::default().fg(badge_color).bold(),
        ));
    }

    if let Some(ref ver) = entry.http_version {
        request_line.push(Span::styled(
            format!("{} ", sanitize_for_tui(ver)),
            Style::default().fg(muted_color(light_theme)),
        ));
    }

    request_line.extend([
        Span::styled(
            format!("→ {}", sanitize_for_tui(&entry.method)),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::raw(" "),
        Span::styled(
            sanitize_for_tui(&entry.host.clone().unwrap_or_else(|| "<no-host>".into())),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(sanitize_for_tui(&entry.path)),
    ]);

    request_line.push(Span::raw("  "));
    if is_streaming(entry) {
        let stream_label = match entry.kind {
            HttpRequestKind::SSE => "⇣ streaming (SSE)",
            HttpRequestKind::WebSocket => "⇅ open (WS)",
            _ => "⇣ streaming",
        };
        request_line.push(Span::styled(
            stream_label,
            Style::default().fg(Color::LightGreen).bold(),
        ));
        if let Some(status) = entry.status {
            request_line.push(Span::styled(
                format!(" {status}"),
                Style::default().fg(Color::Green),
            ));
        }
    } else {
        match (entry.status, entry.duration_ms) {
            (Some(status), Some(duration)) => {
                let status_style = if status >= 200 && status < 300 {
                    Style::default().fg(Color::Green)
                } else if status >= 400 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                request_line.push(Span::styled(
                    format!("← {} ({} ms)", status, duration),
                    status_style.bold(),
                ));
            }
            _ => {
                request_line.push(Span::styled(
                    "↻ pending...",
                    Style::default().fg(muted_color(light_theme)),
                ));
            }
        }
    }
    lines.push(Line::from(request_line));

    // Request headers - one per line
    if let Some(headers) = &entry.req_headers {
        if !headers.is_empty() {
            lines.push(Line::from(Span::styled(
                "REQUEST HEADERS:",
                Style::default().fg(Color::Blue).bold().underlined(),
            )));
            for (key, value) in headers {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", sanitize_for_tui(key)),
                        Style::default().fg(Color::Blue).bold(),
                    ),
                    Span::styled(sanitize_for_tui(value), Style::default().fg(Color::Blue)),
                ]));
            }
        }
    }

    // Request body - full content
    if entry.req_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if entry.req_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = entry
            .req_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        lines.push(Line::from(vec![
            Span::styled("REQUEST BODY: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                format!(
                    "{} bytes{}",
                    entry.req_body_size.unwrap_or(0),
                    truncated_marker
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]));
        if let Some(preview) = read_body_preview(
            capture_store,
            entry.req_id,
            true,
            content_encoding.as_deref(),
        ) {
            for body_line in sanitize_for_tui(&preview).lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", body_line),
                    Style::default().fg(Color::Cyan),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "  [binary data, {} bytes{}]",
                    entry.req_body_size.unwrap_or(0),
                    truncated_marker
                ),
                Style::default().fg(muted_color(light_theme)),
            )));
        }
    }

    // Response headers - one per line
    if let Some(headers) = &entry.resp_headers {
        if !headers.is_empty() {
            lines.push(Line::from(Span::styled(
                "RESPONSE HEADERS:",
                Style::default().fg(Color::Magenta).bold().underlined(),
            )));
            for (key, value) in headers {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {}: ", sanitize_for_tui(key)),
                        Style::default().fg(Color::Magenta).bold(),
                    ),
                    Span::styled(sanitize_for_tui(value), Style::default().fg(Color::Magenta)),
                ]));
            }
        }
    }

    // Response body - full content
    if entry.resp_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if entry.resp_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = entry
            .resp_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        lines.push(Line::from(vec![
            Span::styled(
                "RESPONSE BODY: ",
                Style::default().fg(Color::LightMagenta).bold(),
            ),
            Span::styled(
                format!(
                    "{} bytes{}",
                    entry.resp_body_size.unwrap_or(0),
                    truncated_marker
                ),
                Style::default().fg(Color::LightMagenta),
            ),
        ]));
        if let Some(preview) = read_body_preview(
            capture_store,
            entry.req_id,
            false,
            content_encoding.as_deref(),
        ) {
            for body_line in sanitize_for_tui(&preview).lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", body_line),
                    Style::default().fg(Color::LightMagenta),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "  [binary data, {} bytes{}]",
                    entry.resp_body_size.unwrap_or(0),
                    truncated_marker
                ),
                Style::default().fg(muted_color(light_theme)),
            )));
        }
    }

    // WebSocket messages - full detail
    if entry.kind == HttpRequestKind::WebSocket {
        if let Some(snap) = capture_store.ws_messages(entry.req_id) {
            use cruma_proxy_lib::proxying::ws_capture::{WsDirection, WsMessageKind};
            let total = snap.total_message_count;
            let shown = snap.messages.len();
            let c2o = snap.total_client_to_origin;
            let o2c = snap.total_origin_to_client;
            let summary = if shown as u64 == total {
                format!("{total} messages  ↑ {c2o} B  ↓ {o2c} B")
            } else {
                format!("{shown}/{total} messages (oldest evicted)  ↑ {c2o} B  ↓ {o2c} B")
            };
            lines.push(Line::from(Span::styled(
                format!("WEBSOCKET MESSAGES: {summary}"),
                Style::default().fg(Color::LightMagenta).bold().underlined(),
            )));
            for msg in &snap.messages {
                let (arrow, dir_label, arrow_color) = match msg.direction {
                    WsDirection::ClientToOrigin => ("↑", "send", Color::Cyan),
                    WsDirection::OriginToClient => ("↓", "recv", Color::LightMagenta),
                };
                let kind_label = match msg.kind {
                    WsMessageKind::Text => "text",
                    WsMessageKind::Binary => "bin",
                    WsMessageKind::Ping => "ping",
                    WsMessageKind::Pong => "pong",
                    WsMessageKind::Close => "close",
                };
                let size_info = if msg.original_len != msg.payload.len() {
                    format!(
                        "{} B (truncated from {} B)",
                        msg.payload.len(),
                        msg.original_len
                    )
                } else {
                    format!("{} B", msg.original_len)
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {arrow} {dir_label} "),
                        Style::default().fg(arrow_color).bold(),
                    ),
                    Span::styled(
                        format!("[{kind_label}] "),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                    Span::styled(size_info, Style::default().fg(muted_color(light_theme))),
                ]));
                let payload_str = if msg.kind == WsMessageKind::Text {
                    match std::str::from_utf8(&msg.payload) {
                        Ok(s) => sanitize_for_tui(s),
                        Err(_) => format!("[binary, {} B]", msg.original_len),
                    }
                } else if msg.payload.is_empty() {
                    String::new()
                } else {
                    format!("[binary, {} B]", msg.original_len)
                };
                if !payload_str.is_empty() {
                    for payload_line in payload_str.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("    {payload_line}"),
                            Style::default().fg(arrow_color),
                        )));
                    }
                }
            }
        }
    }

    // SSE events - full detail
    if entry.kind == HttpRequestKind::SSE {
        if let Some(snap) = capture_store.sse_events(entry.req_id) {
            let total = snap.total_event_count;
            let shown = snap.events.len();
            let total_bytes = snap.total_bytes;
            let summary = if shown as u64 == total {
                format!("{total} events  ↓ {total_bytes} B")
            } else {
                format!("{shown}/{total} events (oldest evicted)  ↓ {total_bytes} B")
            };
            lines.push(Line::from(Span::styled(
                format!("SSE EVENTS: {summary}"),
                Style::default().fg(Color::LightGreen).bold().underlined(),
            )));
            for evt in &snap.events {
                let event_type = sanitize_for_tui(evt.event_type.as_deref().unwrap_or("message"));
                let is_comment_only = evt.data.is_empty() && !evt.comments.is_empty();
                let label = if is_comment_only {
                    "comment".to_string()
                } else {
                    event_type
                };
                let size_info = if evt.truncated {
                    format!(
                        "{} B (truncated from {} B)",
                        evt.data.len(),
                        evt.original_data_len
                    )
                } else if !evt.data.is_empty() {
                    format!("{} B", evt.data.len())
                } else {
                    String::new()
                };
                let mut header_spans = vec![
                    Span::styled("  ↓ recv ", Style::default().fg(Color::LightGreen).bold()),
                    Span::styled(
                        format!("[{label}] "),
                        Style::default().fg(muted_color(light_theme)),
                    ),
                ];
                if let Some(id) = &evt.id {
                    header_spans.push(Span::styled(
                        format!("id={} ", sanitize_for_tui(id)),
                        Style::default().fg(muted_color(light_theme)),
                    ));
                }
                if !size_info.is_empty() {
                    header_spans.push(Span::styled(
                        size_info,
                        Style::default().fg(muted_color(light_theme)),
                    ));
                }
                lines.push(Line::from(header_spans));
                if is_comment_only {
                    for comment in &evt.comments {
                        lines.push(Line::from(Span::styled(
                            format!("    : {}", sanitize_for_tui(comment)),
                            Style::default().fg(Color::LightGreen),
                        )));
                    }
                } else if !evt.data.is_empty() {
                    for data_line in sanitize_for_tui(&evt.data).lines() {
                        lines.push(Line::from(Span::styled(
                            format!("    {data_line}"),
                            Style::default().fg(Color::LightGreen),
                        )));
                    }
                }
            }
        }
    }

    // Separator
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "═".repeat(100),
        Style::default().fg(muted_color(light_theme)),
    )));
    lines.push(Line::from(""));
    lines
}

/// Build a Paragraph-based view for detailed or zoom mode traffic inspection.
fn build_traffic_detailed_view<'a>(
    capture_snapshot: &'a cruma_proxy_lib::proxying::capture_store::CaptureSnapshot,
    enabled: bool,
    area: Rect,
    light_theme: bool,
    capture_store: &Arc<cruma_proxy_lib::proxying::capture_store::HttpCaptureStore>,
    zoom: bool,
) -> (Paragraph<'a>, usize, usize, usize, Rect) {
    use std::sync::atomic::Ordering;

    let start = TUI_TRAFFIC_SCROLL.load(Ordering::Relaxed);
    let border_rows = 2u16; // top + bottom border
    let visible = area.height.saturating_sub(border_rows) as usize;

    let mut all_lines: Vec<Line<'a>> = Vec::new();

    if !enabled {
        all_lines.push(Line::from(Span::styled(
            "Traffic inspection is disabled. Press 'i' to enable.",
            Style::default().fg(muted_color(light_theme)),
        )));
    } else if capture_snapshot.order.is_empty() {
        all_lines.push(Line::from(Span::styled(
            "No captured requests yet.",
            Style::default().fg(muted_color(light_theme)),
        )));
    } else {
        // Newest first
        for req_id in capture_snapshot.order.iter().rev() {
            if let Some(entry) = capture_snapshot.entries.get(req_id) {
                let entry_lines = if zoom {
                    format_traffic_request_zoom(entry, capture_store, light_theme)
                } else {
                    format_traffic_request_detailed(entry, capture_store, light_theme)
                };
                all_lines.extend(entry_lines);
            }
        }
    }

    let total = all_lines.len();
    // Cache line count so mouse scroll handlers can use it without recomputing.
    TRAFFIC_TOTAL_LINES.store(total, Ordering::Relaxed);
    let max_start = total.saturating_sub(visible);
    let clamped_start = start.min(max_start);
    if clamped_start != start {
        TUI_TRAFFIC_SCROLL.store(clamped_start, Ordering::Relaxed);
    }

    // Slice to visible window
    let end = (clamped_start + visible).min(total);
    let visible_lines: Vec<Line<'a>> = if clamped_start < total {
        all_lines[clamped_start..end].to_vec()
    } else {
        Vec::new()
    };

    let mode_label = if zoom { "zoom" } else { "detailed" };
    let title = format!(
        " Traffic [{mode_label}] ({} captured{}) ",
        capture_snapshot.order.len(),
        if enabled { "" } else { " — paused" },
    );

    let paragraph = Paragraph::new(visible_lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    let scroll_area = scroll_area_for_content(area);

    (paragraph, total, clamped_start, visible, scroll_area)
}

#[derive(Clone, Debug)]
struct LogLine {
    level: Level,
    source: String,
    timestamp: String,
    message: String,
}

#[derive(Clone, Copy, Debug)]
enum LogLevelFilter {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    fn next(self) -> Self {
        match self {
            LogLevelFilter::Trace => LogLevelFilter::Debug,
            LogLevelFilter::Debug => LogLevelFilter::Info,
            LogLevelFilter::Info => LogLevelFilter::Warn,
            LogLevelFilter::Warn => LogLevelFilter::Error,
            LogLevelFilter::Error => LogLevelFilter::Trace,
        }
    }

    fn label(self) -> &'static str {
        match self {
            LogLevelFilter::Trace => "TRACE+",
            LogLevelFilter::Debug => "DEBUG+",
            LogLevelFilter::Info => "INFO+",
            LogLevelFilter::Warn => "WARN+",
            LogLevelFilter::Error => "ERROR",
        }
    }

    fn from_log_level(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => LogLevelFilter::Trace,
            LogLevel::Debug => LogLevelFilter::Debug,
            LogLevel::Info => LogLevelFilter::Info,
            LogLevel::Warn => LogLevelFilter::Warn,
            LogLevel::Error => LogLevelFilter::Error,
        }
    }

    fn to_log_level(self) -> LogLevel {
        match self {
            LogLevelFilter::Trace => LogLevel::Trace,
            LogLevelFilter::Debug => LogLevel::Debug,
            LogLevelFilter::Info => LogLevel::Info,
            LogLevelFilter::Warn => LogLevel::Warn,
            LogLevelFilter::Error => LogLevel::Error,
        }
    }
}

fn level_rank(level: Level) -> u8 {
    match level {
        Level::TRACE => 0,
        Level::DEBUG => 1,
        Level::INFO => 2,
        Level::WARN => 3,
        Level::ERROR => 4,
    }
}

fn filter_rank(filter: LogLevelFilter) -> u8 {
    match filter {
        LogLevelFilter::Trace => 0,
        LogLevelFilter::Debug => 1,
        LogLevelFilter::Info => 2,
        LogLevelFilter::Warn => 3,
        LogLevelFilter::Error => 4,
    }
}

fn fmt_bytes(n: usize) -> String {
    if n == 0 {
        "0B".to_string()
    } else if n < 1024 {
        format!("{}B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{:.1}M", n as f64 / (1024.0 * 1024.0))
    }
}

fn build_traffic_table<'a>(
    capture_snapshot: &'a cruma_proxy_lib::proxying::capture_store::CaptureSnapshot,
    enabled: bool,
    area: Rect,
    light_theme: bool,
) -> (Table<'a>, TableState, usize, usize, usize, Rect) {
    use std::sync::atomic::Ordering;

    let start = TUI_TRAFFIC_SCROLL.load(Ordering::Relaxed);
    let header_rows = 3u16; // block border + header row + separator
    let visible = area.height.saturating_sub(header_rows) as usize;

    let mut rows: Vec<Row<'a>> = Vec::new();

    if !enabled {
        rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from(Span::styled(
                "Traffic inspection is disabled. Press 'i' to enable.",
                Style::default().fg(muted_color(light_theme)),
            )),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]));
    } else if capture_snapshot.order.is_empty() {
        rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from(Span::styled(
                "No captured requests yet.",
                Style::default().fg(muted_color(light_theme)),
            )),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]));
    } else {
        // Iterate in reverse order so newest requests appear at the top
        for req_id in capture_snapshot.order.iter().rev() {
            if let Some(exchange) = capture_snapshot.entries.get(req_id) {
                let method_color = match exchange.method.as_str() {
                    "GET" => Color::Green,
                    "POST" => Color::Yellow,
                    "PUT" => Color::Blue,
                    "DELETE" => Color::Red,
                    "PATCH" => Color::Magenta,
                    _ => Color::White,
                };

                let status_str = match exchange.status {
                    Some(s) => format!("{}", s),
                    None if exchange.is_inflight => "...".to_string(),
                    None => "—".to_string(),
                };
                let status_color = match exchange.status {
                    Some(s) if s < 300 => Color::Green,
                    Some(s) if s < 400 => Color::Cyan,
                    Some(s) if s < 500 => Color::Yellow,
                    Some(_) => Color::Red,
                    None if exchange.is_inflight => Color::DarkGray,
                    None => Color::DarkGray,
                };

                let duration_str = match exchange.duration_ms {
                    Some(d) if d < 1000 => format!("{}ms", d),
                    Some(d) => format!("{:.1}s", d as f64 / 1000.0),
                    None if exchange.is_inflight => "...".to_string(),
                    None => "—".to_string(),
                };

                let host_str = sanitize_for_tui(&exchange.host.clone().unwrap_or_default());

                let kind_str = format!("{}", exchange.kind);

                let inflight_marker = if exchange.is_inflight { "●" } else { "" };

                // Format body sizes: "req↑ / resp↓"
                let size_str = {
                    let req_sz = exchange
                        .req_body_size
                        .map(|s| fmt_bytes(s))
                        .unwrap_or_default();
                    let resp_sz = exchange
                        .resp_body_size
                        .map(|s| fmt_bytes(s))
                        .unwrap_or_default();
                    let req_trunc = if exchange.req_body_truncated { "+" } else { "" };
                    let resp_trunc = if exchange.resp_body_truncated {
                        "+"
                    } else {
                        ""
                    };
                    if req_sz.is_empty() && resp_sz.is_empty() {
                        if exchange.is_inflight {
                            "...".to_string()
                        } else {
                            "—".to_string()
                        }
                    } else {
                        format!("{}{}↑ {}{}↓", req_sz, req_trunc, resp_sz, resp_trunc)
                    }
                };

                rows.push(Row::new(vec![
                    Cell::from(Span::styled(
                        sanitize_for_tui(&exchange.method),
                        Style::default().fg(method_color).bold(),
                    )),
                    Cell::from(Span::styled(
                        host_str,
                        Style::default().fg(muted_color(light_theme)),
                    )),
                    Cell::from(sanitize_for_tui(&exchange.path)),
                    Cell::from(Span::styled(status_str, Style::default().fg(status_color))),
                    Cell::from(Span::styled(
                        duration_str,
                        Style::default().fg(muted_color(light_theme)),
                    )),
                    Cell::from(Span::styled(
                        size_str,
                        Style::default().fg(muted_color(light_theme)),
                    )),
                    Cell::from(Span::styled(
                        kind_str,
                        Style::default().fg(info_color(light_theme)),
                    )),
                    Cell::from(Span::styled(
                        inflight_marker,
                        Style::default().fg(Color::Yellow),
                    )),
                ]));
            }
        }
    }

    let total = rows.len();

    // Clamp scroll
    let max_start = total.saturating_sub(visible);
    let clamped_start = start.min(max_start);
    if clamped_start != start {
        TUI_TRAFFIC_SCROLL.store(clamped_start, Ordering::Relaxed);
    }

    let title = format!(
        " Traffic ({} captured{}) ",
        capture_snapshot.order.len(),
        if enabled { "" } else { " — paused" },
    );

    let header = Row::new(vec![
        Cell::from(Span::styled("Method", Style::default().bold())),
        Cell::from(Span::styled("Host", Style::default().bold())),
        Cell::from(Span::styled("Path", Style::default().bold())),
        Cell::from(Span::styled("Status", Style::default().bold())),
        Cell::from(Span::styled("Duration", Style::default().bold())),
        Cell::from(Span::styled("Size", Style::default().bold())),
        Cell::from(Span::styled("Kind", Style::default().bold())),
        Cell::from(Span::styled("", Style::default().bold())),
    ])
    .height(1);

    let widths = [
        Constraint::Length(7),
        Constraint::Percentage(20),
        Constraint::Percentage(35),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(4),
        Constraint::Length(1),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(Style::default().bg(row_highlight_bg(light_theme)));

    let mut state = TableState::default();
    state.select(None);
    *state.offset_mut() = clamped_start;

    let scroll_area = scroll_area_for_content(area);

    (table, state, total, clamped_start, visible, scroll_area)
}

fn build_log_view<'a>(
    entries: &'a VecDeque<LogLine>,
    scroll: usize,
    area: ratatui::layout::Rect,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    light_theme: bool,
) -> (Paragraph<'a>, usize, usize, usize, ratatui::layout::Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let content_width = area.width.saturating_sub(3) as usize;
    let all_lines = build_log_lines(
        entries,
        content_width,
        show_timestamp,
        log_level_filter,
        light_theme,
    );
    let total = all_lines.len();
    let max_start = total.saturating_sub(visible);
    let start = scroll.min(max_start);
    TUI_LOG_SCROLL.store(start, std::sync::atomic::Ordering::Relaxed);
    let end = (start + visible).min(total);

    let lines: Vec<Line> = all_lines
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .cloned()
        .collect();

    let view = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Logs ({} total)", total)),
    );

    let scroll_area = ratatui::layout::Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y + 1,
        width: 1,
        height: area.height.saturating_sub(2),
    };

    (view, total, start, visible, scroll_area)
}

fn build_log_lines(
    entries: &VecDeque<LogLine>,
    content_width: usize,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    light_theme: bool,
) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for entry in entries.iter() {
        if level_rank(entry.level) < filter_rank(log_level_filter) {
            continue;
        }
        let (label, color) = fmt_level(entry.level, light_theme);
        let source: &str = &entry.source;
        let message: &str = &entry.message;
        let meta_prefix = if show_timestamp {
            format!("[{}] {} {} ", label, entry.timestamp, source)
        } else {
            format!("[{}] {} ", label, source)
        };
        let meta_len = meta_prefix.chars().count();
        let cont_prefix = " ".repeat(meta_len);

        let mut msg_iter = message.lines().peekable();
        if msg_iter.peek().is_none() {
            lines.push(Line::from(vec![
                Span::styled(format!("[{}]", label), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(source, Style::default().fg(info_color(light_theme))),
            ]));
            continue;
        }

        for (idx, raw_line) in msg_iter.enumerate() {
            let indent_len = raw_line.chars().take_while(|c| *c == ' ').count();
            let indent = " ".repeat(indent_len);
            let content = raw_line[indent_len..].to_string();

            let base_len = if idx == 0 {
                meta_len
            } else {
                cont_prefix.chars().count()
            };
            let wrap_width = content_width.saturating_sub(base_len + indent_len).max(1);

            for (seg_idx, segment) in wrap_text(&content, wrap_width).into_iter().enumerate() {
                let prefix = if idx == 0 && seg_idx == 0 {
                    let mut spans = vec![Span::styled(
                        format!("[{}]", label),
                        Style::default().fg(color),
                    )];
                    spans.push(Span::raw(" "));
                    if show_timestamp {
                        spans.push(Span::styled(
                            &entry.timestamp,
                            Style::default().fg(timestamp_color(light_theme)),
                        ));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        source,
                        Style::default().fg(info_color(light_theme)),
                    ));
                    spans.push(Span::raw(" "));
                    spans
                } else {
                    vec![Span::raw(cont_prefix.clone())]
                };

                let mut spans = prefix;
                spans.push(Span::raw(indent.clone()));
                spans.push(Span::raw(segment));
                lines.push(Line::from(spans));
            }
        }
    }

    lines
}

fn count_log_lines(
    entries: &VecDeque<LogLine>,
    content_width: usize,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    light_theme: bool,
) -> usize {
    let mut count = 0usize;
    for entry in entries {
        if level_rank(entry.level) < filter_rank(log_level_filter) {
            continue;
        }
        let (label, _) = fmt_level(entry.level, light_theme);
        let source: &str = &entry.source;
        let message: &str = &entry.message;
        let meta_prefix = if show_timestamp {
            format!("[{}] {} {} ", label, entry.timestamp, source)
        } else {
            format!("[{}] {} ", label, source)
        };
        let meta_len = meta_prefix.chars().count();
        let cont_len = meta_len;
        let mut msg_iter = message.lines().peekable();
        if msg_iter.peek().is_none() {
            count += 1;
            continue;
        }
        for (idx, raw_line) in msg_iter.enumerate() {
            let indent_len = raw_line.chars().take_while(|c| *c == ' ').count();
            let content = raw_line[indent_len..].to_string();
            let base_len = if idx == 0 { meta_len } else { cont_len };
            let wrap_width = content_width.saturating_sub(base_len + indent_len).max(1);
            let wrapped = wrap_text(&content, wrap_width);
            count += wrapped.len().max(1);
        }
    }
    count
}

fn log_max_start_for_content(
    content_area: Rect,
    entries: &VecDeque<LogLine>,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    light_theme: bool,
) -> usize {
    let visible = content_area.height.saturating_sub(2) as usize;
    let width = content_area.width.saturating_sub(3) as usize;
    let total = count_log_lines(
        entries,
        width,
        show_timestamp,
        log_level_filter,
        light_theme,
    );
    total.saturating_sub(visible)
}

fn sync_log_tail_from_scroll(
    content_area: Rect,
    entries: &VecDeque<LogLine>,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    light_theme: bool,
    log_tail: &mut bool,
) -> bool {
    let max_start = log_max_start_for_content(
        content_area,
        entries,
        show_timestamp,
        log_level_filter,
        light_theme,
    );
    let cur = TUI_LOG_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let clamped = cur.min(max_start);
    let mut changed = false;

    if clamped != cur {
        TUI_LOG_SCROLL.store(clamped, std::sync::atomic::Ordering::Relaxed);
        changed = true;
    }

    let at_bottom = clamped >= max_start;
    if *log_tail != at_bottom {
        *log_tail = at_bottom;
        changed = true;
    }

    changed
}

fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![input.to_string()];
    }
    let mut out = Vec::new();
    let mut remaining = input;
    while !remaining.is_empty() {
        if remaining.chars().count() <= width {
            out.push(remaining.to_string());
            break;
        }
        let mut cut = width;
        let mut iter = remaining.char_indices().take(width + 1).collect::<Vec<_>>();
        if let Some((idx, _)) = iter.pop() {
            cut = idx;
        }
        let slice = &remaining[..cut];
        let mut split_at = slice.rfind(' ').unwrap_or(cut);
        if split_at == 0 {
            split_at = cut;
        }
        let chunk = remaining[..split_at].trim_end().to_string();
        out.push(chunk);
        remaining = remaining[split_at..].trim_start();
    }
    out
}

fn setup_terminal() -> io::Result<Terminal<ratatui::backend::CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();
}

fn detect_light_terminal_from_env() -> bool {
    std::env::var("COLORFGBG")
        .ok()
        .and_then(|value| {
            value
                .rsplit(';')
                .next()
                .and_then(|bg| bg.parse::<u8>().ok())
        })
        .map(|bg| bg == 7 || bg == 15)
        .unwrap_or(false)
}

fn resolve_light_theme(theme_arg: Option<&str>) -> bool {
    match theme_arg {
        Some("light") => true,
        Some("dark") => false,
        _ => detect_light_terminal_from_env(),
    }
}

pub async fn run(global_state: Arc<GlobalState>, theme_arg: Option<String>) {
    let mut terminal = match setup_terminal() {
        Ok(t) => t,
        Err(_) => return,
    };
    let light_theme = resolve_light_theme(theme_arg.as_deref());

    let refresh_every = Duration::from_millis(500);
    let mut last_refresh = Instant::now() - refresh_every;
    let mut data = build_snapshot(&global_state).await;
    let mut dirty = true;
    let mut page = TuiPage::Sites;
    let mut log_entries: VecDeque<LogLine> = VecDeque::new();
    let log_limit = 1000usize;
    let mut log_tail = true;
    let mut log_show_timestamp = true;
    let mut log_level_filter =
        LogLevelFilter::from_log_level(global_state.config.load_full().log_level.clone());
    let mut confirm_quit = false;
    let mut hovered_row: Option<usize> = None;
    let mut drag_scroll: Option<TuiPage> = None;

    loop {
        if global_state.exit.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        if last_refresh.elapsed() >= refresh_every {
            data = build_snapshot(&global_state).await;
            let new_logs = global_state.tui_log_buffer.drain();
            if !new_logs.is_empty() {
                for msg in new_logs {
                    let source = if let Some(thread) = msg.thread.as_ref() {
                        if !thread.is_empty() {
                            thread.clone()
                        } else {
                            msg.src.clone()
                        }
                    } else {
                        msg.src.clone()
                    };
                    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f").to_string();
                    log_entries.push_back(LogLine {
                        level: msg.lvl,
                        source,
                        timestamp,
                        message: msg.msg,
                    });
                }
                while log_entries.len() > log_limit {
                    log_entries.pop_front();
                }

                // When tail mode is on, always pin to the newest line. Manual
                // scrolling already disables tail mode, so we do not need a
                // separate "was at bottom" heuristic here.
                if page == TuiPage::Logs && log_tail {
                    let visible = terminal
                        .size()
                        .map(|s| s.height.saturating_sub(3 + 3 + 2).saturating_sub(2) as usize)
                        .unwrap_or(0);
                    let width = terminal
                        .size()
                        .map(|s| s.width.saturating_sub(3) as usize)
                        .unwrap_or(0);
                    let total_lines = count_log_lines(
                        &log_entries,
                        width,
                        log_show_timestamp,
                        log_level_filter,
                        light_theme,
                    );
                    let max_start = total_lines.saturating_sub(visible);
                    TUI_LOG_SCROLL.store(max_start, std::sync::atomic::Ordering::Relaxed);
                }
            }
            last_refresh = Instant::now();
            dirty = true;
        }
        if dirty {
            let capture_snap = global_state.http_capture_store.snapshot();
            let traffic_on = global_state
                .enable_global_traffic_inspection
                .load(std::sync::atomic::Ordering::Relaxed);
            let _ = terminal.draw(|f| {
                draw_ui(
                    f,
                    &data,
                    page,
                    light_theme,
                    &log_entries,
                    TUI_LOG_SCROLL.load(std::sync::atomic::Ordering::Relaxed),
                    log_tail,
                    log_show_timestamp,
                    log_level_filter,
                    confirm_quit,
                    hovered_row,
                    &capture_snap,
                    traffic_on,
                    &global_state.http_capture_store,
                )
            });
            dirty = false;
        }

        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(event) = event::read() {
                match event {
                    Event::Mouse(mouse) => {
                        if confirm_quit {
                            continue;
                        }
                        let size = terminal.size().unwrap_or_default();
                        let root = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(TUI_HEADER_HEIGHT),
                                Constraint::Min(0),
                                Constraint::Length(TUI_FOOTER_HEIGHT),
                            ])
                            .split(Rect::new(0, 0, size.width, size.height));
                        let content_area = root[1];
                        match mouse.kind {
                            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                                if mouse.row < content_area.y
                                    || mouse.row
                                        >= content_area.y.saturating_add(content_area.height)
                                {
                                    continue;
                                }
                                let delta: isize = if matches!(mouse.kind, MouseEventKind::ScrollUp)
                                {
                                    -3
                                } else {
                                    3
                                };
                                match page {
                                    TuiPage::Sites => {
                                        let rows = build_rows(&data, light_theme);
                                        let total = rows.len();
                                        let visible =
                                            content_area.height.saturating_sub(3) as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let cur = TUI_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as isize;
                                        let next =
                                            (cur + delta).clamp(0, max_start as isize) as usize;
                                        if next != cur as usize {
                                            TUI_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Docker => {
                                        let total = data.docker_discovered.len();
                                        let visible =
                                            content_area.height.saturating_sub(3) as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let cur = TUI_DOCKER_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as isize;
                                        let next =
                                            (cur + delta).clamp(0, max_start as isize) as usize;
                                        if next != cur as usize {
                                            TUI_DOCKER_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Traffic => {
                                        let detailed = TRAFFIC_DETAILED_MODE
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        let zoom = TRAFFIC_ZOOM_MODE
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        let total = if detailed || zoom {
                                            // Use cached line count from last render
                                            TRAFFIC_TOTAL_LINES
                                                .load(std::sync::atomic::Ordering::Relaxed)
                                        } else {
                                            let snap = global_state.http_capture_store.snapshot();
                                            snap.order.len()
                                        };
                                        let visible = content_area
                                            .height
                                            .saturating_sub(if detailed || zoom { 2 } else { 3 })
                                            as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let cur = TUI_TRAFFIC_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as isize;
                                        let next =
                                            (cur + delta).clamp(0, max_start as isize) as usize;
                                        if next != cur as usize {
                                            TUI_TRAFFIC_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Logs => {
                                        let max_start = log_max_start_for_content(
                                            content_area,
                                            &log_entries,
                                            log_show_timestamp,
                                            log_level_filter,
                                            light_theme,
                                        );
                                        let cur = TUI_LOG_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as isize;
                                        let next =
                                            (cur + delta).clamp(0, max_start as isize) as usize;
                                        if next != cur as usize {
                                            TUI_LOG_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                        if sync_log_tail_from_scroll(
                                            content_area,
                                            &log_entries,
                                            log_show_timestamp,
                                            log_level_filter,
                                            light_theme,
                                            &mut log_tail,
                                        ) {
                                            dirty = true;
                                        }
                                    }
                                }
                            }
                            MouseEventKind::Moved => {
                                if page != TuiPage::Sites
                                    && page != TuiPage::Docker
                                    && page != TuiPage::Traffic
                                {
                                    continue;
                                }
                                if drag_scroll.is_some() {
                                    continue;
                                }
                                let new_hover = if page == TuiPage::Sites {
                                    let rows = build_rows(&data, light_theme);
                                    let total = rows.len();
                                    hit_test_site_row(content_area, mouse.row, total).map(|row| row)
                                } else {
                                    let rows = sorted_docker_rows(&data);
                                    let total = rows.len();
                                    hit_test_docker_row(content_area, mouse.row, total)
                                        .map(|row| row)
                                };
                                if new_hover != hovered_row {
                                    hovered_row = new_hover;
                                    dirty = true;
                                }
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                if matches!(
                                    page,
                                    TuiPage::Sites
                                        | TuiPage::Docker
                                        | TuiPage::Logs
                                        | TuiPage::Traffic
                                ) {
                                    let scroll_area = scroll_area_for_content(content_area);
                                    if mouse.column == scroll_area.x
                                        && mouse.row >= scroll_area.y
                                        && mouse.row
                                            < scroll_area.y.saturating_add(scroll_area.height)
                                    {
                                        drag_scroll = Some(page);
                                        match page {
                                            TuiPage::Sites => {
                                                let rows = build_rows(&data, light_theme);
                                                let total = rows.len();
                                                let visible =
                                                    content_area.height.saturating_sub(3) as usize;
                                                let max_start = total.saturating_sub(visible);
                                                let next = scroll_pos_from_mouse(
                                                    scroll_area,
                                                    mouse.row,
                                                    max_start,
                                                );
                                                let cur = TUI_SCROLL
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                if next != cur {
                                                    TUI_SCROLL.store(
                                                        next,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    dirty = true;
                                                }
                                            }
                                            TuiPage::Docker => {
                                                let total = data.docker_discovered.len();
                                                let visible =
                                                    content_area.height.saturating_sub(3) as usize;
                                                let max_start = total.saturating_sub(visible);
                                                let next = scroll_pos_from_mouse(
                                                    scroll_area,
                                                    mouse.row,
                                                    max_start,
                                                );
                                                let cur = TUI_DOCKER_SCROLL
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                if next != cur {
                                                    TUI_DOCKER_SCROLL.store(
                                                        next,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    dirty = true;
                                                }
                                            }
                                            TuiPage::Logs => {
                                                let max_start = log_max_start_for_content(
                                                    content_area,
                                                    &log_entries,
                                                    log_show_timestamp,
                                                    log_level_filter,
                                                    light_theme,
                                                );
                                                let next = scroll_pos_from_mouse(
                                                    scroll_area,
                                                    mouse.row,
                                                    max_start,
                                                );
                                                let cur = TUI_LOG_SCROLL
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                if next != cur {
                                                    TUI_LOG_SCROLL.store(
                                                        next,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    dirty = true;
                                                }
                                                if sync_log_tail_from_scroll(
                                                    content_area,
                                                    &log_entries,
                                                    log_show_timestamp,
                                                    log_level_filter,
                                                    light_theme,
                                                    &mut log_tail,
                                                ) {
                                                    dirty = true;
                                                }
                                            }
                                            TuiPage::Traffic => {
                                                let detailed = TRAFFIC_DETAILED_MODE
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                let zoom = TRAFFIC_ZOOM_MODE
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                let total = if detailed || zoom {
                                                    TRAFFIC_TOTAL_LINES
                                                        .load(std::sync::atomic::Ordering::Relaxed)
                                                } else {
                                                    let snap =
                                                        global_state.http_capture_store.snapshot();
                                                    snap.order.len()
                                                };
                                                let visible = content_area.height.saturating_sub(
                                                    if detailed || zoom { 2 } else { 3 },
                                                )
                                                    as usize;
                                                let max_start = total.saturating_sub(visible);
                                                let next = scroll_pos_from_mouse(
                                                    scroll_area,
                                                    mouse.row,
                                                    max_start,
                                                );
                                                let cur = TUI_TRAFFIC_SCROLL
                                                    .load(std::sync::atomic::Ordering::Relaxed);
                                                if next != cur {
                                                    TUI_TRAFFIC_SCROLL.store(
                                                        next,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    dirty = true;
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                }
                                if page != TuiPage::Sites {
                                    continue;
                                }
                                let rows = build_rows(&data, light_theme);
                                let total = rows.len();
                                if let Some(row_idx) =
                                    hit_test_site_row(content_area, mouse.row, total)
                                {
                                    if let Some(row) = rows.get(row_idx) {
                                        if let Some(proc_id) = row.process_id.as_ref() {
                                            let mut next_state: Option<ProcState> = None;
                                            match row.state {
                                                ProcState::Running => {
                                                    global_state.process_registry.update_state(
                                                        proc_id,
                                                        ProcState::Stopping,
                                                        None,
                                                        None,
                                                        Some(false),
                                                        None,
                                                        None,
                                                        None,
                                                        None,
                                                    );
                                                    next_state = Some(ProcState::Stopping);
                                                }
                                                ProcState::Stopped => {
                                                    global_state.process_registry.update_state(
                                                        proc_id,
                                                        ProcState::Starting,
                                                        None,
                                                        None,
                                                        Some(true),
                                                        None,
                                                        None,
                                                        None,
                                                        None,
                                                    );
                                                    next_state = Some(ProcState::Starting);
                                                }
                                                ProcState::Faulty => {
                                                    global_state.process_registry.update_state(
                                                        proc_id,
                                                        ProcState::Stopping,
                                                        None,
                                                        None,
                                                        Some(false),
                                                        None,
                                                        None,
                                                        None,
                                                        None,
                                                    );
                                                    next_state = Some(ProcState::Stopping);
                                                }
                                                _ => {}
                                            }
                                            if let Some(state) = next_state {
                                                if let Some(entry) = data
                                                    .processes
                                                    .iter_mut()
                                                    .find(|(n, _, _, _, _, _)| n == proc_id)
                                                {
                                                    entry.1 = state;
                                                }
                                                dirty = true;
                                            }
                                        }
                                    }
                                }
                            }
                            MouseEventKind::Drag(MouseButton::Left) => {
                                let Some(target_page) = drag_scroll else {
                                    continue;
                                };
                                match target_page {
                                    TuiPage::Sites => {
                                        let rows = build_rows(&data, light_theme);
                                        let total = rows.len();
                                        let visible =
                                            content_area.height.saturating_sub(3) as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let scroll_area = scroll_area_for_content(content_area);
                                        let next = scroll_pos_from_mouse(
                                            scroll_area,
                                            mouse.row,
                                            max_start,
                                        );
                                        let cur =
                                            TUI_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
                                        if next != cur {
                                            TUI_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Traffic => {
                                        let detailed = TRAFFIC_DETAILED_MODE
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        let zoom = TRAFFIC_ZOOM_MODE
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        let total = if detailed || zoom {
                                            TRAFFIC_TOTAL_LINES
                                                .load(std::sync::atomic::Ordering::Relaxed)
                                        } else {
                                            let snap = global_state.http_capture_store.snapshot();
                                            snap.order.len()
                                        };
                                        let visible = content_area
                                            .height
                                            .saturating_sub(if detailed || zoom { 2 } else { 3 })
                                            as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let scroll_area = scroll_area_for_content(content_area);
                                        let next = scroll_pos_from_mouse(
                                            scroll_area,
                                            mouse.row,
                                            max_start,
                                        );
                                        let cur = TUI_TRAFFIC_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        if next != cur {
                                            TUI_TRAFFIC_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Docker => {
                                        let total = data.docker_discovered.len();
                                        let visible =
                                            content_area.height.saturating_sub(3) as usize;
                                        let max_start = total.saturating_sub(visible);
                                        let scroll_area = scroll_area_for_content(content_area);
                                        let next = scroll_pos_from_mouse(
                                            scroll_area,
                                            mouse.row,
                                            max_start,
                                        );
                                        let cur = TUI_DOCKER_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        if next != cur {
                                            TUI_DOCKER_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                    }
                                    TuiPage::Logs => {
                                        let max_start = log_max_start_for_content(
                                            content_area,
                                            &log_entries,
                                            log_show_timestamp,
                                            log_level_filter,
                                            light_theme,
                                        );
                                        let scroll_area = scroll_area_for_content(content_area);
                                        let next = scroll_pos_from_mouse(
                                            scroll_area,
                                            mouse.row,
                                            max_start,
                                        );
                                        let cur = TUI_LOG_SCROLL
                                            .load(std::sync::atomic::Ordering::Relaxed);
                                        if next != cur {
                                            TUI_LOG_SCROLL
                                                .store(next, std::sync::atomic::Ordering::Relaxed);
                                            dirty = true;
                                        }
                                        if sync_log_tail_from_scroll(
                                            content_area,
                                            &log_entries,
                                            log_show_timestamp,
                                            log_level_filter,
                                            light_theme,
                                            &mut log_tail,
                                        ) {
                                            dirty = true;
                                        }
                                    }
                                }
                            }
                            MouseEventKind::Up(MouseButton::Left) => {
                                drag_scroll = None;
                            }
                            _ => {}
                        }
                    }
                    Event::Key(key) => {
                        let key_content_area = {
                            let size = terminal.size().unwrap_or_default();
                            let root = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([
                                    Constraint::Length(TUI_HEADER_HEIGHT),
                                    Constraint::Min(0),
                                    Constraint::Length(TUI_FOOTER_HEIGHT),
                                ])
                                .split(Rect::new(0, 0, size.width, size.height));
                            root[1]
                        };
                        if confirm_quit {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    restore_terminal(&mut terminal);
                                    return;
                                }
                                KeyCode::Char('n')
                                | KeyCode::Char('N')
                                | KeyCode::Esc
                                | KeyCode::Enter => {
                                    confirm_quit = false;
                                    dirty = true;
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if key.code == KeyCode::Char('q')
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            if key.code == KeyCode::Char('q') {
                                confirm_quit = true;
                                dirty = true;
                                continue;
                            }
                            restore_terminal(&mut terminal);
                            return;
                        } else if key.code == KeyCode::Tab {
                            page = match page {
                                TuiPage::Sites => TuiPage::Docker,
                                TuiPage::Docker => TuiPage::Logs,
                                TuiPage::Logs => TuiPage::Traffic,
                                TuiPage::Traffic => TuiPage::Sites,
                            };
                            if page == TuiPage::Logs {
                                let visible = terminal
                                    .size()
                                    .map(|s| {
                                        s.height
                                            .saturating_sub(TUI_HEADER_HEIGHT + TUI_FOOTER_HEIGHT)
                                            .saturating_sub(2)
                                            as usize
                                    })
                                    .unwrap_or(0);
                                let width = terminal
                                    .size()
                                    .map(|s| s.width.saturating_sub(3) as usize)
                                    .unwrap_or(0);
                                let total_lines = count_log_lines(
                                    &log_entries,
                                    width,
                                    log_show_timestamp,
                                    log_level_filter,
                                    light_theme,
                                );
                                let max_start = total_lines.saturating_sub(visible);
                                TUI_LOG_SCROLL
                                    .store(max_start, std::sync::atomic::Ordering::Relaxed);
                            }
                            hovered_row = None;
                            dirty = true;
                        } else if page == TuiPage::Sites && key.code == KeyCode::Char('s') {
                            let mut changed = false;
                            for (name, state, _, _, _, exclude_from_start_all) in
                                data.processes.iter_mut()
                            {
                                if *exclude_from_start_all {
                                    continue;
                                }
                                if matches!(state, ProcState::Stopped | ProcState::Faulty) {
                                    global_state.process_registry.update_state(
                                        name,
                                        ProcState::Starting,
                                        None,
                                        None,
                                        Some(true),
                                        None,
                                        None,
                                        None,
                                        None,
                                    );
                                    *state = ProcState::Starting;
                                    changed = true;
                                }
                            }
                            if changed {
                                dirty = true;
                            }
                        } else if page == TuiPage::Traffic && key.code == KeyCode::Char('i') {
                            let prev = global_state
                                .enable_global_traffic_inspection
                                .load(std::sync::atomic::Ordering::Relaxed);
                            let next = !prev;
                            global_state
                                .enable_global_traffic_inspection
                                .store(next, std::sync::atomic::Ordering::Relaxed);
                            global_state.http_capture_store.set_enabled(next);
                            dirty = true;
                        } else if page == TuiPage::Traffic && key.code == KeyCode::Char('d') {
                            let was_detailed =
                                TRAFFIC_DETAILED_MODE.load(std::sync::atomic::Ordering::Relaxed);
                            let new_val = !was_detailed;
                            TRAFFIC_DETAILED_MODE
                                .store(new_val, std::sync::atomic::Ordering::Relaxed);
                            if !new_val {
                                // Turning off detailed mode also turns off zoom
                                TRAFFIC_ZOOM_MODE
                                    .store(false, std::sync::atomic::Ordering::Relaxed);
                            }
                            TUI_TRAFFIC_SCROLL.store(0, std::sync::atomic::Ordering::Relaxed);
                            dirty = true;
                        } else if page == TuiPage::Traffic && key.code == KeyCode::Char('z') {
                            if TRAFFIC_DETAILED_MODE.load(std::sync::atomic::Ordering::Relaxed) {
                                let was_zoom =
                                    TRAFFIC_ZOOM_MODE.load(std::sync::atomic::Ordering::Relaxed);
                                TRAFFIC_ZOOM_MODE
                                    .store(!was_zoom, std::sync::atomic::Ordering::Relaxed);
                                TUI_TRAFFIC_SCROLL.store(0, std::sync::atomic::Ordering::Relaxed);
                                dirty = true;
                            }
                        } else if page == TuiPage::Traffic && key.code == KeyCode::Char('c') {
                            global_state.http_capture_store.clear();
                            TUI_TRAFFIC_SCROLL.store(0, std::sync::atomic::Ordering::Relaxed);
                            dirty = true;
                        } else if page == TuiPage::Sites && key.code == KeyCode::Char('p') {
                            let prev = SHOW_FULL_PATH.load(std::sync::atomic::Ordering::Relaxed);
                            SHOW_FULL_PATH.store(!prev, std::sync::atomic::Ordering::Relaxed);
                            dirty = true;
                        } else if page == TuiPage::Sites && key.code == KeyCode::Char('x') {
                            let mut changed = false;
                            for (name, state, _, _, _, _) in data.processes.iter_mut() {
                                if matches!(
                                    state,
                                    ProcState::Running | ProcState::Starting | ProcState::Faulty
                                ) {
                                    global_state.process_registry.update_state(
                                        name,
                                        ProcState::Stopping,
                                        None,
                                        None,
                                        Some(false),
                                        None,
                                        None,
                                        None,
                                        None,
                                    );
                                    *state = ProcState::Stopping;
                                    changed = true;
                                }
                            }
                            if changed {
                                dirty = true;
                            }
                        } else if page == TuiPage::Logs && key.code == KeyCode::Enter {
                            log_tail = true;
                            let visible = terminal
                                .size()
                                .map(|s| {
                                    s.height.saturating_sub(3 + 3 + 2).saturating_sub(2) as usize
                                })
                                .unwrap_or(0);
                            let width = terminal
                                .size()
                                .map(|s| s.width.saturating_sub(3) as usize)
                                .unwrap_or(0);
                            let total_lines = count_log_lines(
                                &log_entries,
                                width,
                                log_show_timestamp,
                                log_level_filter,
                                light_theme,
                            );
                            let max_start = total_lines.saturating_sub(visible);
                            TUI_LOG_SCROLL.store(max_start, std::sync::atomic::Ordering::Relaxed);
                            dirty = true;
                        } else if page == TuiPage::Logs && key.code == KeyCode::Char('f') {
                            log_tail = !log_tail;
                            if log_tail {
                                let visible = terminal
                                    .size()
                                    .map(|s| {
                                        s.height.saturating_sub(3 + 3 + 2).saturating_sub(2)
                                            as usize
                                    })
                                    .unwrap_or(0);
                                let width = terminal
                                    .size()
                                    .map(|s| s.width.saturating_sub(3) as usize)
                                    .unwrap_or(0);
                                let total_lines = count_log_lines(
                                    &log_entries,
                                    width,
                                    log_show_timestamp,
                                    log_level_filter,
                                    light_theme,
                                );
                                let max_start = total_lines.saturating_sub(visible);
                                TUI_LOG_SCROLL
                                    .store(max_start, std::sync::atomic::Ordering::Relaxed);
                            }
                            dirty = true;
                        } else if page == TuiPage::Logs && key.code == KeyCode::Char('t') {
                            log_show_timestamp = !log_show_timestamp;
                            dirty = true;
                        } else if page == TuiPage::Logs && key.code == KeyCode::Char('l') {
                            let next_level = log_level_filter.next();
                            if apply_tui_log_level(&global_state, next_level).await {
                                log_level_filter = next_level;
                            }
                            let visible = terminal
                                .size()
                                .map(|s| {
                                    s.height.saturating_sub(3 + 3 + 2).saturating_sub(2) as usize
                                })
                                .unwrap_or(0);
                            let width = terminal
                                .size()
                                .map(|s| s.width.saturating_sub(3) as usize)
                                .unwrap_or(0);
                            let total_lines = count_log_lines(
                                &log_entries,
                                width,
                                log_show_timestamp,
                                log_level_filter,
                                light_theme,
                            );
                            let max_start = total_lines.saturating_sub(visible);
                            TUI_LOG_SCROLL.store(max_start, std::sync::atomic::Ordering::Relaxed);
                            dirty = true;
                        } else if page == TuiPage::Logs && key.code == KeyCode::Char('c') {
                            global_state.tui_log_buffer.clear();
                            log_entries.clear();
                            TUI_LOG_SCROLL.store(0, std::sync::atomic::Ordering::Relaxed);
                            log_tail = true;
                            dirty = true;
                        } else {
                            let target_scroll = match page {
                                TuiPage::Sites => &TUI_SCROLL,
                                TuiPage::Docker => &TUI_DOCKER_SCROLL,
                                TuiPage::Logs => &TUI_LOG_SCROLL,
                                TuiPage::Traffic => &TUI_TRAFFIC_SCROLL,
                            };
                            let mut logs_scroll_input = false;
                            if key.code == KeyCode::Up {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                target_scroll.store(
                                    cur.saturating_sub(1),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                logs_scroll_input = page == TuiPage::Logs;
                                dirty = true;
                            } else if key.code == KeyCode::Down {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                target_scroll.store(
                                    cur.saturating_add(1),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                logs_scroll_input = page == TuiPage::Logs;
                                dirty = true;
                            } else if key.code == KeyCode::PageUp {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                let page_size = terminal
                                    .size()
                                    .map(|s| {
                                        s.height.saturating_sub(3 + 3 + 2).saturating_sub(2)
                                            as usize
                                    })
                                    .unwrap_or(10);
                                let jump = (page_size / 2).max(1);
                                target_scroll.store(
                                    cur.saturating_sub(jump),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                logs_scroll_input = page == TuiPage::Logs;
                                dirty = true;
                            } else if key.code == KeyCode::PageDown {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                let page_size = terminal
                                    .size()
                                    .map(|s| {
                                        s.height.saturating_sub(3 + 3 + 2).saturating_sub(2)
                                            as usize
                                    })
                                    .unwrap_or(10);
                                let jump = (page_size / 2).max(1);
                                target_scroll.store(
                                    cur.saturating_add(jump),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                logs_scroll_input = page == TuiPage::Logs;
                                dirty = true;
                            }
                            if logs_scroll_input
                                && sync_log_tail_from_scroll(
                                    key_content_area,
                                    &log_entries,
                                    log_show_timestamp,
                                    log_level_filter,
                                    light_theme,
                                    &mut log_tail,
                                )
                            {
                                dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    restore_terminal(&mut terminal);
    let _ = terminal::disable_raw_mode();
}

fn build_tui_env_filter(log_level: LogLevel) -> EnvFilter {
    let rust_log = std::env::var("RUST_LOG").ok();
    let has_odd_box_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box")))
        .unwrap_or(false);
    let has_cruma_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box::cruma")))
        .unwrap_or(false);

    let mut what = EnvFilter::from_default_env();
    if !has_odd_box_override {
        let level_str = match log_level {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        };
        what = what.add_directive(
            format!("odd_box={}", level_str)
                .parse()
                .expect("This directive should always work"),
        );
    }
    what = what.add_directive(
        "cruma_proc_host=trace"
            .parse()
            .expect("This directive should always work"),
    );
    if !has_odd_box_override && !has_cruma_override {
        what = what.add_directive(
            "odd_box::cruma=info"
                .parse()
                .expect("This directive should always work"),
        );
    }
    what
}

async fn apply_tui_log_level(state: &GlobalState, filter: LogLevelFilter) -> bool {
    let log_level = filter.to_log_level();
    let what = build_tui_env_filter(log_level);

    match &state.log_handle {
        crate::OddLogHandle::CLI(rw_lock) => match rw_lock.write().await.reload(what) {
            Ok(_) => true,
            Err(e) => {
                tracing::error!("failed to change log level due to error {e:?}");
                false
            }
        },
        crate::OddLogHandle::TUI(rw_lock) => match rw_lock.write().await.reload(what) {
            Ok(_) => true,
            Err(e) => {
                tracing::error!("failed to change log level due to error {e:?}");
                false
            }
        },
        crate::OddLogHandle::None => {
            tracing::error!("NO LOG HANDLE EXISTS!!");
            false
        }
    }
}
