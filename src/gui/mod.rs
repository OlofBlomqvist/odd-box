pub mod components;
pub mod logs;
mod pages;

use iced::gradient::{ColorStop, Linear};
use iced::widget::{
    Column, Id, Scrollable, button, column, container, image, row, scrollable, text,
};
use iced::{
    Application, Background, Border, Color, Element, Font, Length, Padding, Radians, Subscription, Task, Theme, system, theme, time
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use crate::global_state::GlobalState;
use crate::configuration::v4;
use logs::{LogFilter, SharedLogState};
use pages::{CachedConfig, CachedLogLine, fetch_config};

static SIDEBAR_LOGO_LIGHT: LazyLock<iced::widget::image::Handle> = LazyLock::new(|| {
    iced::widget::image::Handle::from_bytes(
        &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ob3.png"))[..],
    )
});

static SIDEBAR_LOGO_DARK: LazyLock<iced::widget::image::Handle> = LazyLock::new(|| {
    iced::widget::image::Handle::from_bytes(
        &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ob3_black.png"))[..],
    )
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl ThemeMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::System,
        }
    }
}

pub fn run(
    state: Arc<GlobalState>,
    theme_mode: ThemeMode,
    log_state: SharedLogState,
) -> iced::Result {
    let window_settings = iced::window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(900.0, 400.0)),
        decorations: true, // Use native window decorations (KDE/GNOME title bar)
        blur: true,
        transparent: true,
        ..Default::default()
    };

    let state_clone = state.clone();
    let log_state_clone = log_state.clone();

    iced::application(
        move || OddBoxGui::new(state_clone.clone(), theme_mode, log_state_clone.clone()),
        OddBoxGui::update,
        OddBoxGui::view,
    )
    .style(|_state, theme: &Theme| {
        // Use theme's background with transparency for blur effect
        let bg = theme.palette().background;
        theme::Style {
            background_color: Color::from_rgba(bg.r, bg.g, bg.b, 0.85),
            text_color: theme.palette().text,
        }
    })
    .theme(OddBoxGui::theme)
    .subscription(OddBoxGui::subscription)
    .title("ODD-BOX").window(window_settings)
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Dashboard,
    CrumaIngress,
    Monitoring,
    Statistics,
    Backends,
    Frontends,
    ManagedProcesses,
    EditFrontend,
    EditBackend,
}

impl Page {
    fn title(&self) -> &'static str {
        match self {
            Page::Dashboard => "Dashboard",
            Page::CrumaIngress => "Cruma Ingress",
            Page::Monitoring => "Monitoring",
            Page::Statistics => "Statistics",
            Page::Backends => "Backends",
            Page::Frontends => "Frontends",
            Page::ManagedProcesses => "Managed Processes",
            Page::EditFrontend => "Edit Frontend",
            Page::EditBackend => "Edit Backend",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Page::Dashboard => "⌂",
            Page::CrumaIngress => "⇄",

            Page::Monitoring => "◉",
            Page::Statistics => "▤",
            Page::Backends => "⬚",
            Page::Frontends => "◧",
            Page::ManagedProcesses => "⚙",
            Page::EditFrontend => "✎",
            Page::EditBackend => "✎",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevelPreset {
    All,
    DebugAndAbove,
    #[default]
    InfoAndAbove,
    WarnAndAbove,
    ErrorOnly,
}

impl LogLevelPreset {
    pub const ALL: [LogLevelPreset; 5] = [
        LogLevelPreset::All,
        LogLevelPreset::DebugAndAbove,
        LogLevelPreset::InfoAndAbove,
        LogLevelPreset::WarnAndAbove,
        LogLevelPreset::ErrorOnly,
    ];
}

impl std::fmt::Display for LogLevelPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevelPreset::All => write!(f, "All Levels"),
            LogLevelPreset::DebugAndAbove => write!(f, "Debug+"),
            LogLevelPreset::InfoAndAbove => write!(f, "Info+"),
            LogLevelPreset::WarnAndAbove => write!(f, "Warn+"),
            LogLevelPreset::ErrorOnly => write!(f, "Errors"),
        }
    }
}

