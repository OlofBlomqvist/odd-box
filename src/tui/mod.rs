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
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
    Table, TableState,
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

fn fmt_state(state: ProcState) -> (&'static str, Color) {
    match state {
        ProcState::Running => ("running", Color::Green),
        ProcState::Starting => ("starting", Color::Yellow),
        ProcState::Stopping => ("stopping", Color::Yellow),
        ProcState::Stopped => ("stopped", Color::Gray),
        ProcState::Faulty => ("faulty", Color::Red),
        ProcState::Remote => ("remote", Color::Cyan),
        ProcState::DirServer => ("dir", Color::LightBlue),
        ProcState::Docker => ("docker", Color::Magenta),
    }
}

fn fmt_level(level: Level) -> (&'static str, Color) {
    match level {
        Level::TRACE => ("TRC", Color::Gray),
        Level::DEBUG => ("DBG", Color::Blue),
        Level::INFO => ("INF", Color::Green),
        Level::WARN => ("WRN", Color::Yellow),
        Level::ERROR => ("ERR", Color::Red),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TuiPage {
    Sites,
    Logs,
}

const TUI_HEADER_HEIGHT: u16 = 6;
const TUI_FOOTER_HEIGHT: u16 = 2;

struct Snapshot {
    version: String,
    cruma_fqdn: String,
    cruma_motd: String,
    listen_ports: String,
    processes: Vec<(String, ProcState, String, String)>,
    remotes: Vec<(String, ProcState, String)>,
    statics: Vec<(String, ProcState, String)>,
    docker: Vec<(String, ProcState, String)>,
    routes: Vec<(String, String, bool)>,
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
            let bin = kv.value().bin.clone();
            let handle = snapshot.get(&backend_id);
            let state = handle.map(|h| h.proc_state()).unwrap_or(ProcState::Stopped);
            let port = handle
                .and_then(|h| h.active_port())
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            (backend_id, state, bin, port)
        })
        .collect();

    let remotes = cfg
        .remote_sites
        .iter()
        .map(|kv| {
            let backend_id = kv.key().clone();
            let state = snapshot.state_of(&backend_id).unwrap_or(ProcState::Remote);
            let remote = kv.value();
            let detail = if remote.endpoints.len() == 1 {
                let ep = &remote.endpoints[0];
                let scheme = if remote.https { "https" } else { "http" };
                format!("{}:{} ({})", ep.addr, ep.port, scheme)
            } else {
                format!("{} endpoints", remote.endpoints.len())
            };
            (backend_id, state, detail)
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
            (host, state, cont.image_name)
        })
        .collect();

    let mut routes: Vec<(String, String, bool)> = Vec::new();
    if let Some(http) = &cfg.frontends.http {
        for (host, target) in &http.routes {
            routes.push((host.clone(), target.backend_id().to_string(), false));
        }
    }
    if let Some(https) = &cfg.frontends.https {
        if let Some(crate::configuration::HttpsRoutes::Explicit(explicit)) = &https.routes {
            for (host, target) in explicit {
                if !routes.iter().any(|(h, _, _)| h == host) {
                    routes.push((host.clone(), target.backend_id().to_string(), true));
                }
            }
        }
    }

    Snapshot {
        version,
        cruma_fqdn,
        cruma_motd,
        listen_ports,
        processes,
        remotes,
        statics,
        docker,
        routes,
    }
}

