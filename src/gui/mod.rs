pub mod components;
pub mod logs;
mod macos_app_icon;
mod pages;
mod tray;

use iced::widget::{
    Column, Scrollable, button, column, container, image, row, scrollable, text,
};
use iced::widget::scrollable::RelativeOffset;

use iced::{
    Background, Border, Color, Element, Length, Padding, Subscription,
    Task, Theme, system, theme, time, window,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::configuration::{LogLevel, v4};
use crate::global_state::GlobalState;
use crate::types::proc_info::ProcId;
use logs::{LogFilter, SharedLogState};
use pages::{CachedConfig, fetch_config};

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

static GUI_TEXT_SCALE_BITS: AtomicU32 = AtomicU32::new(1.0f32.to_bits());

fn compute_text_scale(size: iced::Size) -> f32 {
    let width_scale = (size.width / 1200.0).clamp(0.85, 1.35);
    let height_scale = (size.height / 800.0).clamp(0.85, 1.35);
    width_scale.min(height_scale)
}

fn set_gui_text_scale(size: iced::Size) {
    GUI_TEXT_SCALE_BITS.store(compute_text_scale(size).to_bits(), Ordering::Relaxed);
}

pub(in crate::gui) fn text_size(base: u16) -> f32 {
    let scale = f32::from_bits(GUI_TEXT_SCALE_BITS.load(Ordering::Relaxed));
    (base as f32 * scale).round()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessesTab {
    #[default]
    Processes,
    GlobalVariables,
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
    macos_app_icon::apply_default_icon();

    // Initialize system tray icon
    let tray_handle = match tray::TrayHandle::new("ODD-BOX") {
        Ok(handle) => {
            tracing::info!("System tray initialized successfully");
            Some(handle)
        }
        Err(e) => {
            tracing::warn!("Failed to initialize system tray: {}", e);
            None
        }
    };

    let use_glass_effects = cfg!(target_os = "macos");
    let initial_window_size = iced::Size::new(1200.0, 800.0);
    set_gui_text_scale(initial_window_size);
    let window_settings = iced::window::Settings {
        size: initial_window_size,
        min_size: Some(iced::Size::new(900.0, 400.0)),
        decorations: true, // Use native window decorations (KDE/GNOME title bar)
        blur: use_glass_effects,
        transparent: use_glass_effects,
        // Disable default close behavior so we can intercept and hide instead
        #[cfg(target_os = "macos")]
        exit_on_close_request: false,
        ..Default::default()
    };

    let state_clone = state.clone();
    let log_state_clone = log_state.clone();

    // Wrap tray_handle in Arc<Mutex> so we can move it into the closure
    let tray_handle = std::sync::Arc::new(std::sync::Mutex::new(tray_handle));
    let tray_handle_clone = tray_handle.clone();

    iced::application(
        move || {
            let tray = tray_handle_clone.lock().unwrap().take();
            OddBoxGui::new(state_clone.clone(), theme_mode, log_state_clone.clone(), tray)
        },
        OddBoxGui::update,
        OddBoxGui::view,
    )
    .style(|_state, theme: &Theme| {
        let bg = theme.palette().background;
        let use_glass_effects = cfg!(target_os = "macos");
        let is_light = (0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b) > 0.5;
        let alpha = if use_glass_effects {
            if is_light { 0.68 } else { 0.80 }
        } else {
            1.0
        };
        theme::Style {
            background_color: Color::from_rgba(bg.r, bg.g, bg.b, alpha),
            text_color: theme.palette().text,
        }
    })
    .theme(OddBoxGui::theme)
    .subscription(OddBoxGui::subscription)
    .title("ODD-BOX")
    .window(window_settings)
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Dashboard,
    CrumaIngress,
    Monitoring,
    TrafficInspection,
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
            Page::TrafficInspection => "Traffic Inspection",
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
            Page::TrafficInspection => "⇆",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessLogLevelChoice {
    #[default]
    Default,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl ProcessLogLevelChoice {
    pub const ALL: [ProcessLogLevelChoice; 6] = [
        ProcessLogLevelChoice::Default,
        ProcessLogLevelChoice::Trace,
        ProcessLogLevelChoice::Debug,
        ProcessLogLevelChoice::Info,
        ProcessLogLevelChoice::Warn,
        ProcessLogLevelChoice::Error,
    ];

    pub fn from_option(level: &Option<LogLevel>) -> Self {
        match level {
            None => ProcessLogLevelChoice::Default,
            Some(LogLevel::Trace) => ProcessLogLevelChoice::Trace,
            Some(LogLevel::Debug) => ProcessLogLevelChoice::Debug,
            Some(LogLevel::Info) => ProcessLogLevelChoice::Info,
            Some(LogLevel::Warn) => ProcessLogLevelChoice::Warn,
            Some(LogLevel::Error) => ProcessLogLevelChoice::Error,
        }
    }

    pub fn to_option(self) -> Option<LogLevel> {
        match self {
            ProcessLogLevelChoice::Default => None,
            ProcessLogLevelChoice::Trace => Some(LogLevel::Trace),
            ProcessLogLevelChoice::Debug => Some(LogLevel::Debug),
            ProcessLogLevelChoice::Info => Some(LogLevel::Info),
            ProcessLogLevelChoice::Warn => Some(LogLevel::Warn),
            ProcessLogLevelChoice::Error => Some(LogLevel::Error),
        }
    }
}

impl std::fmt::Display for ProcessLogLevelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessLogLevelChoice::Default => write!(f, "Default (Info)"),
            ProcessLogLevelChoice::Trace => write!(f, "Trace"),
            ProcessLogLevelChoice::Debug => write!(f, "Debug"),
            ProcessLogLevelChoice::Info => write!(f, "Info"),
            ProcessLogLevelChoice::Warn => write!(f, "Warn"),
            ProcessLogLevelChoice::Error => write!(f, "Error"),
        }
    }
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
    WindowResized(window::Id, iced::Size),
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
    ExitPoll,
    ExitWindowId(Option<window::Id>),
    // Window close request (for hiding instead of closing on macOS)
    WindowCloseRequested(window::Id),
    // Window focus events (to track minimized state)
    WindowFocused(window::Id),
    WindowUnfocused(window::Id),
    // Window minimized state check result
    WindowMinimizedCheck(Option<bool>),
    // Window ID resolved after opening
    WindowIdResolved(Option<window::Id>),
    // Tray command received
    TrayCommandReceived,
    // Tray quit phase 2 - complete the quit after window is hidden
    TrayQuitPhase2,
    // Config data updated
    ConfigUpdated(CachedConfig),
    // Frontend port settings
    FrontendHttpPortChanged(String),
    FrontendHttpsPortChanged(String),
    FrontendPortsSave,
    FrontendPortsSaveResult(Result<(), String>),
    // Process control
    ProcessStart(String),
    ProcessStop(String),
    ProcessStartAll,
    ProcessStopAll,
    ProcessToggleDetails(String),
    // Dashboard card menu
    DashboardToggleProcessMenu(String),
    DashboardCursorMoved(f32, f32),
    OpenEditFrontend(String),
    OpenEditBackend(String),
    OpenNewFrontend,
    OpenNewBackend(BackendKind),
    EditFrontendLoaded(EditFrontendForm),
    EditFrontendHostChanged(String),
    EditFrontendBackendChanged(BackendOption),
    EditFrontendCaptureSubdomainsToggled(bool),
    EditFrontendForwardSubdomainsToggled(bool),
    EditFrontendRedirectHttpsToggled(bool),
    EditFrontendLetsEncryptToggled(bool),
    EditFrontendSave,
    EditFrontendSaveResult(Result<(), String>),
    EditFrontendDelete,
    EditFrontendDeleteResult(Result<(), String>),
    EditBackendLoaded(EditBackendForm),
    EditBackendFieldChanged(EditBackendField),
    EditBackendPickDir,
    EditBackendDirPicked(Option<String>),
    EditBackendPickBin,
    EditBackendBinPicked(Option<String>),
    EditBackendResolveDir(String),
    EditBackendResolvedDir(Result<Option<String>, String>),
    EditBackendSave,
    EditBackendSaveResult(Result<(), String>),
    EditBackendDelete,
    EditBackendDeleteResult(Result<(), String>),
    // Process backend env vars (keypair editor)
    EditBackendEnvKeyChanged(usize, String),
    EditBackendEnvValueChanged(usize, String),
    EditBackendEnvRemove(usize),
    EditBackendEnvNewKeyChanged(String),
    EditBackendEnvNewValueChanged(String),
    EditBackendEnvAdd,
    CrumaAuthModeChanged(CrumaAuthMode),
    CrumaAuthModeSaveResult(Result<(), String>),
    TrafficInspectionToggled(bool),
    TrafficInspectionClear,
    TrafficInspectionSelect(Option<u64>),
    // Processes page tab
    ProcessesTabChanged(ProcessesTab),
    // Global environment variables
    GlobalEnvKeyChanged(usize, String),
    GlobalEnvValueChanged(usize, String),
    GlobalEnvRemove(usize),
    GlobalEnvNewKeyChanged(String),
    GlobalEnvNewValueChanged(String),
    GlobalEnvAdd,
    GlobalEnvSave,
    GlobalEnvSaveResult(Result<(), String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrumaAuthMode {
    Disabled,
    #[default]
    Anonymous,
    Authenticated,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackendOption {
    pub id: String,
    pub label: String,
}

impl BackendOption {
    pub fn new(id: String, kind: &str) -> Self {
        let label = format!("{id} ({kind})");
        Self { id, label }
    }
}

impl std::fmt::Display for BackendOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.label.fmt(f)
    }
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
    // Process
    pub proc_bin: String,
    pub proc_args: String,
    pub proc_dir: String,
    pub proc_port: String,
    pub proc_env: Vec<(String, String)>,
    pub proc_auto_start: bool,
    pub proc_exclude_from_start_all: bool,
    pub proc_log_level: ProcessLogLevelChoice,
    // Process (read-only for now)
    pub bin: String,
}

#[derive(Debug, Clone)]
pub enum EditBackendField {
    Id(String),
    Endpoints(String),
    Protocol(v4::Protocol),
    Https(bool),
    KeepOriginalHostHeader(bool),
    Dir(String),
    ListDir(bool),
    RenderMarkdown(bool),
    CacheMaxAge(String),
    ProcBin(String),
    ProcArgs(String),
    ProcDir(String),
    ProcPort(String),
    ProcAutoStart(bool),
    ProcExcludeFromStartAll(bool),
    ProcLogLevel(ProcessLogLevelChoice),
}

pub struct OddBoxGui {
    pub(in crate::gui) state: Arc<GlobalState>,
    pub(in crate::gui) log_state: SharedLogState,
    current_page: Page,
    theme_mode: ThemeMode,
    system_theme: Option<theme::Mode>,
    log_is_at_bottom: bool,
    // Window and tray management
    window_id: Option<window::Id>,
    window_visible: bool,
    tray_handle: Option<tray::TrayHandle>,
    // Log filtering (local copy for UI display; synced to log_state via set_filter)
    pub(in crate::gui) log_filter: LogFilter,
    pub(in crate::gui) log_level_preset: LogLevelPreset,
    expanded_process: Option<String>,
    // Track last filtered entry ID for auto-tail change detection
    last_seen_filtered_id: Option<u64>,
    // Track previous viewport values so content growth does not disable tail mode.
    last_log_max_scroll_y: Option<f32>,
    last_log_viewport_y: Option<f32>,
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
    pub(in crate::gui) edit_frontend_pending_reload: bool,
    pub(in crate::gui) edit_frontend_original: Option<String>,
    pub(in crate::gui) edit_frontend_is_new: bool,
    pub(in crate::gui) edit_backend_form: EditBackendForm,
    pub(in crate::gui) edit_backend_notice: Option<String>,
    pub(in crate::gui) edit_backend_resolved_dir: Option<String>,
    pub(in crate::gui) edit_backend_resolve_error: Option<String>,
    pub(in crate::gui) edit_backend_pending_reload: bool,
    pub(in crate::gui) edit_backend_original: Option<String>,
    pub(in crate::gui) edit_backend_is_new: bool,
    pub(in crate::gui) edit_backend_confirm_delete: bool,
    pub(in crate::gui) edit_backend_env_new_key: String,
    pub(in crate::gui) edit_backend_env_new_value: String,
    pub(in crate::gui) cruma_auth_mode: CrumaAuthMode,
    pub(in crate::gui) cruma_mode_notice: Option<String>,
    pub(in crate::gui) processes_tab: ProcessesTab,
    pub(in crate::gui) global_env_vars: Vec<(String, String)>,
    pub(in crate::gui) global_env_new_key: String,
    pub(in crate::gui) global_env_new_value: String,
    pub(in crate::gui) global_env_notice: Option<String>,
    pub(in crate::gui) global_env_dirty: bool,
    exit_requested: bool,
    tray_quit_pending: bool,
    pub(in crate::gui) traffic_inspection_selected: Option<u64>,
    frontend_http_port_input: String,
    frontend_https_port_input: String,
    frontend_port_notice: Option<String>,
    frontend_ports_dirty: bool,
}

fn log_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("odd_box_log_scroll")
}

fn snap_log_to_bottom() -> Task<Message> {
    iced::widget::operation::snap_to::<Message>(
        log_scroll_id(),
        RelativeOffset {
            x: None,
            y: Some(1.0),
        },
    )
}

fn cruma_mode_from_config(cfg: &crate::configuration::ConfigWrapper) -> CrumaAuthMode {
    match cfg.cruma.as_ref().and_then(|c| c.mode()) {
        Some(crate::configuration::v4::CrumaMode::Anonymous) => CrumaAuthMode::Anonymous,
        Some(crate::configuration::v4::CrumaMode::Authenticated { .. }) => {
            CrumaAuthMode::Authenticated
        }
        None => CrumaAuthMode::Disabled,
    }
}

async fn load_frontend_form(state: Arc<GlobalState>, hostname: String) -> EditFrontendForm {
    let guard = state.config.load_full();

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

    let mut guard = (*state.config.load_full()).clone();
    if original_host.is_none() {
        if let Some(http) = &guard.frontends.http {
            if http.routes.contains_key(&form.hostname) {
                return Err("Route already exists.".to_string());
            }
        }
        if let Some(https) = &guard.frontends.https {
            if let Some(v4::HttpsRoutes::Explicit(routes)) = &https.routes {
                if routes.contains_key(&form.hostname) {
                    return Err("Route already exists.".to_string());
                }
            }
        }
    }
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
    state.config.store(std::sync::Arc::new(guard));

    Ok(())
}

async fn save_frontend_ports(
    state: Arc<GlobalState>,
    http_port_input: String,
    https_port_input: String,
) -> Result<(), String> {
    fn parse_port(value: &str) -> Result<Option<u16>, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let port: u16 = trimmed
            .parse()
            .map_err(|_| format!("Invalid port '{trimmed}'. Use 1-65535."))?;
        if port == 0 {
            return Err("Port must be between 1 and 65535.".to_string());
        }
        Ok(Some(port))
    }

    let http_port = parse_port(&http_port_input)?;
    let https_port = parse_port(&https_port_input)?;

    let mut guard = (*state.config.load_full()).clone();

    if let Some(port) = http_port {
        if guard.frontends.http.is_none() {
            guard.frontends.http = Some(v4::HttpFrontend {
                port,
                routes: HashMap::new(),
            });
        } else if let Some(http) = guard.frontends.http.as_mut() {
            http.port = port;
        }
    }

    if let Some(port) = https_port {
        if guard.frontends.https.is_none() {
            let routes = guard
                .frontends
                .http
                .as_ref()
                .map(|_| v4::HttpsRoutes::Inherit(v4::InheritMarker::Inherit));
            guard.frontends.https = Some(v4::HttpsFrontend {
                port,
                cert: v4::CertMode::default(),
                routes,
            });
        } else if let Some(https) = guard.frontends.https.as_mut() {
            https.port = port;
        }
    }

    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    crate::cruma_integration::rebuild_cruma_config(state.clone());

    Ok(())
}