impl std::fmt::Display for v4::Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            v4::Protocol::H1 => write!(f, "h1"),
            v4::Protocol::H2 => write!(f, "h2"),
            v4::Protocol::H2C => write!(f, "h2c"),
            v4::Protocol::H2CPK => write!(f, "h2cpk"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// No-op message for hover-only interactive elements
    NoOp,
    NavigateTo(Page),
    SystemThemeChanged(theme::Mode),
    LogViewportChanged(scrollable::Viewport),
    // Log filter messages
    LogFilterTextChanged(String),
    LogLevelPresetChanged(LogLevelPreset),
    LogFilterToggleSource(String, bool),
    LogFilterClearSources,
    LogsClear,
    LogToggleWrap(bool),
    LogToggleAutoTail(bool),
    // Tick for refreshing log view
    Tick,
    // Config data updated
    ConfigUpdated(CachedConfig),
    // Process control
    ProcessStart(String),
    ProcessStop(String),
    // Dashboard card menu
    DashboardToggleProcessMenu(String),
    DashboardCursorMoved(f32, f32),
    OpenEditFrontend(String),
    OpenEditBackend(String),
    EditFrontendLoaded(EditFrontendForm),
    EditFrontendHostChanged(String),
    EditFrontendBackendChanged(String),
    EditFrontendCaptureSubdomainsToggled(bool),
    EditFrontendForwardSubdomainsToggled(bool),
    EditFrontendRedirectHttpsToggled(bool),
    EditFrontendLetsEncryptToggled(bool),
    EditFrontendSave,
    EditFrontendSaveResult(Result<(), String>),
    EditBackendLoaded(EditBackendForm),
    EditBackendFieldChanged(EditBackendField),
    EditBackendPickDir,
    EditBackendDirPicked(Option<String>),
    EditBackendSave,
    EditBackendSaveResult(Result<(), String>),
}

#[derive(Debug, Clone, Default)]
pub struct EditFrontendForm {
    pub hostname: String,
    pub backend: String,
    pub capture_subdomains: bool,
    pub forward_subdomains: bool,
    pub redirect_to_https: bool,
    pub lets_encrypt: bool,
    pub https_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Process,
    Remote,
    Static,
    Unknown,
}

impl Default for BackendKind {
    fn default() -> Self {
        BackendKind::Unknown
    }
}

#[derive(Debug, Clone, Default)]
pub struct EditBackendForm {
    pub id: String,
    pub kind: BackendKind,
    // Remote
    pub endpoints: String,
    pub protocol: v4::Protocol,
    pub https: bool,
    pub keep_original_host_header: bool,
    // Static
    pub dir: String,
    pub list_dir: bool,
    pub render_markdown: bool,
    pub cache_max_age: String,
    // Process (read-only for now)
    pub bin: String,
}

#[derive(Debug, Clone)]
pub enum EditBackendField {
    Endpoints(String),
    Protocol(v4::Protocol),
    Https(bool),
    KeepOriginalHostHeader(bool),
    Dir(String),
    ListDir(bool),
    RenderMarkdown(bool),
    CacheMaxAge(String),
}

pub struct OddBoxGui {
    pub(in crate::gui) state: Arc<GlobalState>,
    pub(in crate::gui) log_state: SharedLogState,
    current_page: Page,
    theme_mode: ThemeMode,
    system_theme: Option<theme::Mode>,
    log_is_at_bottom: bool,
    log_view_rev: u64,
    // Log filtering
    pub(in crate::gui) log_filter: LogFilter,
    pub(in crate::gui) log_level_preset: LogLevelPreset,
    // Cached list of known sources
    pub(in crate::gui) known_sources: Vec<String>,
    // Cached filtered log lines for performance
    pub(in crate::gui) cached_log_lines: Arc<Vec<CachedLogLine>>,
    pub(in crate::gui) last_log_count: usize,
    pub(in crate::gui) total_log_count: usize,
    // Track last seen log ID to avoid unnecessary rebuilds
    pub(in crate::gui) last_seen_log_id: Option<u64>,
    // Log display options
    pub(in crate::gui) log_wrap_enabled: bool,
    pub(in crate::gui) log_auto_tail: bool,
    // Cached config data
    pub(in crate::gui) cached_config: CachedConfig,
    pub(in crate::gui) backend_names: Vec<String>,
    // Dashboard: which process card has its action menu open
    pub(in crate::gui) dashboard_process_menu: Option<String>,
    pub(in crate::gui) dashboard_cursor_pos: (f32, f32),
    pub(in crate::gui) dashboard_menu_pos: (f32, f32),
    pub(in crate::gui) edit_target: Option<String>,
    pub(in crate::gui) edit_frontend_form: EditFrontendForm,
    pub(in crate::gui) edit_frontend_notice: Option<String>,
    pub(in crate::gui) edit_backend_form: EditBackendForm,
    pub(in crate::gui) edit_backend_notice: Option<String>,
}