fn draw_ui(
    f: &mut Frame<'_>,
    data: &Snapshot,
    page: TuiPage,
    log_entries: &VecDeque<LogLine>,
    log_scroll: usize,
    log_tail: bool,
    log_show_timestamp: bool,
    log_level_filter: LogLevelFilter,
    confirm_quit: bool,
    hovered_row: Option<usize>,
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
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::raw(match page {
            TuiPage::Sites => "TUI (sites)",
            TuiPage::Logs => "TUI (logs)",
        }),
    ]));

    let status = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Cruma FQDN: ", Style::default().fg(Color::Gray)),
            Span::raw(&data.cruma_fqdn),
        ]),
        Line::from(vec![
            Span::styled("Cruma MOTD: ", Style::default().fg(Color::Gray)),
            Span::raw(&data.cruma_motd),
        ]),
        Line::from(vec![
            Span::styled("Listen: ", Style::default().fg(Color::Gray)),
            Span::raw(&data.listen_ports),
        ]),
    ]);

    f.render_widget(header, header_rows[0]);
    f.render_widget(status, header_rows[1]);

    match page {
        TuiPage::Sites => {
            let (table, mut state, total, start, visible, scroll_area) =
                build_flat_table(data, root[1], hovered_row);
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
    }

    let mut footer_spans = vec![
        Span::styled("↑/↓", Style::default().fg(Color::Gray)),
        Span::raw(" scroll  "),
        Span::styled("tab", Style::default().fg(Color::Gray)),
        Span::raw(" switch  "),
    ];
    if page == TuiPage::Logs {
        footer_spans.extend([
            Span::styled("f", Style::default().fg(Color::Gray)),
            Span::raw(if log_tail {
                " tail:on  "
            } else {
                " tail:off  "
            }),
            Span::styled("t", Style::default().fg(Color::Gray)),
            Span::raw(if log_show_timestamp {
                " ts:on  "
            } else {
                " ts:off  "
            }),
            Span::styled("l", Style::default().fg(Color::Gray)),
            Span::raw(format!(" lvl:{}  ", log_level_filter.label())),
            Span::styled("c", Style::default().fg(Color::Gray)),
            Span::raw(" clear  "),
        ]);
    } else {
        footer_spans.extend([
            Span::styled("s", Style::default().fg(Color::Gray)),
            Span::raw(" start all  "),
            Span::styled("x", Style::default().fg(Color::Gray)),
            Span::raw(" stop all  "),
        ]);
    }
    footer_spans.extend([
        Span::styled("q", Style::default().fg(Color::Gray)),
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
    detail: String,
    muted: bool,
    alert: bool,
}

fn build_rows(data: &Snapshot) -> Vec<RowData> {
    let mut rows = Vec::new();
    let mut backend_ids: Vec<String> = Vec::new();
    backend_ids.extend(data.processes.iter().map(|(n, _, _, _)| n.clone()));
    backend_ids.extend(data.remotes.iter().map(|(n, _, _)| n.clone()));
    backend_ids.extend(data.statics.iter().map(|(n, _, _)| n.clone()));
    backend_ids.extend(data.docker.iter().map(|(n, _, _)| n.clone()));

    let process_ids: std::collections::HashSet<String> = data
        .processes
        .iter()
        .map(|(n, _, _, _)| n.clone())
        .collect();

    let mut backend_state: std::collections::BTreeMap<String, ProcState> =
        std::collections::BTreeMap::new();
    for (name, state, _, _) in &data.processes {
        backend_state.insert(name.clone(), state.clone());
    }
    for (name, state, _) in &data.remotes {
        backend_state.insert(name.clone(), state.clone());
    }
    for (name, state, _) in &data.statics {
        backend_state.insert(name.clone(), state.clone());
    }
    for (name, state, _) in &data.docker {
        backend_state.insert(name.clone(), state.clone());
    }

    let mut routes_by_backend: std::collections::BTreeMap<String, Vec<(String, bool)>> =
        std::collections::BTreeMap::new();
    for (host, backend, https_only) in &data.routes {
        routes_by_backend
            .entry(backend.clone())
            .or_default()
            .push((host.clone(), *https_only));
    }

    for (host, backend, https_only) in &data.routes {
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
        let mut detail = if *https_only {
            format!("-> {} (https-only)", backend)
        } else {
            format!("-> {}", backend)
        };
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
            detail,
            muted: false,
            alert: missing || !backend_ok,
        });
    }
    for (name, state, bin, port) in &data.processes {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let (display_name, suffix) = if let Some((host, https_only)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            (
                host,
                format!("backend: {}{} · {} · port: {}", name, marker, bin, port),
            )
        } else {
            (name.clone(), format!("{} · port: {}", bin, port))
        };
        rows.push(RowData {
            kind: "process",
            name: display_name,
            process_id: Some(name.clone()),
            state: state.clone(),
            detail: suffix,
            muted: false,
            alert: false,
        });
    }
    for (name, state, detail) in &data.remotes {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let (display_name, suffix, muted, alert) = if let Some((host, https_only)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            (
                host,
                format!("backend: {}{} · {}", name, marker, detail),
                false,
                false,
            )
        } else {
            (
                name.clone(),
                format!("{} (no frontend)", detail),
                true,
                true,
            )
        };
        rows.push(RowData {
            kind: "remote",
            name: display_name,
            process_id: None,
            state: state.clone(),
            detail: suffix,
            muted,
            alert,
        });
    }
    for (name, state, dir) in &data.statics {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let (display_name, suffix, muted, alert) = if let Some((host, https_only)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            (
                host,
                format!("backend: {}{} · {}", name, marker, dir),
                false,
                false,
            )
        } else {
            (name.clone(), format!("{} (no frontend)", dir), true, true)
        };
        rows.push(RowData {
            kind: "static",
            name: display_name,
            process_id: None,
            state: state.clone(),
            detail: suffix,
            muted,
            alert,
        });
    }
    for (name, state, image) in &data.docker {
        let combined = routes_by_backend.get(name).and_then(|r| r.first()).cloned();
        let (display_name, suffix) = if let Some((host, https_only)) = combined {
            let marker = if https_only { " (https-only)" } else { "" };
            (host, format!("backend: {}{} · {}", name, marker, image))
        } else {
            (name.clone(), image.clone())
        };
        rows.push(RowData {
            kind: "docker",
            name: display_name,
            process_id: None,
            state: state.clone(),
            detail: suffix,
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
) -> (
    Table<'a>,
    TableState,
    usize,
    usize,
    usize,
    ratatui::layout::Rect,
) {
    let rows = build_rows(data);
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
            let (label, color) = fmt_state(row.state.clone());
            let mut name_cell = Cell::from(row.name.clone());
            let mut detail_cell = Cell::from(row.detail.clone());
            let mut kind_cell = Cell::from(row.kind);
            if row.alert {
                let alert_style = Style::default().fg(Color::Red);
                name_cell = name_cell.style(alert_style);
                detail_cell = detail_cell.style(alert_style);
                kind_cell = kind_cell.style(alert_style);
            }
            let mut table_row = Row::new(vec![
                kind_cell,
                name_cell,
                Cell::from(label).style(Style::default().fg(color)),
                detail_cell,
            ]);
            if row.muted {
                table_row = table_row.style(Style::default().fg(Color::Gray));
            }
            table_row
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Type"),
        Cell::from("Name"),
        Cell::from("State"),
        Cell::from("Detail"),
    ])
    .style(Style::default().fg(Color::Gray));

    let table = Table::new(
        rows_vec,
        [
            Constraint::Length(8),
            Constraint::Percentage(35),
            Constraint::Length(10),
            Constraint::Percentage(47),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray))
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
static TUI_LOG_SCROLL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

fn build_log_view<'a>(
    entries: &'a VecDeque<LogLine>,
    scroll: usize,
    area: ratatui::layout::Rect,
    show_timestamp: bool,
    log_level_filter: LogLevelFilter,
) -> (Paragraph<'a>, usize, usize, usize, ratatui::layout::Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let content_width = area.width.saturating_sub(3) as usize;
    let all_lines = build_log_lines(entries, content_width, show_timestamp, log_level_filter);
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
) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for entry in entries.iter() {
        if level_rank(entry.level) < filter_rank(log_level_filter) {
            continue;
        }
        let (label, color) = fmt_level(entry.level);
        let meta_prefix = if show_timestamp {
            format!("[{}] {} {} ", label, entry.timestamp, entry.source)
        } else {
            format!("[{}] {} ", label, entry.source)
        };
        let meta_len = meta_prefix.chars().count();
        let cont_prefix = " ".repeat(meta_len);

        let mut msg_iter = entry.message.lines().peekable();
        if msg_iter.peek().is_none() {
            lines.push(Line::from(vec![
                Span::styled(format!("[{}]", label), Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(&entry.source, Style::default().fg(Color::LightBlue)),
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
                            Style::default().fg(Color::Cyan),
                        ));
                        spans.push(Span::raw(" "));
                    }
                    spans.push(Span::styled(
                        &entry.source,
                        Style::default().fg(Color::LightBlue),
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
) -> usize {
    let mut count = 0usize;
    for entry in entries {
        if level_rank(entry.level) < filter_rank(log_level_filter) {
            continue;
        }
        let (label, _) = fmt_level(entry.level);
        let meta_prefix = if show_timestamp {
            format!("[{}] {} {} ", label, entry.timestamp, entry.source)
        } else {
            format!("[{}] {} ", label, entry.source)
        };
        let meta_len = meta_prefix.chars().count();
        let cont_len = meta_len;
        let mut msg_iter = entry.message.lines().peekable();
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

pub async fn run(global_state: Arc<GlobalState>) {
    let mut terminal = match setup_terminal() {
        Ok(t) => t,
        Err(_) => return,
    };

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
                let was_at_bottom = {
                    let cur = TUI_LOG_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
                    let visible = terminal
                        .size()
                        .map(|s| {
                            s.height
                                .saturating_sub(TUI_HEADER_HEIGHT + TUI_FOOTER_HEIGHT)
                                .saturating_sub(2) as usize
                        })
                        .unwrap_or(0);
                    let width = terminal
                        .size()
                        .map(|s| s.width.saturating_sub(3) as usize)
                        .unwrap_or(0);
                    let total_lines =
                        count_log_lines(&log_entries, width, log_show_timestamp, log_level_filter);
                    let max_start = total_lines.saturating_sub(visible);
                    cur >= max_start.saturating_sub(1)
                };

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

                if page == TuiPage::Logs && was_at_bottom && log_tail {
                    let visible = terminal
                        .size()
                        .map(|s| s.height.saturating_sub(3 + 3 + 2).saturating_sub(2) as usize)
                        .unwrap_or(0);
                    let width = terminal
                        .size()
                        .map(|s| s.width.saturating_sub(3) as usize)
                        .unwrap_or(0);
                    let total_lines =
                        count_log_lines(&log_entries, width, log_show_timestamp, log_level_filter);
                    let max_start = total_lines.saturating_sub(visible);
                    TUI_LOG_SCROLL.store(max_start, std::sync::atomic::Ordering::Relaxed);
                }
                dirty = true;
            }
            last_refresh = Instant::now();
            dirty = true;
        }
        if dirty {
            let _ = terminal.draw(|f| {
                draw_ui(
                    f,
                    &data,
                    page,
                    &log_entries,
                    TUI_LOG_SCROLL.load(std::sync::atomic::Ordering::Relaxed),
                    log_tail,
                    log_show_timestamp,
                    log_level_filter,
                    confirm_quit,
                    hovered_row,
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
                                        let rows = build_rows(&data);
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
                                    TuiPage::Logs => {
                                        if log_tail {
                                            log_tail = false;
                                            dirty = true;
                                        }
                                        let visible =
                                            content_area.height.saturating_sub(2) as usize;
                                        let width = content_area.width.saturating_sub(3) as usize;
                                        let total = count_log_lines(
                                            &log_entries,
                                            width,
                                            log_show_timestamp,
                                            log_level_filter,
                                        );
                                        let max_start = total.saturating_sub(visible);
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
                                    }
                                }
                            }
                            MouseEventKind::Moved => {
                                if page != TuiPage::Sites {
                                    continue;
                                }
                                if drag_scroll.is_some() {
                                    continue;
                                }
                                let rows = build_rows(&data);
                                let total = rows.len();
                                let new_hover = hit_test_site_row(content_area, mouse.row, total)
                                    .map(|row| row);
                                if new_hover != hovered_row {
                                    hovered_row = new_hover;
                                    dirty = true;
                                }
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                if matches!(page, TuiPage::Sites | TuiPage::Logs) {
                                    let scroll_area = scroll_area_for_content(content_area);
                                    if mouse.column == scroll_area.x
                                        && mouse.row >= scroll_area.y
                                        && mouse.row
                                            < scroll_area.y.saturating_add(scroll_area.height)
                                    {
                                        drag_scroll = Some(page);
                                        match page {
                                            TuiPage::Sites => {
                                                let rows = build_rows(&data);
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
                                            TuiPage::Logs => {
                                                if log_tail {
                                                    log_tail = false;
                                                }
                                                let visible =
                                                    content_area.height.saturating_sub(2) as usize;
                                                let width =
                                                    content_area.width.saturating_sub(3) as usize;
                                                let total = count_log_lines(
                                                    &log_entries,
                                                    width,
                                                    log_show_timestamp,
                                                    log_level_filter,
                                                );
                                                let max_start = total.saturating_sub(visible);
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
                                            }
                                        }
                                        continue;
                                    }
                                }
                                if page != TuiPage::Sites {
                                    continue;
                                }
                                let rows = build_rows(&data);
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
                                                    .find(|(n, _, _, _)| n == proc_id)
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
                                        let rows = build_rows(&data);
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
                                    TuiPage::Logs => {
                                        if log_tail {
                                            log_tail = false;
                                        }
                                        let visible =
                                            content_area.height.saturating_sub(2) as usize;
                                        let width = content_area.width.saturating_sub(3) as usize;
                                        let total = count_log_lines(
                                            &log_entries,
                                            width,
                                            log_show_timestamp,
                                            log_level_filter,
                                        );
                                        let max_start = total.saturating_sub(visible);
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
                                TuiPage::Sites => TuiPage::Logs,
                                TuiPage::Logs => TuiPage::Sites,
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
                                );
                                let max_start = total_lines.saturating_sub(visible);
                                TUI_LOG_SCROLL
                                    .store(max_start, std::sync::atomic::Ordering::Relaxed);
                            }
                            hovered_row = None;
                            dirty = true;
                        } else if page == TuiPage::Sites && key.code == KeyCode::Char('s') {
                            let mut changed = false;
                            for (name, state, _, _) in data.processes.iter_mut() {
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
                        } else if page == TuiPage::Sites && key.code == KeyCode::Char('x') {
                            let mut changed = false;
                            for (name, state, _, _) in data.processes.iter_mut() {
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
                                TuiPage::Logs => &TUI_LOG_SCROLL,
                            };
                            if key.code == KeyCode::Up {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                target_scroll.store(
                                    cur.saturating_sub(1),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                if page == TuiPage::Logs {
                                    log_tail = false;
                                }
                                dirty = true;
                            } else if key.code == KeyCode::Down {
                                let cur = target_scroll.load(std::sync::atomic::Ordering::Relaxed);
                                target_scroll.store(
                                    cur.saturating_add(1),
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                                if page == TuiPage::Logs {
                                    log_tail = false;
                                }
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
                                if page == TuiPage::Logs {
                                    log_tail = false;
                                }
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
                                if page == TuiPage::Logs {
                                    log_tail = false;
                                }
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
        "odd_box::proc_host=trace"
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