async fn save_global_env(
    state: Arc<GlobalState>,
    vars: Vec<(String, String)>,
) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();

    let env: std::collections::HashMap<String, String> = vars
        .into_iter()
        .filter(|(k, _)| !k.is_empty())
        .collect();
    guard.env = env;

    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    
    // Collect all process backend IDs before storing (global env affects all)
    let process_ids: Vec<String> = guard
        .hosted_processes
        .iter()
        .map(|e| e.key().clone())
        .collect();
    
    state.config.store(std::sync::Arc::new(guard));
    
    // Restart all process backends to pick up new global env vars
    for backend_id in process_ids {
        restart_process_backend_sync(&state, &backend_id);
    }

    Ok(())
}

/// Restart a process backend with freshly resolved configuration.
/// This is used after env var changes to ensure the process picks up new values.
/// Note: This function bridges from iced's smol runtime to tokio.
fn restart_process_backend_sync(state: &Arc<GlobalState>, backend_id: &str) {
    use tokio_util::sync::CancellationToken;
    
    let handle = state.tokio_handle.clone();
    let config = state.config.load_full();
    
    // Get the process backend config
    let Some(proc) = config.hosted_processes.get(backend_id).map(|e| e.clone()) else {
        return;
    };
    
    // Check if process is currently registered
    let was_enabled = state.process_registry.is_enabled(backend_id);
    
    // Mark for removal and wait for it to stop (using tokio runtime)
    if let Some(token) = state.process_registry.mark_for_removal(backend_id) {
        let _ = handle.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                token.cancelled()
            ).await
        });
    }
    state.process_registry.cleanup_finished();
    
    // Resolve and spawn with fresh config
    match config.resolve_process_backend(backend_id, &proc) {
        Ok(resolved) => {
            let token = CancellationToken::new();
            let enabled = was_enabled && resolved.auto_start.unwrap_or(config.auto_start);
            state.process_registry.register_host(
                backend_id.to_string(),
                token.clone(),
                crate::global_state::ProcState::Stopped,
                enabled,
                resolved.port,
            );
            // Spawn on tokio runtime
            handle.spawn(crate::proc_host::host(
                resolved,
                state.process_registry.clone(),
                state.clone(),
                token,
            ));
        }
        Err(e) => {
            tracing::error!("Failed to restart process {}: {:?}", backend_id, e);
        }
    }
    
    crate::cruma_integration::rebuild_cruma_config(state.clone());
}