fn log_scroll_id() -> Id {
    Id::new("odd_box_log_scroll")
}

async fn load_frontend_form(state: Arc<GlobalState>, hostname: String) -> EditFrontendForm {
    let guard = state.config.read().await;

    let mut form = EditFrontendForm {
        hostname: hostname.clone(),
        ..EditFrontendForm::default()
    };

    let mut target: Option<&v4::RouteTarget> = None;
    let mut https_only = false;

    if let Some(http) = &guard.frontends.http {
        if let Some(t) = http.routes.get(&hostname) {
            target = Some(t);
        }
    }

    if target.is_none() {
        if let Some(https) = &guard.frontends.https {
            if let Some(v4::HttpsRoutes::Explicit(routes)) = &https.routes {
                if let Some(t) = routes.get(&hostname) {
                    target = Some(t);
                    https_only = true;
                }
            }
        }
    }

    if let Some(t) = target {
        match t {
            v4::RouteTarget::Simple(backend) => {
                form.backend = backend.clone();
            }
            v4::RouteTarget::Detailed(d) => {
                form.backend = d.backend.clone();
                form.capture_subdomains = d.capture_subdomains;
                form.forward_subdomains = d.forward_subdomains;
                form.redirect_to_https = d.redirect_to_https;
                form.lets_encrypt = d.lets_encrypt;
            }
        }
    }

    form.https_only = https_only;

    form
}

async fn save_frontend_form(
    state: Arc<GlobalState>,
    original_host: Option<String>,
    mut form: EditFrontendForm,
) -> Result<(), String> {
    if form.hostname.trim().is_empty() {
        return Err("Hostname is required.".to_string());
    }
    if form.backend.trim().is_empty() {
        return Err("Backend is required.".to_string());
    }
    if form.capture_subdomains && form.lets_encrypt {
        return Err(
            "LetsEncrypt cannot be enabled when capture subdomains is enabled.".to_string(),
        );
    }

    let mut guard = state.config.write().await;
    if !guard.backends.contains_key(&form.backend) {
        return Err(format!("Backend '{}' does not exist.", form.backend));
    }

    let target = if form.capture_subdomains
        || form.forward_subdomains
        || form.redirect_to_https
        || form.lets_encrypt
    {
        v4::RouteTarget::Detailed(v4::DetailedRoute {
            backend: form.backend.clone(),
            capture_subdomains: form.capture_subdomains,
            forward_subdomains: form.forward_subdomains,
            redirect_to_https: form.redirect_to_https,
            lets_encrypt: form.lets_encrypt,
        })
    } else {
        v4::RouteTarget::Simple(form.backend.clone())
    };

    if form.https_only {
        if let Some(https) = guard.frontends.https.as_mut() {
            if let Some(v4::HttpsRoutes::Explicit(routes)) = https.routes.as_mut() {
                if let Some(old) = original_host.clone() {
                    if old != form.hostname {
                        routes.remove(&old);
                    }
                }
                routes.insert(form.hostname.clone(), target.clone());
            } else {
                form.https_only = false;
            }
        } else {
            form.https_only = false;
        }
    }

    if !form.https_only {
        if guard.frontends.http.is_none() {
            guard.frontends.http = Some(v4::HttpFrontend {
                port: 80,
                routes: HashMap::new(),
            });
        }

        let http = guard.frontends.http.as_mut().unwrap();
        if let Some(old) = original_host {
            if old != form.hostname {
                http.routes.remove(&old);
            }
        }
        http.routes.insert(form.hostname.clone(), target);
    }

    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;

    Ok(())
}

async fn load_backend_form(state: Arc<GlobalState>, backend_id: String) -> EditBackendForm {
    let guard = state.config.read().await;
    let mut form = EditBackendForm {
        id: backend_id.clone(),
        ..EditBackendForm::default()
    };

    if let Some(backend) = guard.backends.get(&backend_id) {
        match backend {
            v4::Backend::Process(p) => {
                form.kind = BackendKind::Process;
                form.bin = p.bin.clone();
            }
            v4::Backend::Remote(r) => {
                form.kind = BackendKind::Remote;
                form.protocol = r.protocol.clone();
                form.https = r.https;
                form.keep_original_host_header = r.keep_original_host_header;
                form.endpoints = r
                    .endpoints
                    .iter()
                    .map(|e| format!("{}:{}", e.addr, e.port))
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            v4::Backend::Static(s) => {
                form.kind = BackendKind::Static;
                form.dir = s.dir.clone();
                form.list_dir = s.list_dir;
                form.render_markdown = s.render_markdown;
                form.cache_max_age = s
                    .cache_max_age
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
        }
    } else {
        form.kind = BackendKind::Unknown;
    }

    form
}

async fn save_backend_form(
    state: Arc<GlobalState>,
    form: EditBackendForm,
) -> Result<(), String> {
    if form.id.trim().is_empty() {
        return Err("Backend id is required.".to_string());
    }

    let mut guard = state.config.write().await;

    match form.kind {
        BackendKind::Remote => {
            let endpoints = form
                .endpoints
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|item| {
                    let (addr, port_str) = item
                        .rsplit_once(':')
                        .ok_or_else(|| format!("Invalid endpoint '{}'", item))?;
                    let port: u16 = port_str
                        .parse()
                        .map_err(|_| format!("Invalid port in '{}'", item))?;
                    Ok(v4::Endpoint {
                        addr: addr.to_string(),
                        port,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;

            if endpoints.is_empty() {
                return Err("At least one endpoint is required.".to_string());
            }

            guard.backends.insert(
                form.id.clone(),
                v4::Backend::Remote(v4::RemoteBackend {
                    endpoints,
                    protocol: form.protocol,
                    https: form.https,
                    keep_original_host_header: form.keep_original_host_header,
                }),
            );
        }
        BackendKind::Static => {
            if form.dir.trim().is_empty() {
                return Err("Directory is required.".to_string());
            }

            let cache_max_age = if form.cache_max_age.trim().is_empty() {
                None
            } else {
                Some(
                    form.cache_max_age
                        .trim()
                        .parse::<u64>()
                        .map_err(|_| "Cache max-age must be a number.".to_string())?,
                )
            };

            guard.backends.insert(
                form.id.clone(),
                v4::Backend::Static(v4::StaticBackend {
                    dir: form.dir.clone(),
                    index: "index.html".to_string(),
                    list_dir: form.list_dir,
                    render_markdown: form.render_markdown,
                    cache_max_age,
                }),
            );
        }
        BackendKind::Process => {
            return Err("Process backends are edited on the Managed Processes page.".to_string());
        }
        BackendKind::Unknown => {
            return Err("Backend not found.".to_string());
        }
    }

    guard.reload_dashmaps();
    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    Ok(())
}

async fn pick_backend_dir() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|p| p.display().to_string())
}

impl OddBoxGui {
    fn apply_log_level_preset(filter: &mut LogFilter, preset: LogLevelPreset) {
        match preset {
            LogLevelPreset::All => {
                filter.show_trace = true;
                filter.show_debug = true;
                filter.show_info = true;
                filter.show_warn = true;
                filter.show_error = true;
            }
            LogLevelPreset::DebugAndAbove => {
                filter.show_trace = false;
                filter.show_debug = true;
                filter.show_info = true;
                filter.show_warn = true;
                filter.show_error = true;
            }
            LogLevelPreset::InfoAndAbove => {
                filter.show_trace = false;
                filter.show_debug = false;
                filter.show_info = true;
                filter.show_warn = true;
                filter.show_error = true;
            }
            LogLevelPreset::WarnAndAbove => {
                filter.show_trace = false;
                filter.show_debug = false;
                filter.show_info = false;
                filter.show_warn = true;
                filter.show_error = true;
            }
            LogLevelPreset::ErrorOnly => {
                filter.show_trace = false;
                filter.show_debug = false;
                filter.show_info = false;
                filter.show_warn = false;
                filter.show_error = true;
            }
        }
    }

    fn new(
        state: Arc<GlobalState>,
        theme_mode: ThemeMode,
        log_state: SharedLogState,
    ) -> (Self, Task<Message>) {
        let state_clone = state.clone();
        let mut tasks: Vec<Task<Message>> = vec![Task::perform(
            fetch_config(state_clone),
            Message::ConfigUpdated,
        )];

        // On system mode, grab current OS theme (winit-powered)
        if matches!(theme_mode, ThemeMode::System) {
            tasks.push(system::theme().map(Message::SystemThemeChanged));
        }

        let mut log_filter = LogFilter::new();
        let log_level_preset = LogLevelPreset::InfoAndAbove;
        Self::apply_log_level_preset(&mut log_filter, log_level_preset);

        (
            Self {
                state,
                log_state,
                current_page: Page::Dashboard,
                theme_mode,
                system_theme: None,
                log_is_at_bottom: true,
                log_view_rev: 0,
                log_filter,
                log_level_preset,
                known_sources: Vec::new(),
                cached_log_lines: Arc::new(Vec::new()),
                last_log_count: 0,
                total_log_count: 0,
                last_seen_log_id: None,
                log_wrap_enabled: false,
                log_auto_tail: true, // Auto-tail enabled by default
                cached_config: CachedConfig::default(),
                backend_names: Vec::new(),
                dashboard_process_menu: None,
                dashboard_cursor_pos: (0.0, 0.0),
                dashboard_menu_pos: (0.0, 0.0),
                edit_target: None,
                edit_frontend_form: EditFrontendForm::default(),
                edit_frontend_notice: None,
                edit_backend_form: EditBackendForm::default(),
                edit_backend_notice: None,
            },
            Task::batch(tasks),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        // Tick for pages that need live updates
        let page_sub = match self.current_page {
            Page::Monitoring => {
                time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick)
            }
            Page::ManagedProcesses | Page::Backends | Page::Frontends | Page::Dashboard => {
                // Slower tick for config pages (process status can change)
                time::every(std::time::Duration::from_millis(1000)).map(|_| Message::Tick)
            }
            Page::EditFrontend | Page::EditBackend => {
                time::every(std::time::Duration::from_millis(1000)).map(|_| Message::Tick)
            }
            _ => Subscription::none(),
        };

        let theme_sub = system::theme_changes().map(Message::SystemThemeChanged);

        Subscription::batch(vec![page_sub, theme_sub])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => {}
            Message::NavigateTo(page) => {
                self.current_page = page;
                self.dashboard_process_menu = None;
                if page == Page::Monitoring {
                    self.refresh_log_cache(true);
                }
                // Trigger config refresh for config-related pages
                if matches!(
                    page,
                    Page::ManagedProcesses
                        | Page::Backends
                        | Page::Frontends
                        | Page::Dashboard
                        | Page::EditFrontend
                        | Page::EditBackend
                ) {
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
            }
            Message::SystemThemeChanged(mode) => {
                self.system_theme = Some(mode);
            }
            Message::LogViewportChanged(viewport) => {
                let bounds = viewport.bounds();
                let content_bounds = viewport.content_bounds();

                let max_scroll_y = (content_bounds.height - bounds.height).max(0.0);
                let current_y = viewport.absolute_offset().y;

                // Allow a small tolerance for rounding/layout differences.
                self.log_is_at_bottom = max_scroll_y - current_y <= 4.0;
            }
            Message::Tick => {
                if self.current_page == Page::Monitoring {
                    self.refresh_log_cache(false);
                }
                // Refresh config for config-related pages
                if matches!(
                    self.current_page,
                    Page::ManagedProcesses
                        | Page::Backends
                        | Page::Frontends
                        | Page::Dashboard
                        | Page::EditFrontend
                        | Page::EditBackend
                ) {
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
            }
            Message::ConfigUpdated(config) => {
                self.cached_config = config;
                let mut names: Vec<String> = Vec::new();
                names.extend(self.cached_config.processes.iter().map(|p| p.name.clone()));
                names.extend(
                    self.cached_config
                        .remote_backends
                        .iter()
                        .map(|b| b.name.clone()),
                );
                names.extend(
                    self.cached_config
                        .static_backends
                        .iter()
                        .map(|b| b.name.clone()),
                );
                names.sort();
                names.dedup();
                self.backend_names = names;
            }
            Message::LogFilterTextChanged(text) => {
                self.log_filter.text = text;
                self.refresh_log_cache(true);
            }
            Message::LogLevelPresetChanged(preset) => {
                self.log_level_preset = preset;
                Self::apply_log_level_preset(&mut self.log_filter, preset);
                self.refresh_log_cache(true);
            }
            Message::LogFilterToggleSource(source, enabled) => {
                if enabled {
                    self.log_filter.sources.insert(source);
                } else {
                    self.log_filter.sources.remove(&source);
                }
                self.refresh_log_cache(true);
            }
            Message::LogFilterClearSources => {
                self.log_filter.sources.clear();
                self.refresh_log_cache(true);
            }
            Message::LogsClear => {
                self.log_state.write().clear();
                self.cached_log_lines = Arc::new(Vec::new());
                self.last_log_count = 0;
                self.total_log_count = 0;
                self.last_seen_log_id = None;
                self.log_view_rev = self.log_view_rev.wrapping_add(1);
            }
            Message::LogToggleWrap(enabled) => {
                self.log_wrap_enabled = enabled;
                self.log_view_rev = self.log_view_rev.wrapping_add(1);
            }
            Message::LogToggleAutoTail(enabled) => {
                self.log_auto_tail = enabled;
                if enabled {
                    self.log_is_at_bottom = true;
                    return iced::widget::operation::snap_to_end::<Message>(log_scroll_id());
                }
            }
            Message::ProcessStart(name) => {
                self.state.process_registry.set_enabled(&name, true);
            }
            Message::ProcessStop(name) => {
                self.state.process_registry.set_enabled(&name, false);
            }
            Message::DashboardToggleProcessMenu(name) => {
                if self.dashboard_process_menu.as_ref() == Some(&name) {
                    self.dashboard_process_menu = None;
                } else {
                    self.dashboard_process_menu = Some(name);
                    self.dashboard_menu_pos = self.dashboard_cursor_pos;
                }
            }
            Message::DashboardCursorMoved(x, y) => {
                self.dashboard_cursor_pos = (x, y);
            }
            Message::OpenEditFrontend(name) => {
                self.edit_target = Some(name.clone());
                self.current_page = Page::EditFrontend;
                self.dashboard_process_menu = None;
                self.edit_frontend_notice = None;
                return Task::perform(
                    load_frontend_form(self.state.clone(), name),
                    Message::EditFrontendLoaded,
                );
            }
            Message::OpenEditBackend(name) => {
                self.edit_target = Some(name);
                self.current_page = Page::EditBackend;
                self.dashboard_process_menu = None;
                self.edit_backend_notice = None;
                return Task::perform(
                    load_backend_form(self.state.clone(), self.edit_target.clone().unwrap()),
                    Message::EditBackendLoaded,
                );
            }
            Message::EditFrontendLoaded(form) => {
                self.edit_frontend_form = form;
            }
            Message::EditFrontendHostChanged(value) => {
                self.edit_frontend_form.hostname = value;
            }
            Message::EditFrontendBackendChanged(value) => {
                self.edit_frontend_form.backend = value;
            }
            Message::EditFrontendCaptureSubdomainsToggled(value) => {
                self.edit_frontend_form.capture_subdomains = value;
            }
            Message::EditFrontendForwardSubdomainsToggled(value) => {
                self.edit_frontend_form.forward_subdomains = value;
            }
            Message::EditFrontendRedirectHttpsToggled(value) => {
                self.edit_frontend_form.redirect_to_https = value;
            }
            Message::EditFrontendLetsEncryptToggled(value) => {
                self.edit_frontend_form.lets_encrypt = value;
            }
            Message::EditFrontendSave => {
                self.edit_frontend_notice = None;
                let form = self.edit_frontend_form.clone();
                let original_host = self.edit_target.clone();
                return Task::perform(
                    save_frontend_form(self.state.clone(), original_host, form),
                    Message::EditFrontendSaveResult,
                );
            }
            Message::EditFrontendSaveResult(result) => match result {
                Ok(_) => {
                    self.edit_target = Some(self.edit_frontend_form.hostname.clone());
                    self.edit_frontend_notice = Some("Saved. Waiting for reload...".to_string());
                }
                Err(err) => {
                    self.edit_frontend_notice = Some(err);
                }
            },
            Message::EditBackendLoaded(form) => {
                self.edit_backend_form = form;
            }
            Message::EditBackendFieldChanged(field) => match field {
                EditBackendField::Endpoints(v) => self.edit_backend_form.endpoints = v,
                EditBackendField::Protocol(v) => self.edit_backend_form.protocol = v,
                EditBackendField::Https(v) => self.edit_backend_form.https = v,
                EditBackendField::KeepOriginalHostHeader(v) => {
                    self.edit_backend_form.keep_original_host_header = v;
                }
                EditBackendField::Dir(v) => self.edit_backend_form.dir = v,
                EditBackendField::ListDir(v) => self.edit_backend_form.list_dir = v,
                EditBackendField::RenderMarkdown(v) => self.edit_backend_form.render_markdown = v,
                EditBackendField::CacheMaxAge(v) => self.edit_backend_form.cache_max_age = v,
            },
            Message::EditBackendPickDir => {
                return Task::perform(pick_backend_dir(), Message::EditBackendDirPicked);
            }
            Message::EditBackendDirPicked(path) => {
                if let Some(p) = path {
                    self.edit_backend_form.dir = p;
                }
            }
            Message::EditBackendSave => {
                self.edit_backend_notice = None;
                let form = self.edit_backend_form.clone();
                return Task::perform(
                    save_backend_form(self.state.clone(), form),
                    Message::EditBackendSaveResult,
                );
            }
            Message::EditBackendSaveResult(result) => match result {
                Ok(_) => {
                    self.edit_backend_notice = Some("Saved. Waiting for reload...".to_string());
                }
                Err(err) => {
                    self.edit_backend_notice = Some(err);
                }
            },
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_content();

        // Main layout
        container(
            row![sidebar, content]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(|_theme: &Theme| {
            container::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let is_light = match self.theme_mode {
            ThemeMode::Light => true,
            ThemeMode::Dark => false,
            ThemeMode::System => matches!(self.system_theme, Some(theme::Mode::Light)),
        };
        let logo_handle = if is_light { SIDEBAR_LOGO_DARK.clone() } else { SIDEBAR_LOGO_LIGHT.clone() };
        let logo = image(logo_handle).expand(true);

        let header = container(
            row![
                logo,
                // column![
                //     text("ODD-BOX")
                //         .font(Font::MONOSPACE)
                //         .style(|theme: &Theme| iced::widget::text::Style {
                //             color: Some(theme.extended_palette().primary.base.color),
                //             ..Default::default()
                //         }),
                //     text("Reverse Proxy").style(|theme: &Theme| iced::widget::text::Style {
                //         color: Some(theme.extended_palette().background.weak.text),
                //         ..Default::default()
                //     }),
                // ]
                // .spacing(4)
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding::new(20.0));

        let nav_items = [
            Page::Dashboard,
            Page::CrumaIngress,
            Page::Monitoring,
            Page::Statistics,
            Page::Backends,
            Page::Frontends,
            Page::ManagedProcesses,
        ];

        let nav_buttons: Vec<Element<'_, Message>> = nav_items
            .iter()
            .map(|&page| self.nav_button(page))
            .collect();

        let nav = Column::with_children(nav_buttons)
            .spacing(4)
            .padding(Padding {
                top: 10.0,
                right: 10.0,
                bottom: 10.0,
                left: 10.0,
            });

        let sidebar_content = column![header, nav]
            .width(Length::Fixed(200.0))
            .height(Length::Fill);

        container(sidebar_content)
            .style(|theme: &Theme| {
                // Slightly darker/lighter than main bg for contrast
                let bg = theme.extended_palette().background.weak.color;
                container::Style {
                    background: Some(Background::Color(Color::from_rgba(bg.r, bg.g, bg.b, 0.5))),
                    ..Default::default()
                }
            })
            .width(Length::Fixed(200.0))
            .height(Length::Fill)
            .into()
    }

    fn nav_button(&self, page: Page) -> Element<'_, Message> {
        let is_active = self.current_page == page;

        let label = row![
            text(page.icon()).width(Length::Fixed(24.0)),
            text(page.title()),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let btn = button(label)
            .width(Length::Fill)
            .padding(Padding {
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
                left: 12.0,
            })
            .style(move |theme: &Theme, status| {
                let palette = theme.extended_palette();

                // Check if theme is light by looking at background luminance
                let bg = theme.palette().background;
                let is_light_theme = (0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b) > 0.5;

                let (background, text_color) = if is_active {
                    let bg_color = palette.primary.strong.color;
                    // For light themes, ensure good contrast on the active button
                    let text = if is_light_theme {
                        Color::WHITE
                    } else {
                        palette.primary.strong.text
                    };
                    (bg_color, text)
                } else {
                    match status {
                        button::Status::Hovered => {
                            (palette.background.weak.color, palette.background.weak.text)
                        }
                        _ => (Color::TRANSPARENT, palette.background.weak.text),
                    }
                };

                button::Style {
                    background: Some(background.into()),
                    text_color,
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::NavigateTo(page));

        btn.into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let page_content: Element<'_, Message> = match self.current_page {
            Page::Dashboard => self.view_dashboard(),
            Page::CrumaIngress => self.view_placeholder("Cruma Ingress configuration"),
            Page::Monitoring => self.view_monitoring(),
            Page::Statistics => self.view_placeholder("Traffic statistics and metrics"),
            Page::Backends => self.view_backends(),
            Page::Frontends => self.view_frontends(),
            Page::ManagedProcesses => self.view_processes(),
            Page::EditFrontend => self.view_edit_frontend(),
            Page::EditBackend => self.view_edit_backend(),
        };

        // Monitoring page handles its own layout (no extra scrollable wrapper)
        if self.current_page == Page::Monitoring {
            page_content
        } else {
            let page_title = text(self.current_page.title()).size(20);
            let content = column![page_title, page_content]
                .spacing(20)
                .padding(30)
                .width(Length::Fill);

            Scrollable::new(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &Theme, status| scrollable::Style {
                    container: container::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
                        ..Default::default()
                    },
                    ..scrollable::default(theme, status)
                })
                .into()
        }
    }

    fn view_placeholder(&self, description: &'static str) -> Element<'_, Message> {
        container(
            text(description).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.text),
                ..Default::default()
            }),
        )
        .padding(20)
        .width(Length::Fill)
        .into()
    }

    pub(crate) fn theme(&self) -> Theme {
        match self.theme_mode {
            ThemeMode::Light => Theme::Light,
            ThemeMode::Dark => Theme::Dracula,
            ThemeMode::System => match self.system_theme {
                Some(theme::Mode::Light) => Theme::Light,
                Some(theme::Mode::Dark) => Theme::Dracula,
                _ => Theme::Dracula,
            },
        }
    }
}