async fn save_cruma_mode(state: Arc<GlobalState>, mode: CrumaAuthMode) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();

    match mode {
        CrumaAuthMode::Disabled => {
            guard.cruma = None;
        }
        CrumaAuthMode::Anonymous => {
            guard.cruma = Some(v4::CrumaConfig::Mode("anon".to_string()));
        }
        CrumaAuthMode::Authenticated => {
            let Some(v4::CrumaConfig::Auth { id, key }) = guard.cruma.clone() else {
                return Err("Authenticated credentials are not configured.".to_string());
            };
            guard.cruma = Some(v4::CrumaConfig::Auth { id, key });
        }
    }

    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));

    Ok(())
}

async fn delete_frontend(state: Arc<GlobalState>, host: String) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();

    if let Some(http) = guard.frontends.http.as_mut() {
        http.routes.remove(&host);
    }
    if let Some(https) = guard.frontends.https.as_mut() {
        if let Some(v4::HttpsRoutes::Explicit(routes)) = https.routes.as_mut() {
            routes.remove(&host);
        }
    }

    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    Ok(())
}

async fn load_backend_form(state: Arc<GlobalState>, backend_id: String) -> EditBackendForm {
    let guard = state.config.load_full();
    let mut form = EditBackendForm {
        id: backend_id.clone(),
        ..EditBackendForm::default()
    };

    if let Some(backend) = guard.backends.get(&backend_id) {
        match backend {
            v4::Backend::Process(p) => {
                form.kind = BackendKind::Process;
                form.bin = p.bin.clone();
                form.proc_bin = p.bin.clone();
                form.proc_args = p.args.join(" ");
                form.proc_dir = p.dir.clone().unwrap_or_default();
                form.protocol = p.protocol.clone();
                form.https = p.https;
                form.proc_port = p.port.map(|v| v.to_string()).unwrap_or_default();
                form.proc_auto_start = p.auto_start.unwrap_or(true);
                form.proc_exclude_from_start_all = p.exclude_from_start_all;
                form.proc_log_level = ProcessLogLevelChoice::from_option(&p.log_level);
                let mut env_vec: Vec<(String, String)> = p.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                env_vec.sort_by(|a, b| a.0.cmp(&b.0));
                form.proc_env = env_vec;
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
                form.cache_max_age = s.cache_max_age.map(|v| v.to_string()).unwrap_or_default();
            }
        }
    } else {
        form.kind = BackendKind::Unknown;
    }

    form
}

async fn save_backend_form(
    state: Arc<GlobalState>,
    original_id: Option<String>,
    form: EditBackendForm,
) -> Result<(), String> {
    if form.id.trim().is_empty() {
        return Err("Backend id is required.".to_string());
    }

    let mut guard = (*state.config.load_full()).clone();
    if original_id.as_deref() != Some(&form.id) && guard.backends.contains_key(&form.id) {
        return Err("Backend id already exists.".to_string());
    }

    let key = form.id.clone();
    if let Some(old) = original_id.clone() {
        if old != key {
            guard.backends.remove(&old);
            if let Some(http) = guard.frontends.http.as_mut() {
                for (_, target) in http.routes.iter_mut() {
                    match target {
                        v4::RouteTarget::Simple(b) => {
                            if b == &old {
                                *b = key.clone();
                            }
                        }
                        v4::RouteTarget::Detailed(d) => {
                            if d.backend == old {
                                d.backend = key.clone();
                            }
                        }
                    }
                }
            }
            if let Some(https) = guard.frontends.https.as_mut() {
                if let Some(v4::HttpsRoutes::Explicit(routes)) = https.routes.as_mut() {
                    for (_, target) in routes.iter_mut() {
                        match target {
                            v4::RouteTarget::Simple(b) => {
                                if b == &old {
                                    *b = key.clone();
                                }
                            }
                            v4::RouteTarget::Detailed(d) => {
                                if d.backend == old {
                                    d.backend = key.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

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
                key,
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
                key,
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
            if form.proc_bin.trim().is_empty() {
                return Err("Binary is required.".to_string());
            }

            let port = if form.proc_port.trim().is_empty() {
                None
            } else {
                Some(
                    form.proc_port
                        .trim()
                        .parse::<u16>()
                        .map_err(|_| "Port must be a number.".to_string())?,
                )
            };

            let args: Vec<String> = form
                .proc_args
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            let (proc_id, log_format) = match guard.backends.get(&form.id) {
                Some(v4::Backend::Process(p)) => (p.proc_id.clone(), p.log_format.clone()),
                _ => (ProcId::new(), None),
            };
            let log_level = form.proc_log_level.to_option();
            let env: std::collections::HashMap<String, String> = form
                .proc_env
                .into_iter()
                .filter(|(k, _)| !k.is_empty())
                .collect();

            guard.backends.insert(
                key.clone(),
                v4::Backend::Process(v4::ProcessBackend {
                    proc_id,
                    bin: form.proc_bin.clone(),
                    args,
                    dir: if form.proc_dir.trim().is_empty() {
                        None
                    } else {
                        Some(form.proc_dir.clone())
                    },
                    env,
                    protocol: form.protocol.clone(),
                    https: form.https,
                    port,
                    auto_start: Some(form.proc_auto_start),
                    exclude_from_start_all: form.proc_exclude_from_start_all,
                    log_level,
                    log_format,
                }),
            );
            
            guard.is_valid().map_err(|e| e.to_string())?;
            guard.write_to_disk().map_err(|e| e.to_string())?;
            state.config.store(std::sync::Arc::new(guard));
            
            // Restart the process to pick up config changes
            restart_process_backend_sync(&state, &key);
            
            return Ok(());
        }
        BackendKind::Unknown => {
            return Err("Backend not found.".to_string());
        }
    }

    guard.reload_dashmaps();
    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    Ok(())
}

async fn delete_backend(state: Arc<GlobalState>, backend_id: String) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();
    guard.backends.remove(&backend_id);

    guard.reload_dashmaps();
    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    Ok(())
}

async fn pick_backend_dir() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|p| p.display().to_string())
}

async fn pick_backend_bin() -> Option<String> {
    rfd::FileDialog::new()
        .pick_file()
        .map(|p| p.display().to_string())
}

async fn resolve_backend_dir(
    state: Arc<GlobalState>,
    dir: String,
) -> Result<Option<String>, String> {
    let input = dir.trim();
    if input.is_empty() {
        return Ok(None);
    }

    // Detect unknown variables
    let mut idx = 0;
    while let Some(pos) = input[idx..].find('$') {
        let start = idx + pos + 1;
        let mut end = start;
        for ch in input[start..].chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                end += ch.len_utf8();
            } else {
                break;
            }
        }
        if end > start {
            let var = &input[start..end];
            if var != "root_dir" && var != "cfg_dir" {
                return Err(format!("Unknown variable: ${}", var));
            }
        }
        idx = end;
    }

    let guard = state.config.load_full();
    let probe = v4::StaticBackend {
        dir: input.to_string(),
        index: "index.html".to_string(),
        list_dir: false,
        render_markdown: false,
        cache_max_age: None,
    };

    let resolved = guard
        .resolve_static_backend(&probe)
        .map_err(|e| e.to_string())?;

    let mut resolved_dir = resolved.dir.clone();
    let resolved_path = std::path::Path::new(&resolved_dir);
    if !resolved_path.is_absolute() {
        let base = guard
            .get_parent_path()
            .ok()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        resolved_dir = base.join(resolved_path).display().to_string();
    }

    let canonical = std::fs::canonicalize(&resolved_dir)
        .ok()
        .map(|p| p.display().to_string());

    Ok(Some(canonical.unwrap_or(resolved_dir)))
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
        tray_handle: Option<tray::TrayHandle>,
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
        // Push initial filter to background task
        log_state.set_filter(log_filter.clone());

        let initial_cruma_mode = cruma_mode_from_config(&state.config.load_full());

        (
            Self {
                state,
                log_state,
                current_page: Page::Dashboard,
                theme_mode,
                system_theme: None,
                log_is_at_bottom: true,
                window_id: None,
                window_visible: true,
                tray_handle,
                log_filter,
                log_level_preset,
                expanded_process: None,
                last_seen_filtered_id: None,
                last_log_max_scroll_y: None,
                last_log_viewport_y: None,
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
                edit_frontend_pending_reload: false,
                edit_frontend_original: None,
                edit_frontend_is_new: false,
                edit_backend_form: EditBackendForm::default(),
                edit_backend_notice: None,
                edit_backend_resolved_dir: None,
                edit_backend_resolve_error: None,
                edit_backend_pending_reload: false,
                edit_backend_original: None,
                edit_backend_is_new: false,
                edit_backend_confirm_delete: false,
                edit_backend_env_new_key: String::new(),
                edit_backend_env_new_value: String::new(),
                cruma_auth_mode: initial_cruma_mode,
                cruma_mode_notice: None,
                processes_tab: ProcessesTab::default(),
                global_env_vars: Vec::new(),
                global_env_new_key: String::new(),
                global_env_new_value: String::new(),
                global_env_notice: None,
                global_env_dirty: false,
                exit_requested: false,
                tray_quit_pending: false,
                traffic_inspection_selected: None,
                frontend_http_port_input: String::new(),
                frontend_https_port_input: String::new(),
                frontend_port_notice: None,
                frontend_ports_dirty: false,
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
        let resize_sub = window::resize_events().map(|(id, size)| Message::WindowResized(id, size));
        let exit_sub =
            time::every(std::time::Duration::from_millis(250)).map(|_| Message::ExitPoll);
        // Listen for window close requests (to hide instead of quit on macOS)
        let close_sub = window::close_requests().map(Message::WindowCloseRequested);
        // Listen for window events (focus/unfocus to detect minimize)
        let window_events_sub = window::events().map(|(id, event)| {
            match event {
                iced::window::Event::Focused => Message::WindowFocused(id),
                iced::window::Event::Unfocused => Message::WindowUnfocused(id),
                _ => Message::NoOp,
            }
        });
        // Poll for tray commands
        let tray_sub = time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayCommandReceived);

        Subscription::batch(vec![page_sub, theme_sub, resize_sub, exit_sub, close_sub, window_events_sub, tray_sub])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => {}
            Message::NavigateTo(page) => {
                self.current_page = page;
                self.dashboard_process_menu = None;
                if page == Page::Monitoring {
                    if self.log_auto_tail {
                        self.log_is_at_bottom = true;
                        return snap_log_to_bottom();
                    }
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
            Message::WindowResized(id, size) => {
                // Capture window ID for tray commands
                if self.window_id.is_none() {
                    self.window_id = Some(id);
                }
                set_gui_text_scale(size);
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
                let at_bottom_now = max_scroll_y - current_y <= 4.0;
                let content_grew = self
                    .last_log_max_scroll_y
                    .map(|prev| max_scroll_y > prev + 0.5)
                    .unwrap_or(false);
                let user_scrolled = self
                    .last_log_viewport_y
                    .map(|prev| (current_y - prev).abs() > 0.5)
                    .unwrap_or(false);

                // If auto-tail is on, and we were at bottom, and the only change is that
                // content grew (new logs), keep tailing instead of entering paused state.
                self.log_is_at_bottom = if self.log_auto_tail
                    && self.log_is_at_bottom
                    && content_grew
                    && !user_scrolled
                {
                    true
                } else {
                    at_bottom_now
                };

                self.last_log_max_scroll_y = Some(max_scroll_y);
                self.last_log_viewport_y = Some(current_y);
            }
            Message::Tick => {
                if self.current_page == Page::Monitoring {
                    let filtered = self.log_state.filtered_snapshot();
                    let last_id = filtered.last_filtered_id;
                    if last_id != self.last_seen_filtered_id {
                        self.last_seen_filtered_id = last_id;
                        if self.log_auto_tail && self.log_is_at_bottom {
                            return snap_log_to_bottom();
                        }
                    }
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
            Message::ExitPoll => {
                if !self.exit_requested && self.state.exit.load(std::sync::atomic::Ordering::SeqCst)
                {
                    self.exit_requested = true;
                    return window::oldest().map(Message::ExitWindowId);
                }
            }
            Message::ExitWindowId(id) => {
                if let Some(id) = id {
                    return window::close(id);
                }
            }
            Message::WindowCloseRequested(id) => {
                // On macOS with tray available, hide the window instead of closing
                #[cfg(target_os = "macos")]
                if self.tray_handle.is_some() {
                    self.window_visible = false;
                    if let Some(ref tray) = self.tray_handle {
                        tray.set_window_visible(false);
                    }
                    // Remove from Dock when hidden
                    macos_app_icon::set_activation_policy(macos_app_icon::ActivationPolicy::Accessory);
                    return window::set_mode(id, window::Mode::Hidden);
                }
                
                // On other platforms or if no tray on macOS, close normally
                #[cfg(not(target_os = "macos"))]
                {
                    self.state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                    return window::close(id);
                }
                
                // macOS without tray - also close normally
                #[cfg(target_os = "macos")]
                {
                    self.state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                    return window::close(id);
                }
            }
            Message::WindowIdResolved(id) => {
                self.window_id = id;
            }
            Message::WindowFocused(_id) => {
                // Window is now focused (restored from minimize or brought to front)
                self.window_visible = true;
                if let Some(ref tray) = self.tray_handle {
                    tray.set_window_visible(true);
                }
            }
            Message::WindowUnfocused(id) => {
                // Window lost focus - could be minimized or just clicked away
                // Check if actually minimized
                return window::is_minimized(id).map(Message::WindowMinimizedCheck);
            }
            Message::WindowMinimizedCheck(is_minimized) => {
                // Update tray based on minimized state
                if is_minimized == Some(true) {
                    self.window_visible = false;
                    if let Some(ref tray) = self.tray_handle {
                        tray.set_window_visible(false);
                    }
                }
            }
            Message::TrayCommandReceived => {
                // Poll for tray commands
                if let Some(ref tray) = self.tray_handle {
                    while let Ok(cmd) = tray.command_rx.try_recv() {
                        match cmd {
                            tray::TrayCommand::Show => {
                                self.window_visible = true;
                                tray.set_window_visible(true);
                                // Show in Dock when window is visible and reapply icon
                                #[cfg(target_os = "macos")]
                                {
                                    macos_app_icon::set_activation_policy(macos_app_icon::ActivationPolicy::Regular);
                                    macos_app_icon::apply_default_icon();
                                }
                                if let Some(id) = self.window_id {
                                    return Task::batch(vec![
                                        window::minimize(id, false), // Unminimize if minimized
                                        window::set_mode(id, window::Mode::Windowed),
                                        window::gain_focus(id),
                                    ]);
                                } else {
                                    // Window doesn't exist, need to open a new one
                                    // For now, just log - full implementation would open new window
                                    tracing::info!("Show requested but no window ID available");
                                }
                            }
                            tray::TrayCommand::Hide => {
                                self.window_visible = false;
                                tray.set_window_visible(false);
                                // Remove from Dock when hidden
                                #[cfg(target_os = "macos")]
                                macos_app_icon::set_activation_policy(macos_app_icon::ActivationPolicy::Accessory);
                                if let Some(id) = self.window_id {
                                    return window::set_mode(id, window::Mode::Hidden);
                                }
                            }
                            tray::TrayCommand::Quit => {
                                // Phase 1: Immediate visual feedback
                                // Gray out the tray icon and disable menu items
                                tray.set_shutting_down();
                                
                                // Remove from Dock immediately
                                #[cfg(target_os = "macos")]
                                macos_app_icon::set_activation_policy(macos_app_icon::ActivationPolicy::Accessory);
                                
                                // Mark that we're in quit pending state
                                self.tray_quit_pending = true;
                                
                                // Hide window immediately, then trigger phase 2 after a small delay
                                // to ensure the window system has time to actually hide the window
                                if let Some(id) = self.window_id {
                                    return window::set_mode(id, window::Mode::Hidden)
                                        .chain(Task::perform(
                                            async {
                                                // Small delay to let the window hide visually
                                                std::thread::sleep(std::time::Duration::from_millis(50));
                                            },
                                            |_| Message::TrayQuitPhase2,
                                        ));
                                }
                                // No window, go directly to phase 2
                                return Task::perform(async {}, |_| Message::TrayQuitPhase2);
                            }
                        }
                    }
                }
            }
            Message::TrayQuitPhase2 => {
                // Phase 2: Now that visual feedback is complete, trigger actual exit
                // We set the exit flag which signals main.rs to start graceful shutdown.
                // The GUI exits, allowing gui::run() to return, and main.rs handles
                // waiting for processes to stop (up to 30 seconds).
                if self.tray_quit_pending {
                    self.tray_quit_pending = false;
                    self.state.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                    
                    // Exit the GUI - this allows gui::run() to return and main.rs
                    // will handle the graceful shutdown of all processes
                    if let Some(id) = self.window_id {
                        return Task::batch(vec![
                            window::close(id),
                            iced::exit(),
                        ]);
                    }
                    return iced::exit();
                }
            }
            Message::ConfigUpdated(config) => {
                // Sync global env vars from config if user hasn't made local edits
                if !self.global_env_dirty {
                    self.global_env_vars = config.global_env.clone();
                }
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
                if !self.frontend_ports_dirty {
                    self.frontend_http_port_input = self
                        .cached_config
                        .http_port
                        .map(|p| p.to_string())
                        .unwrap_or_default();
                    self.frontend_https_port_input = self
                        .cached_config
                        .https_port
                        .map(|p| p.to_string())
                        .unwrap_or_default();
                }
                if self.edit_frontend_pending_reload {
                    self.edit_frontend_notice = Some("Saved.".to_string());
                    self.edit_frontend_pending_reload = false;
                }
                if self.edit_backend_pending_reload {
                    self.edit_backend_notice = Some("Saved.".to_string());
                    self.edit_backend_pending_reload = false;
                }
                self.cruma_auth_mode = cruma_mode_from_config(&self.state.config.load_full());
            }
            Message::LogFilterTextChanged(text) => {
                self.log_filter.text = text;
                self.log_state.set_filter(self.log_filter.clone());
            }
            Message::LogLevelPresetChanged(preset) => {
                self.log_level_preset = preset;
                Self::apply_log_level_preset(&mut self.log_filter, preset);
                self.log_state.set_filter(self.log_filter.clone());
            }
            Message::LogFilterToggleSource(source, enabled) => {
                if enabled {
                    self.log_filter.sources.insert(source);
                } else {
                    self.log_filter.sources.remove(&source);
                }
                self.log_state.set_filter(self.log_filter.clone());
            }
            Message::LogFilterClearSources => {
                self.log_filter.sources.clear();
                self.log_state.set_filter(self.log_filter.clone());
            }
            Message::LogsClear => {
                self.log_state.clear();
                self.last_seen_filtered_id = None;
                self.last_log_max_scroll_y = None;
                self.last_log_viewport_y = None;
                self.log_is_at_bottom = true;
            }

            Message::LogToggleWrap(enabled) => {
                self.log_wrap_enabled = enabled;
            }
            Message::LogToggleAutoTail(enabled) => {
                self.log_auto_tail = enabled;
                if enabled {
                    self.last_log_max_scroll_y = None;
                    self.last_log_viewport_y = None;
                    self.log_is_at_bottom = true;
                    return snap_log_to_bottom();
                }
            }
            Message::FrontendHttpPortChanged(value) => {
                self.frontend_http_port_input = value;
                self.frontend_ports_dirty = true;
            }
            Message::FrontendHttpsPortChanged(value) => {
                self.frontend_https_port_input = value;
                self.frontend_ports_dirty = true;
            }
            Message::FrontendPortsSave => {
                self.frontend_port_notice = None;
                return Task::perform(
                    save_frontend_ports(
                        self.state.clone(),
                        self.frontend_http_port_input.clone(),
                        self.frontend_https_port_input.clone(),
                    ),
                    Message::FrontendPortsSaveResult,
                );
            }
            Message::FrontendPortsSaveResult(result) => match result {
                Ok(()) => {
                    self.frontend_port_notice = Some("Frontend ports updated.".to_string());
                    self.frontend_ports_dirty = false;
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
                Err(err) => {
                    self.frontend_port_notice = Some(err);
                }
            },
            Message::ProcessStart(name) => {
                self.state.process_registry.set_enabled(&name, true);
            }
            Message::ProcessStop(name) => {
                self.state.process_registry.set_enabled(&name, false);
            }
            Message::ProcessStartAll => {
                for proc in &self.cached_config.processes {
                    self.state.process_registry.set_enabled(&proc.name, true);
                }
            }
            Message::ProcessStopAll => {
                for proc in &self.cached_config.processes {
                    self.state.process_registry.set_enabled(&proc.name, false);
                }
            }
            Message::ProcessToggleDetails(name) => {
                if self.expanded_process.as_deref() == Some(&name) {
                    self.expanded_process = None;
                } else {
                    self.expanded_process = Some(name);
                }
            }
            Message::ProcessesTabChanged(tab) => {
                self.processes_tab = tab;
                // Reset notice when switching tabs
                self.global_env_notice = None;
            }
            Message::GlobalEnvKeyChanged(idx, val) => {
                if let Some(entry) = self.global_env_vars.get_mut(idx) {
                    entry.0 = val;
                    self.global_env_dirty = true;
                }
            }
            Message::GlobalEnvValueChanged(idx, val) => {
                if let Some(entry) = self.global_env_vars.get_mut(idx) {
                    entry.1 = val;
                    self.global_env_dirty = true;
                }
            }
            Message::GlobalEnvRemove(idx) => {
                if idx < self.global_env_vars.len() {
                    self.global_env_vars.remove(idx);
                    self.global_env_dirty = true;
                }
            }
            Message::GlobalEnvNewKeyChanged(val) => {
                self.global_env_new_key = val;
            }
            Message::GlobalEnvNewValueChanged(val) => {
                self.global_env_new_value = val;
            }
            Message::GlobalEnvAdd => {
                let key = self.global_env_new_key.trim().to_string();
                let value = self.global_env_new_value.trim().to_string();
                if !key.is_empty() {
                    self.global_env_vars.push((key, value));
                    self.global_env_new_key.clear();
                    self.global_env_new_value.clear();
                    self.global_env_dirty = true;
                }
            }
            Message::GlobalEnvSave => {
                let vars = self.global_env_vars.clone();
                return Task::perform(
                    save_global_env(self.state.clone(), vars),
                    Message::GlobalEnvSaveResult,
                );
            }
            Message::GlobalEnvSaveResult(result) => {
                match result {
                    Ok(()) => {
                        self.global_env_notice = Some("Global environment variables saved.".to_string());
                        self.global_env_dirty = false;
                        return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                    }
                    Err(e) => {
                        self.global_env_notice = Some(format!("Error: {e}"));
                    }
                }
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
                self.edit_frontend_original = Some(name.clone());
                self.edit_frontend_is_new = false;
                return Task::perform(
                    load_frontend_form(self.state.clone(), name),
                    Message::EditFrontendLoaded,
                );
            }
            Message::OpenNewFrontend => {
                self.edit_target = None;
                self.current_page = Page::EditFrontend;
                self.dashboard_process_menu = None;
                self.edit_frontend_notice = None;
                self.edit_frontend_form = EditFrontendForm::default();
                self.edit_frontend_original = None;
                self.edit_frontend_is_new = true;
            }
            Message::OpenEditBackend(name) => {
                self.edit_target = Some(name);
                self.current_page = Page::EditBackend;
                self.dashboard_process_menu = None;
                self.edit_backend_notice = None;
                self.edit_backend_original = self.edit_target.clone();
                self.edit_backend_is_new = false;
                self.edit_backend_confirm_delete = false;
                return Task::perform(
                    load_backend_form(self.state.clone(), self.edit_target.clone().unwrap()),
                    Message::EditBackendLoaded,
                );
            }
            Message::OpenNewBackend(kind) => {
                self.edit_target = None;
                self.current_page = Page::EditBackend;
                self.dashboard_process_menu = None;
                self.edit_backend_notice = None;
                self.edit_backend_form = EditBackendForm {
                    kind,
                    ..EditBackendForm::default()
                };
                self.edit_backend_original = None;
                self.edit_backend_is_new = true;
                self.edit_backend_confirm_delete = false;
            }
            Message::EditFrontendLoaded(form) => {
                self.edit_frontend_form = form;
            }
            Message::EditFrontendHostChanged(value) => {
                self.edit_frontend_form.hostname = value;
            }
            Message::EditFrontendBackendChanged(value) => {
                self.edit_frontend_form.backend = value.id;
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
                let original_host = self.edit_frontend_original.clone();
                return Task::perform(
                    save_frontend_form(self.state.clone(), original_host, form),
                    Message::EditFrontendSaveResult,
                );
            }
            Message::EditFrontendSaveResult(result) => match result {
                Ok(_) => {
                    self.edit_target = Some(self.edit_frontend_form.hostname.clone());
                    self.edit_frontend_notice = Some("Saved. Waiting for reload...".to_string());
                    self.edit_frontend_pending_reload = true;
                    self.edit_frontend_is_new = false;
                    self.edit_frontend_original = Some(self.edit_frontend_form.hostname.clone());
                }
                Err(err) => {
                    self.edit_frontend_notice = Some(err);
                }
            },
            Message::EditFrontendDelete => {
                if let Some(host) = self.edit_frontend_original.clone() {
                    self.edit_frontend_notice = None;
                    return Task::perform(
                        delete_frontend(self.state.clone(), host),
                        Message::EditFrontendDeleteResult,
                    );
                }
            }
            Message::EditFrontendDeleteResult(result) => match result {
                Ok(_) => {
                    self.edit_frontend_notice = Some("Deleted. Waiting for reload...".to_string());
                    self.edit_frontend_pending_reload = true;
                    self.edit_frontend_is_new = true;
                    self.edit_frontend_original = None;
                }
                Err(err) => {
                    self.edit_frontend_notice = Some(err);
                }
            },
            Message::EditBackendLoaded(form) => {
                self.edit_backend_form = form;
                self.edit_backend_resolved_dir = None;
                self.edit_backend_resolve_error = None;
                if matches!(self.edit_backend_form.kind, BackendKind::Static) {
                    let dir = self.edit_backend_form.dir.clone();
                    return Task::perform(
                        resolve_backend_dir(self.state.clone(), dir),
                        Message::EditBackendResolvedDir,
                    );
                }
            }
            Message::EditBackendFieldChanged(field) => match field {
                EditBackendField::Id(v) => self.edit_backend_form.id = v,
                EditBackendField::Endpoints(v) => self.edit_backend_form.endpoints = v,
                EditBackendField::Protocol(v) => self.edit_backend_form.protocol = v,
                EditBackendField::Https(v) => self.edit_backend_form.https = v,
                EditBackendField::KeepOriginalHostHeader(v) => {
                    self.edit_backend_form.keep_original_host_header = v;
                }
                EditBackendField::Dir(v) => {
                    self.edit_backend_form.dir = v.clone();
                    return Task::perform(
                        resolve_backend_dir(self.state.clone(), v),
                        Message::EditBackendResolvedDir,
                    );
                }
                EditBackendField::ListDir(v) => self.edit_backend_form.list_dir = v,
                EditBackendField::RenderMarkdown(v) => self.edit_backend_form.render_markdown = v,
                EditBackendField::CacheMaxAge(v) => self.edit_backend_form.cache_max_age = v,
                EditBackendField::ProcBin(v) => self.edit_backend_form.proc_bin = v,
                EditBackendField::ProcArgs(v) => self.edit_backend_form.proc_args = v,
                EditBackendField::ProcDir(v) => self.edit_backend_form.proc_dir = v,
                EditBackendField::ProcPort(v) => self.edit_backend_form.proc_port = v,
                EditBackendField::ProcAutoStart(v) => self.edit_backend_form.proc_auto_start = v,
                EditBackendField::ProcExcludeFromStartAll(v) => {
                    self.edit_backend_form.proc_exclude_from_start_all = v;
                }
                EditBackendField::ProcLogLevel(v) => self.edit_backend_form.proc_log_level = v,
            },
            Message::EditBackendPickDir => {
                return Task::perform(pick_backend_dir(), Message::EditBackendDirPicked);
            }
            Message::EditBackendDirPicked(path) => {
                if let Some(p) = path {
                    self.edit_backend_form.dir = p;
                    let dir = self.edit_backend_form.dir.clone();
                    return Task::perform(
                        resolve_backend_dir(self.state.clone(), dir),
                        Message::EditBackendResolvedDir,
                    );
                }
            }
            Message::EditBackendPickBin => {
                return Task::perform(pick_backend_bin(), Message::EditBackendBinPicked);
            }
            Message::EditBackendBinPicked(path) => {
                if let Some(p) = path {
                    self.edit_backend_form.proc_bin = p;
                }
            }
            Message::EditBackendEnvKeyChanged(idx, val) => {
                if let Some(entry) = self.edit_backend_form.proc_env.get_mut(idx) {
                    entry.0 = val;
                }
            }
            Message::EditBackendEnvValueChanged(idx, val) => {
                if let Some(entry) = self.edit_backend_form.proc_env.get_mut(idx) {
                    entry.1 = val;
                }
            }
            Message::EditBackendEnvRemove(idx) => {
                if idx < self.edit_backend_form.proc_env.len() {
                    self.edit_backend_form.proc_env.remove(idx);
                }
            }
            Message::EditBackendEnvNewKeyChanged(val) => {
                self.edit_backend_env_new_key = val;
            }
            Message::EditBackendEnvNewValueChanged(val) => {
                self.edit_backend_env_new_value = val;
            }
            Message::EditBackendEnvAdd => {
                let key = self.edit_backend_env_new_key.trim().to_string();
                let value = self.edit_backend_env_new_value.trim().to_string();
                if !key.is_empty() {
                    self.edit_backend_form.proc_env.push((key, value));
                    self.edit_backend_env_new_key.clear();
                    self.edit_backend_env_new_value.clear();
                }
            }
            Message::EditBackendResolveDir(dir) => {
                return Task::perform(
                    resolve_backend_dir(self.state.clone(), dir),
                    Message::EditBackendResolvedDir,
                );
            }
            Message::EditBackendResolvedDir(result) => match result {
                Ok(resolved) => {
                    self.edit_backend_resolved_dir = resolved;
                    self.edit_backend_resolve_error = None;
                }
                Err(err) => {
                    self.edit_backend_resolved_dir = None;
                    self.edit_backend_resolve_error = Some(err);
                }
            },
            Message::EditBackendSave => {
                self.edit_backend_notice = None;
                let form = self.edit_backend_form.clone();
                return Task::perform(
                    save_backend_form(self.state.clone(), self.edit_backend_original.clone(), form),
                    Message::EditBackendSaveResult,
                );
            }
            Message::EditBackendSaveResult(result) => match result {
                Ok(_) => {
                    self.edit_backend_notice = Some("Saved. Waiting for reload...".to_string());
                    self.edit_backend_pending_reload = true;
                    self.edit_backend_is_new = false;
                    self.edit_backend_original = Some(self.edit_backend_form.id.clone());
                    self.edit_backend_confirm_delete = false;
                }
                Err(err) => {
                    self.edit_backend_notice = Some(err);
                }
            },
            Message::EditBackendDelete => {
                if let Some(id) = self.edit_backend_original.clone() {
                    if !self.edit_backend_confirm_delete {
                        let routes_count = self
                            .cached_config
                            .routes
                            .iter()
                            .filter(|r| r.backend == id)
                            .count();
                        if routes_count > 0 {
                            self.edit_backend_notice = Some(format!(
                                "Warning: {} route(s) still point to this backend. Delete again to confirm.",
                                routes_count
                            ));
                            self.edit_backend_confirm_delete = true;
                            return Task::none();
                        }
                    }

                    self.edit_backend_notice = None;
                    self.edit_backend_confirm_delete = false;
                    return Task::perform(
                        delete_backend(self.state.clone(), id),
                        Message::EditBackendDeleteResult,
                    );
                }
            }
            Message::EditBackendDeleteResult(result) => match result {
                Ok(_) => {
                    self.edit_backend_notice = Some("Deleted. Waiting for reload...".to_string());
                    self.edit_backend_pending_reload = true;
                    self.edit_backend_is_new = true;
                    self.edit_backend_original = None;
                }
                Err(err) => {
                    self.edit_backend_notice = Some(err);
                }
            },
            Message::CrumaAuthModeChanged(mode) => {
                self.cruma_auth_mode = mode;
                self.cruma_mode_notice = None;
                return Task::perform(
                    save_cruma_mode(self.state.clone(), mode),
                    Message::CrumaAuthModeSaveResult,
                );
            }
            Message::CrumaAuthModeSaveResult(result) => match result {
                Ok(()) => {
                    self.cruma_mode_notice = Some("Cruma mode updated.".to_string());
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
                Err(err) => {
                    self.cruma_mode_notice = Some(err);
                }
            },
            Message::TrafficInspectionToggled(enabled) => {
                self.state
                    .enable_global_traffic_inspection
                    .store(enabled, std::sync::atomic::Ordering::Relaxed);
                self.state.http_capture_store.set_enabled(enabled);
            }
            Message::TrafficInspectionClear => {
                self.state.http_capture_store.clear();
                self.traffic_inspection_selected = None;
            }
            Message::TrafficInspectionSelect(req_id) => {
                self.traffic_inspection_selected = req_id;
            }
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
        .style(|theme: &Theme| {
            let bg = theme.palette().background;
            let use_glass_effects = cfg!(target_os = "macos");
            container::Style {
                background: Some(Background::Color(if use_glass_effects {
                    Color::TRANSPARENT
                } else {
                    Color::from_rgba(bg.r, bg.g, bg.b, 1.0)
                })),
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
        let logo_handle = if is_light {
            SIDEBAR_LOGO_DARK.clone()
        } else {
            SIDEBAR_LOGO_LIGHT.clone()
        };
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
            Page::TrafficInspection,
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
                let use_glass_effects = cfg!(target_os = "macos");
                let bg = theme.extended_palette().background.base.color;
                let base_bg = theme.palette().background;
                let is_light = (0.299 * base_bg.r + 0.587 * base_bg.g + 0.114 * base_bg.b) > 0.5;
                let (shade, alpha) = if use_glass_effects {
                    let shade = if is_light { 0.7 } else { 0.52 };
                    let alpha = if is_light { 0.07 } else { 0.106 };
                    (shade, alpha)
                } else {
                    (1.0, 1.0)
                };
                container::Style {
                    background: Some(Background::Color(Color::from_rgba(
                        bg.r * shade,
                        bg.g * shade,
                        bg.b * shade,
                        alpha,
                    ))),
                    border: Border::default(),
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

                let (background, text_color) = if is_active {
                    let bg_color = palette.primary.strong.color;
                    // Always use white on active to ensure contrast on purple background.
                    (bg_color, Color::WHITE)
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
            Page::CrumaIngress => self.view_cruma_ingress(),
            Page::Monitoring => self.view_monitoring(),
            Page::TrafficInspection => self.view_traffic_inspection(),
            Page::Backends => self.view_backends(),
            Page::Frontends => self.view_frontends(),
            Page::ManagedProcesses => self.view_processes(),
            Page::EditFrontend => self.view_edit_frontend(),
            Page::EditBackend => self.view_edit_backend(),
        };

        // These pages handle their own layout (no extra scrollable wrapper)
        if self.current_page == Page::Monitoring || self.current_page == Page::TrafficInspection {
            page_content
        } else {
            let page_title = text(self.current_page.title()).size(text_size(20));
            let content = column![page_title, page_content]
                .spacing(20)
                .padding(30)
                .width(Length::Fill);

            Scrollable::new(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &Theme, status| scrollable::Style {
                    container: container::Style {
                        background: Some(Background::Color({
                            let use_glass_effects = cfg!(target_os = "macos");
                            let palette = theme.extended_palette();
                            let bg = palette.background.weak.color;
                            let base_bg = theme.palette().background;
                            let is_light = (0.299 * base_bg.r
                                + 0.587 * base_bg.g
                                + 0.114 * base_bg.b)
                                > 0.5;
                            let shade = if is_light { 1.0 } else { 0.5 };
                            let alpha = if use_glass_effects {
                                if is_light { 0.22 } else { 0.30 }
                            } else {
                                1.0
                            };
                            Color::from_rgba(bg.r * shade, bg.g * shade, bg.b * shade, alpha)
                        })),
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
