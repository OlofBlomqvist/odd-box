pub mod components;
pub mod logs;
mod macos_app_icon;
mod pages;
mod tray;

use iced::widget::scrollable::RelativeOffset;
use iced::widget::{Column, Scrollable, button, column, container, image, row, scrollable, text};

use iced::{
    Background, Border, Color, Element, Length, Padding, Subscription, Task, Theme, event,
    keyboard, system, theme, time, window,
};
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
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
const WINDOW_INITIAL_WIDTH: f32 = 1200.0;
const WINDOW_INITIAL_HEIGHT: f32 = 800.0;
const WINDOW_MIN_WIDTH: f32 = 920.0;
const WINDOW_MIN_HEIGHT: f32 = 560.0;
const SIDEBAR_WIDTH: f32 = 220.0;

fn compute_text_scale(size: iced::Size) -> f32 {
    let width_scale = (size.width / WINDOW_INITIAL_WIDTH).clamp(0.85, 1.35);
    let height_scale = (size.height / WINDOW_INITIAL_HEIGHT).clamp(0.85, 1.35);
    width_scale.min(height_scale)
}

fn set_gui_text_scale(size: iced::Size) {
    GUI_TEXT_SCALE_BITS.store(compute_text_scale(size).to_bits(), Ordering::Relaxed);
}

fn use_glass_effects() -> bool {
    cfg!(target_os = "macos")
}

fn platform_surface_color(color: Color, glass_alpha: f32) -> Color {
    if use_glass_effects() {
        Color::from_rgba(color.r, color.g, color.b, glass_alpha)
    } else {
        Color::from_rgb(color.r, color.g, color.b)
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::gui) enum KdeButtonRole {
    Neutral,
    Primary,
    Success,
    Danger,
}

fn muted(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(color: Color) -> f32 {
    0.2126 * srgb_to_linear(color.r)
        + 0.7152 * srgb_to_linear(color.g)
        + 0.0722 * srgb_to_linear(color.b)
}

fn contrast_ratio(a: Color, b: Color) -> f32 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

pub(in crate::gui) fn readable_on(background: Color, preferred: Color, fallback: Color) -> Color {
    const MIN_CONTRAST: f32 = 3.0;
    let preferred_ratio = contrast_ratio(background, preferred);
    if preferred_ratio >= MIN_CONTRAST {
        return preferred;
    }

    let fallback_ratio = contrast_ratio(background, fallback);
    let white_ratio = contrast_ratio(background, Color::WHITE);
    let black_ratio = contrast_ratio(background, Color::BLACK);

    let mut best = fallback;
    let mut best_ratio = fallback_ratio;
    if white_ratio > best_ratio {
        best = Color::WHITE;
        best_ratio = white_ratio;
    }
    if black_ratio > best_ratio {
        best = Color::BLACK;
    }
    best
}

pub(in crate::gui) fn kde_button_style(
    theme: &Theme,
    status: button::Status,
    role: KdeButtonRole,
) -> button::Style {
    let palette = theme.extended_palette();
    let base_bg = palette.background.base.color;

    let (default_text, accent) = match role {
        KdeButtonRole::Neutral => (palette.background.base.text, palette.primary.base.color),
        KdeButtonRole::Primary => (palette.primary.base.color, palette.primary.base.color),
        KdeButtonRole::Success => (palette.success.base.color, palette.success.base.color),
        KdeButtonRole::Danger => (palette.danger.base.color, palette.danger.base.color),
    };

    let neutral_hover_bg = if palette.is_dark {
        theme::palette::mix(base_bg, Color::WHITE, 0.10)
    } else {
        theme::palette::mix(base_bg, Color::BLACK, 0.06)
    };
    let neutral_pressed_bg = if palette.is_dark {
        theme::palette::mix(base_bg, Color::WHITE, 0.16)
    } else {
        theme::palette::mix(base_bg, Color::BLACK, 0.10)
    };
    let role_hover_bg =
        theme::palette::mix(accent, base_bg, if palette.is_dark { 0.88 } else { 0.92 });
    let role_pressed_bg =
        theme::palette::mix(accent, base_bg, if palette.is_dark { 0.80 } else { 0.86 });

    let (background, text_color, border_color, border_width) = match status {
        button::Status::Disabled => (
            Color::TRANSPARENT,
            muted(default_text, 0.45),
            Color::TRANSPARENT,
            0.0,
        ),
        button::Status::Hovered => {
            let bg = if matches!(role, KdeButtonRole::Neutral) {
                neutral_hover_bg
            } else {
                role_hover_bg
            };
            (bg, default_text, muted(accent, 0.65), 1.0)
        }
        button::Status::Pressed => {
            let bg = if matches!(role, KdeButtonRole::Neutral) {
                neutral_pressed_bg
            } else {
                role_pressed_bg
            };
            (bg, default_text, muted(accent, 0.75), 1.0)
        }
        _ => (Color::TRANSPARENT, default_text, Color::TRANSPARENT, 0.0),
    };

    button::Style {
        background: Some(background.into()),
        text_color,
        border: Border {
            radius: 6.0.into(),
            width: border_width,
            color: border_color,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(in crate::gui) fn kde_neutral_button_style(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    kde_button_style(theme, status, KdeButtonRole::Neutral)
}

pub(in crate::gui) fn kde_primary_button_style(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    kde_button_style(theme, status, KdeButtonRole::Primary)
}

#[allow(dead_code)]
pub(in crate::gui) fn kde_success_button_style(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    kde_button_style(theme, status, KdeButtonRole::Success)
}

#[allow(dead_code)]
pub(in crate::gui) fn kde_danger_button_style(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    kde_button_style(theme, status, KdeButtonRole::Danger)
}

/// Unified button style that works for both KDE and non-KDE, light and dark themes.
/// Use this everywhere instead of ad-hoc inline styles.
pub(in crate::gui) fn themed_button_style(
    theme: &Theme,
    status: button::Status,
    role: KdeButtonRole,
    use_kde: bool,
) -> button::Style {
    if use_kde {
        return kde_button_style(theme, status, role);
    }

    let palette = theme.extended_palette();
    let is_dark = palette.is_dark;

    match role {
        KdeButtonRole::Neutral => {
            let border_color = if is_dark {
                Color::from_rgba(1.0, 1.0, 1.0, 0.25)
            } else {
                Color::from_rgba(0.0, 0.0, 0.0, 0.20)
            };
            let (bg, fg) = match status {
                button::Status::Hovered => {
                    let bg = if is_dark {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.06)
                    };
                    (bg, palette.background.base.text)
                }
                button::Status::Pressed => {
                    let bg = if is_dark {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.15)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.10)
                    };
                    (bg, palette.background.base.text)
                }
                button::Status::Disabled => {
                    (Color::TRANSPARENT, muted(palette.background.base.text, 0.4))
                }
                _ => (Color::TRANSPARENT, palette.background.base.text),
            };
            let r = scaled(4.0);
            button::Style {
                background: Some(bg.into()),
                text_color: fg,
                border: Border {
                    radius: r.into(),
                    width: 1.0,
                    color: if matches!(status, button::Status::Disabled) {
                        muted(border_color, 0.3)
                    } else {
                        border_color
                    },
                },
                ..Default::default()
            }
        }
        KdeButtonRole::Primary | KdeButtonRole::Success | KdeButtonRole::Danger => {
            let (normal_bg, hover_bg) = match role {
                KdeButtonRole::Primary => {
                    if is_dark {
                        (
                            Color::from_rgb(0.25, 0.48, 0.85),
                            Color::from_rgb(0.35, 0.56, 0.92),
                        )
                    } else {
                        (
                            Color::from_rgb(0.20, 0.42, 0.75),
                            Color::from_rgb(0.16, 0.36, 0.68),
                        )
                    }
                }
                KdeButtonRole::Success => {
                    if is_dark {
                        (
                            Color::from_rgb(0.22, 0.58, 0.28),
                            Color::from_rgb(0.30, 0.68, 0.36),
                        )
                    } else {
                        (
                            Color::from_rgb(0.18, 0.52, 0.22),
                            Color::from_rgb(0.14, 0.45, 0.18),
                        )
                    }
                }
                KdeButtonRole::Danger => {
                    if is_dark {
                        (
                            Color::from_rgb(0.72, 0.24, 0.24),
                            Color::from_rgb(0.82, 0.32, 0.32),
                        )
                    } else {
                        (
                            Color::from_rgb(0.65, 0.18, 0.18),
                            Color::from_rgb(0.58, 0.12, 0.12),
                        )
                    }
                }
                _ => unreachable!(),
            };

            let (bg, fg) = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    (hover_bg, readable_on(hover_bg, Color::WHITE, Color::BLACK))
                }
                button::Status::Disabled => {
                    let disabled_bg = Color::from_rgba(normal_bg.r, normal_bg.g, normal_bg.b, 0.35);
                    (disabled_bg, Color::from_rgba(1.0, 1.0, 1.0, 0.45))
                }
                _ => (
                    normal_bg,
                    readable_on(normal_bg, Color::WHITE, Color::BLACK),
                ),
            };

            let r = scaled(4.0);
            button::Style {
                background: Some(bg.into()),
                text_color: fg,
                border: Border {
                    radius: r.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_config_home() -> Option<PathBuf> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        let config_home = config_home.trim();
        if !config_home.is_empty() {
            return Some(PathBuf::from(config_home));
        }
    }
    let home = std::env::var("HOME").ok()?;
    let home = home.trim();
    if home.is_empty() {
        None
    } else {
        Some(PathBuf::from(home).join(".config"))
    }
}

#[cfg(target_os = "linux")]
fn parse_kde_rgb(value: &str) -> Option<Color> {
    let mut parts = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .take(3)
        .map(|part| part.parse::<u16>().ok());

    let r = parts.next().flatten()?;
    let g = parts.next().flatten()?;
    let b = parts.next().flatten()?;

    if r > 255 || g > 255 || b > 255 {
        return None;
    }

    Some(Color::from_rgb(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ))
}

#[cfg(target_os = "linux")]
fn read_kdeglobals_colors() -> Option<HashMap<String, Color>> {
    let path = linux_config_home()?.join("kdeglobals");
    let contents = std::fs::read_to_string(path).ok()?;

    let mut colors = HashMap::new();
    let mut section = String::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if let Some(color) = parse_kde_rgb(value) {
            let key = key.trim();
            if !section.is_empty() && !key.is_empty() {
                colors.insert(format!("{section}.{key}"), color);
            }
        }
    }

    if colors.is_empty() {
        None
    } else {
        Some(colors)
    }
}

#[cfg(target_os = "linux")]
fn kde_color(
    colors: &HashMap<String, Color>,
    section: &'static str,
    key: &'static str,
) -> Option<Color> {
    colors.get(&format!("{section}.{key}")).copied()
}

#[cfg(target_os = "linux")]
fn kde_system_theme(mode: theme::Mode) -> Option<Theme> {
    let base_theme = match mode {
        theme::Mode::Light => Theme::Light,
        _ => Theme::Dark,
    };

    let mut palette = base_theme.palette();
    let colors = read_kdeglobals_colors()?;

    // Use the KDE window background as the app base surface.
    // "View" colors are commonly used for list/table areas and are often darker.
    if let Some(background) = kde_color(&colors, "Colors:Window", "BackgroundNormal")
        .or_else(|| kde_color(&colors, "Colors:View", "BackgroundNormal"))
    {
        palette.background = background;
    }

    if let Some(text) = kde_color(&colors, "Colors:Window", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:View", "ForegroundNormal"))
    {
        palette.text = text;
    }

    if let Some(primary) = kde_color(&colors, "Colors:Selection", "BackgroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Window", "DecorationFocus"))
    {
        palette.primary = primary;
    }

    if let Some(success) = kde_color(&colors, "Colors:Positive", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Positive", "DecorationFocus"))
        .or_else(|| kde_color(&colors, "Colors:Positive", "BackgroundNormal"))
    {
        palette.success = success;
    }

    if let Some(warning) = kde_color(&colors, "Colors:Neutral", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Neutral", "DecorationFocus"))
        .or_else(|| kde_color(&colors, "Colors:Neutral", "BackgroundNormal"))
    {
        palette.warning = warning;
    }

    if let Some(danger) = kde_color(&colors, "Colors:Negative", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Negative", "DecorationFocus"))
        .or_else(|| kde_color(&colors, "Colors:Negative", "BackgroundNormal"))
    {
        palette.danger = danger;
    }

    Some(Theme::custom("KDE System", palette))
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, Default)]
struct KdeSidebarColors {
    window_bg: Option<Color>,
    window_alt_bg: Option<Color>,
    view_bg: Option<Color>,
    view_alt_bg: Option<Color>,
    sidebar_bg: Option<Color>,
    footer_bg: Option<Color>,
    border: Option<Color>,
    nav_text: Option<Color>,
    nav_hover_bg: Option<Color>,
    nav_hover_text: Option<Color>,
    nav_selected_bg: Option<Color>,
    nav_selected_text: Option<Color>,
}

#[cfg(target_os = "linux")]
fn kde_system_sidebar_colors() -> KdeSidebarColors {
    let Some(colors) = read_kdeglobals_colors() else {
        return KdeSidebarColors::default();
    };

    let window_bg = kde_color(&colors, "Colors:Window", "BackgroundNormal")
        .or_else(|| kde_color(&colors, "Colors:View", "BackgroundNormal"));

    let window_alt_bg = kde_color(&colors, "Colors:Window", "BackgroundAlternate")
        .or_else(|| kde_color(&colors, "Colors:Button", "BackgroundNormal"));

    let view_bg = kde_color(&colors, "Colors:View", "BackgroundNormal")
        .or(window_bg)
        .or(window_alt_bg);

    let view_alt_bg = kde_color(&colors, "Colors:View", "BackgroundAlternate").or(window_alt_bg);

    // Prefer alternate surfaces for sidebar so it remains distinct from page content.
    let sidebar_bg = window_alt_bg.or(view_alt_bg).or(view_bg).or(window_bg);

    let footer_bg = view_alt_bg.or(window_alt_bg).or(sidebar_bg);

    let border = window_alt_bg.or(view_alt_bg).or(sidebar_bg);

    let nav_text = kde_color(&colors, "Colors:View", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Window", "ForegroundNormal"));

    let nav_hover_bg = kde_color(&colors, "Colors:Selection", "BackgroundAlternate")
        .or_else(|| kde_color(&colors, "Colors:View", "DecorationHover"))
        .or_else(|| kde_color(&colors, "Colors:Button", "DecorationHover"));

    let nav_hover_text = kde_color(&colors, "Colors:Selection", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:View", "ForegroundNormal"))
        .or_else(|| kde_color(&colors, "Colors:Window", "ForegroundNormal"))
        .or(nav_text);

    let nav_selected_bg = kde_color(&colors, "Colors:Selection", "BackgroundNormal")
        .or_else(|| kde_color(&colors, "Colors:View", "DecorationFocus"))
        .or_else(|| kde_color(&colors, "Colors:Window", "DecorationFocus"));

    let nav_selected_text = kde_color(&colors, "Colors:Selection", "ForegroundNormal")
        .or_else(|| kde_color(&colors, "Colors:Selection", "ForegroundActive"))
        .or(nav_text);

    KdeSidebarColors {
        window_bg,
        window_alt_bg,
        view_bg,
        view_alt_bg,
        sidebar_bg,
        footer_bg,
        border,
        nav_text,
        nav_hover_bg,
        nav_hover_text,
        nav_selected_bg,
        nav_selected_text,
    }
}

pub(in crate::gui) fn text_size(base: u16) -> f32 {
    let scale = f32::from_bits(GUI_TEXT_SCALE_BITS.load(Ordering::Relaxed));
    (base as f32 * scale).round()
}

/// Scale any pixel value (padding, spacing, radius, …) by the current GUI
/// scale factor.  Use this instead of hardcoded literals so the UI stays
/// proportional when the window is resized.
pub(in crate::gui) fn scaled(base: f32) -> f32 {
    let scale = f32::from_bits(GUI_TEXT_SCALE_BITS.load(Ordering::Relaxed));
    (base * scale).round()
}

pub(in crate::gui) fn gui_scale() -> f32 {
    f32::from_bits(GUI_TEXT_SCALE_BITS.load(Ordering::Relaxed))
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
    #[cfg(target_os = "linux")]
    {
        if let Err(err) = ensure_linux_desktop_entry() {
            tracing::warn!("Failed to auto-register linux desktop entry: {err}");
        }
    }

    // Initialize system tray icon
    let tray_handle = match tray::TrayHandle::new("ODD-BOX") {
        Ok(handle) => {
            tracing::trace!("System tray initialized successfully");
            Some(handle)
        }
        Err(e) => {
            tracing::warn!("Failed to initialize system tray: {}", e);
            None
        }
    };

    let initial_window_size = iced::Size::new(WINDOW_INITIAL_WIDTH, WINDOW_INITIAL_HEIGHT);
    set_gui_text_scale(initial_window_size);

    let state_clone = state.clone();
    let log_state_clone = log_state.clone();

    // Wrap tray_handle in Arc<Mutex> so we can move it into the closure
    let tray_handle = std::sync::Arc::new(std::sync::Mutex::new(tray_handle));
    let tray_handle_clone = tray_handle.clone();

    iced::daemon(
        move || {
            let tray = tray_handle_clone.lock().unwrap().take();
            let (mut gui, init_task) = OddBoxGui::new(
                state_clone.clone(),
                theme_mode,
                log_state_clone.clone(),
                tray,
            );
            let (window_id, open_window_task) = window::open(make_window_settings());
            gui.window_id = Some(window_id);
            (
                gui,
                Task::batch(vec![
                    init_task,
                    open_window_task.map(|id| Message::WindowIdResolved(Some(id))),
                ]),
            )
        },
        OddBoxGui::update,
        daemon_view,
    )
    .style(|_state, theme: &Theme| {
        let bg = theme.palette().background;
        let is_light = (0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b) > 0.5;
        let alpha = if use_glass_effects() {
            if is_light { 0.68 } else { 0.80 }
        } else {
            1.0
        };
        theme::Style {
            background_color: platform_surface_color(bg, alpha),
            text_color: theme.palette().text,
        }
    })
    .theme(daemon_theme)
    .subscription(OddBoxGui::subscription)
    .title("ODD-BOX")
    .run()
}

fn daemon_view<'a>(state: &'a OddBoxGui, _window: window::Id) -> Element<'a, Message> {
    state.view()
}

fn daemon_theme(state: &OddBoxGui, _window: window::Id) -> Theme {
    state.theme()
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
    Updates,
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
            Page::Updates => "Updates",
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
            Page::Updates => "↑",
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
    /// Move keyboard focus to the next focusable widget (Tab)
    FocusNext,
    /// Move keyboard focus to the previous focusable widget (Shift+Tab)
    FocusPrevious,
    NavigateTo(Page),
    WindowResized(window::Id, iced::Size),
    SystemThemeChanged(theme::Mode),
    LogViewportChanged(scrollable::Viewport),
    // Log filter messages
    LogFilterTextChanged(String),
    LogLevelPresetChanged(LogLevelPreset),
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
    // Updates page actions
    UpdatesCheck,
    UpdatesCheckResult(Result<String, String>),
    UpdatesRunSelfUpdate,
    UpdatesRunSelfUpdateResult(Result<crate::self_update::UpdateAction, String>),
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
    ProcessToggleAutoStart(String),
    ProcessToggleAutoStartResult(Result<(), String>),
    // Dashboard card menu
    DashboardToggleProcessMenu(String),
    DashboardDismissMenu,
    DashboardCursorMoved(f32, f32),
    ManageProcess(String),
    OpenInBrowser(String),
    OpenEditFrontend(String),
    OpenEditBackend(String),
    OpenNewFrontend,
    OpenNewFrontendForBackend(String),
    OpenNewBackend(BackendKind),
    EditFrontendLoaded(EditFrontendForm),
    EditFrontendHostChanged(String),
    EditFrontendBackendChanged(BackendOption),
    EditFrontendCaptureSubdomainsToggled(bool),
    EditFrontendForwardSubdomainsToggled(bool),
    EditFrontendRedirectHttpsToggled(bool),
    EditFrontendLetsEncryptToggled(bool),
    EditFrontendEnableCrumaToggled(bool),
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
    CrumaAuthIdChanged(String),
    CrumaAuthKeyChanged(String),
    CrumaAuthSave,
    CrumaAuthModeSaveResult(Result<(), String>),
    TrafficInspectionToggled(bool),
    TrafficInspectionClear,
    TrafficInspectionSelect(Option<u64>),
    TrafficInspectionBodyPreviewReady(CachedBodyPreview),
    TrafficInspectionExpandBody(BodySide),
    TrafficInspectionBodyExpandReady(BodySide, Option<String>),
    TrafficInspectionSaveBody(BodySide),
    TrafficInspectionSaveBodyResult(Result<String, String>),
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

/// Which side of the exchange a body action refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodySide {
    Request,
    Response,
}

/// Cached body preview so we avoid re-fetching, decompressing, and
/// converting on every frame while a detail panel is open.
#[derive(Debug, Clone)]
pub struct CachedBodyPreview {
    /// The `req_id` this cache entry belongs to.
    pub req_id: u64,
    /// Precomputed request body preview.
    pub req_body: Option<CachedBody>,
    /// Precomputed response body preview.
    pub resp_body: Option<CachedBody>,
}

/// A single body (request or response) with content-type detection,
/// a short text preview, an optional expanded preview, and an optional
/// decoded image handle for inline rendering.
#[derive(Debug, Clone)]
pub struct CachedBody {
    /// Detected content kind (JSON, image, binary, …).
    pub kind: pages::body_content::BodyContentKind,
    /// Short text preview (≤512 chars) for text bodies, or a hex dump
    /// for binary bodies.
    pub preview: String,
    /// Expanded text preview (≤4096 chars), populated on demand when the
    /// user clicks "Load more".
    pub expanded_preview: Option<String>,
    /// For inline-renderable images: the decoded iced image handle.
    pub image: Option<pages::body_content::DecodedImage>,
    /// Total size of the raw (possibly compressed) body bytes.
    pub raw_size: usize,
    /// Whether the capture store truncated this body.
    pub truncated: bool,
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
    pub enable_cruma: bool,
    pub https_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Process,
    Remote,
    Static,
    Unknown,
}

impl BackendKind {
    /// The selectable backend kinds (excludes Unknown).
    pub const ALL: [BackendKind; 3] = [
        BackendKind::Process,
        BackendKind::Remote,
        BackendKind::Static,
    ];
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Process => write!(f, "Process"),
            BackendKind::Remote => write!(f, "Remote"),
            BackendKind::Static => write!(f, "Static"),
            BackendKind::Unknown => write!(f, "Unknown"),
        }
    }
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
    pub spa_fallback: bool,
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
    Kind(BackendKind),
    Endpoints(String),
    Protocol(v4::Protocol),
    Https(bool),
    KeepOriginalHostHeader(bool),
    Dir(String),
    ListDir(bool),
    RenderMarkdown(bool),
    CacheMaxAge(String),
    SpaFallback(bool),
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
    pub(in crate::gui) window_width: f32,
    theme_mode: ThemeMode,
    system_theme: Option<theme::Mode>,
    #[cfg(target_os = "linux")]
    system_kde_theme: Option<Theme>,
    #[cfg(target_os = "linux")]
    system_kde_sidebar_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_sidebar_footer_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_sidebar_border: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_window_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_window_alt_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_view_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_view_alt_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_nav_text: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_nav_hover_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_nav_hover_text: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_nav_selected_bg: Option<Color>,
    #[cfg(target_os = "linux")]
    system_kde_nav_selected_text: Option<Color>,
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
    // Dashboard: cooldown timestamps for Start All / Stop All feedback
    pub(in crate::gui) dashboard_startall_cooldown: Option<std::time::Instant>,
    pub(in crate::gui) dashboard_stopall_cooldown: Option<std::time::Instant>,
    pub(in crate::gui) edit_target: Option<String>,
    pub(in crate::gui) edit_frontend_form: EditFrontendForm,
    pub(in crate::gui) edit_frontend_notice: Option<String>,
    pub(in crate::gui) edit_frontend_pending_reload: bool,
    pub(in crate::gui) edit_frontend_original: Option<String>,
    pub(in crate::gui) edit_frontend_is_new: bool,
    pub(in crate::gui) edit_frontend_confirm_delete: bool,
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
    pub(in crate::gui) cruma_auth_id: String,
    pub(in crate::gui) cruma_auth_key: String,
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
    pub(in crate::gui) traffic_cached_body_preview: Option<CachedBodyPreview>,
    pub(in crate::gui) traffic_body_expanded: bool,
    frontend_http_port_input: String,
    frontend_https_port_input: String,
    frontend_port_notice: Option<String>,
    frontend_ports_dirty: bool,
    pub(in crate::gui) update_current_version: String,
    pub(in crate::gui) update_install_source: String,
    pub(in crate::gui) update_hint: String,
    pub(in crate::gui) update_install_path: Option<String>,
    pub(in crate::gui) update_is_package_managed: bool,
    pub(in crate::gui) update_latest_tag: Option<String>,
    pub(in crate::gui) update_check_in_progress: bool,
    pub(in crate::gui) update_check_error: Option<String>,
    pub(in crate::gui) update_action_in_progress: bool,
    pub(in crate::gui) update_notice: Option<String>,
    pub(in crate::gui) update_notice_is_error: bool,
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

fn cruma_auth_from_config(cfg: &crate::configuration::ConfigWrapper) -> (String, String) {
    match cfg.cruma.as_ref().and_then(|c| c.mode()) {
        Some(crate::configuration::v4::CrumaMode::Authenticated { id, key }) => (id, key),
        _ => (String::new(), String::new()),
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
                form.enable_cruma = d.enable_cruma;
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
        || form.enable_cruma
    {
        v4::RouteTarget::Detailed(v4::DetailedRoute {
            backend: form.backend.clone(),
            capture_subdomains: form.capture_subdomains,
            forward_subdomains: form.forward_subdomains,
            redirect_to_https: form.redirect_to_https,
            lets_encrypt: form.lets_encrypt,
            enable_cruma: form.enable_cruma,
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
    crate::cruma_integration::rebuild_cruma_config(state.clone());

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

    let env: std::collections::HashMap<String, String> =
        vars.into_iter().filter(|(k, _)| !k.is_empty()).collect();
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

fn normalize_release_tag(tag: &str) -> &str {
    tag.trim_start_matches(|c| c == 'v' || c == 'V')
}

fn should_include_prerelease_for_checks(current_version: &str) -> bool {
    current_version.contains('-')
}

pub(in crate::gui) fn compare_release_versions(
    current_version: &str,
    latest_tag: &str,
) -> Option<std::cmp::Ordering> {
    let latest = normalize_release_tag(latest_tag);
    let latest_is_newer = self_update::version::bump_is_greater(current_version, latest).ok()?;
    if latest_is_newer {
        return Some(std::cmp::Ordering::Greater);
    }
    let current_is_newer = self_update::version::bump_is_greater(latest, current_version).ok()?;
    if current_is_newer {
        Some(std::cmp::Ordering::Less)
    } else {
        Some(std::cmp::Ordering::Equal)
    }
}

async fn check_latest_release(
    tokio_handle: tokio::runtime::Handle,
    include_pre: bool,
) -> Result<String, String> {
    tokio_handle
        .spawn(async move { crate::self_update::find_latest_version(include_pre).await })
        .await
        .map_err(|err| format!("Update check task failed: {err}"))?
        .map_err(|err| err.to_string())
}

async fn run_self_update(
    tokio_handle: tokio::runtime::Handle,
) -> Result<crate::self_update::UpdateAction, String> {
    tokio_handle
        .spawn(async { crate::self_update::update().await })
        .await
        .map_err(|err| format!("Self-update task failed: {err}"))?
        .map_err(|err| err.to_string())
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
            tokio::time::timeout(std::time::Duration::from_secs(10), token.cancelled()).await
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

async fn save_cruma_mode(
    state: Arc<GlobalState>,
    mode: CrumaAuthMode,
    auth_id: String,
    auth_key: String,
) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();

    match mode {
        CrumaAuthMode::Disabled => {
            guard.cruma = None;
        }
        CrumaAuthMode::Anonymous => {
            guard.cruma = Some(v4::CrumaConfig::Mode("anon".to_string()));
        }
        CrumaAuthMode::Authenticated => {
            let id = auth_id.trim().to_string();
            if id.is_empty() {
                return Err("Tunnel ID is required for authenticated mode.".to_string());
            }
            if auth_key.trim().is_empty() {
                return Err("Tunnel key is required for authenticated mode.".to_string());
            }
            let key = auth_key;
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
    crate::cruma_integration::rebuild_cruma_config(state.clone());
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
                let mut env_vec: Vec<(String, String)> =
                    p.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
                form.spa_fallback = s.spa_fallback;
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
                    spa_fallback: form.spa_fallback,
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
    crate::cruma_integration::rebuild_cruma_config(state.clone());
    Ok(())
}

async fn delete_backend(state: Arc<GlobalState>, backend_id: String) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();
    guard.backends.remove(&backend_id);

    guard.reload_dashmaps();
    guard.is_valid().map_err(|e| e.to_string())?;
    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    crate::cruma_integration::rebuild_cruma_config(state.clone());
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
        spa_fallback: false,
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

/// Toggle the auto_start flag for a process backend and save to disk.
async fn toggle_process_auto_start(
    state: Arc<GlobalState>,
    backend_name: String,
) -> Result<(), String> {
    let mut guard = (*state.config.load_full()).clone();

    match guard.backends.get_mut(&backend_name) {
        Some(v4::Backend::Process(proc)) => {
            let current = proc.auto_start.unwrap_or(true);
            proc.auto_start = Some(!current);
        }
        _ => {
            return Err(format!(
                "Backend '{}' is not a process backend.",
                backend_name
            ));
        }
    }

    guard.write_to_disk().map_err(|e| e.to_string())?;
    state.config.store(std::sync::Arc::new(guard));
    Ok(())
}

/// Open a URL in the default system browser.
fn open_url_in_browser(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn();
    }
}

/// Build an `EnvFilter` for the given log level, matching the logic used
/// during startup and in the TUI / config-reload paths.
fn build_gui_env_filter(log_level: &LogLevel) -> tracing_subscriber::EnvFilter {
    use tracing_subscriber::EnvFilter;

    let level_str = match log_level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    };

    let rust_log = std::env::var("RUST_LOG").ok();
    let has_odd_box_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box")))
        .unwrap_or(false);
    let has_cruma_override = rust_log
        .as_ref()
        .map(|v| v.split(',').any(|d| d.trim().starts_with("odd_box::cruma")))
        .unwrap_or(false);

    let mut filter = EnvFilter::from_default_env();
    if !has_odd_box_override {
        filter = filter.add_directive(
            format!("odd_box={}", level_str)
                .parse()
                .expect("This directive should always work"),
        );
    }
    filter = filter.add_directive(
        "odd_box::proc_host=trace"
            .parse()
            .expect("This directive should always work"),
    );
    if !has_odd_box_override && !has_cruma_override {
        filter = filter.add_directive(
            "odd_box::cruma=info"
                .parse()
                .expect("This directive should always work"),
        );
    }
    filter
}

/// Reload the tracing subscriber filter to match the chosen log level.
async fn apply_gui_log_level(state: &GlobalState, level: LogLevel) {
    let filter = build_gui_env_filter(&level);
    match &state.log_handle {
        crate::OddLogHandle::CLI(rw_lock) => match rw_lock.write().await.reload(filter) {
            Ok(_) => {
                tracing::info!("Tracing log level changed to {:?} via GUI", level);
            }
            Err(e) => {
                tracing::error!("Failed to change tracing log level: {e:?}");
            }
        },
        crate::OddLogHandle::TUI(rw_lock) => match rw_lock.write().await.reload(filter) {
            Ok(_) => {
                tracing::info!("Tracing log level changed to {:?} via GUI", level);
            }
            Err(e) => {
                tracing::error!("Failed to change tracing log level: {e:?}");
            }
        },
        crate::OddLogHandle::None => {
            tracing::error!("No log handle exists, cannot change tracing log level");
        }
    }
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

        #[cfg(target_os = "linux")]
        let initial_kde_theme = if matches!(theme_mode, ThemeMode::System) {
            kde_system_theme(theme::Mode::Dark)
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let initial_kde_sidebar_colors = if matches!(theme_mode, ThemeMode::System) {
            kde_system_sidebar_colors()
        } else {
            KdeSidebarColors::default()
        };

        let initial_cfg = state.config.load_full();
        let initial_cruma_mode = cruma_mode_from_config(&initial_cfg);
        let (initial_cruma_auth_id, initial_cruma_auth_key) = cruma_auth_from_config(&initial_cfg);
        let install_source_info = crate::self_update::install_source_info();
        let current_version = crate::self_update::current_version().to_string();
        let include_pre = should_include_prerelease_for_checks(&current_version);
        tasks.push(Task::perform(
            check_latest_release(state.tokio_handle.clone(), include_pre),
            Message::UpdatesCheckResult,
        ));

        (
            Self {
                state,
                log_state,
                current_page: Page::Dashboard,
                window_width: WINDOW_INITIAL_WIDTH,
                theme_mode,
                system_theme: None,
                #[cfg(target_os = "linux")]
                system_kde_theme: initial_kde_theme,
                #[cfg(target_os = "linux")]
                system_kde_sidebar_bg: initial_kde_sidebar_colors.sidebar_bg,
                #[cfg(target_os = "linux")]
                system_kde_sidebar_footer_bg: initial_kde_sidebar_colors.footer_bg,
                #[cfg(target_os = "linux")]
                system_kde_sidebar_border: initial_kde_sidebar_colors.border,
                #[cfg(target_os = "linux")]
                system_kde_window_bg: initial_kde_sidebar_colors.window_bg,
                #[cfg(target_os = "linux")]
                system_kde_window_alt_bg: initial_kde_sidebar_colors.window_alt_bg,
                #[cfg(target_os = "linux")]
                system_kde_view_bg: initial_kde_sidebar_colors.view_bg,
                #[cfg(target_os = "linux")]
                system_kde_view_alt_bg: initial_kde_sidebar_colors.view_alt_bg,
                #[cfg(target_os = "linux")]
                system_kde_nav_text: initial_kde_sidebar_colors.nav_text,
                #[cfg(target_os = "linux")]
                system_kde_nav_hover_bg: initial_kde_sidebar_colors.nav_hover_bg,
                #[cfg(target_os = "linux")]
                system_kde_nav_hover_text: initial_kde_sidebar_colors.nav_hover_text,
                #[cfg(target_os = "linux")]
                system_kde_nav_selected_bg: initial_kde_sidebar_colors.nav_selected_bg,
                #[cfg(target_os = "linux")]
                system_kde_nav_selected_text: initial_kde_sidebar_colors.nav_selected_text,
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
                dashboard_startall_cooldown: None,
                dashboard_stopall_cooldown: None,
                edit_target: None,
                edit_frontend_form: EditFrontendForm::default(),
                edit_frontend_notice: None,
                edit_frontend_pending_reload: false,
                edit_frontend_original: None,
                edit_frontend_is_new: false,
                edit_frontend_confirm_delete: false,
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
                cruma_auth_id: initial_cruma_auth_id,
                cruma_auth_key: initial_cruma_auth_key,
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
                traffic_cached_body_preview: None,
                traffic_body_expanded: false,
                frontend_http_port_input: String::new(),
                frontend_https_port_input: String::new(),
                frontend_port_notice: None,
                frontend_ports_dirty: false,
                update_current_version: current_version,
                update_install_source: install_source_info.source.to_string(),
                update_hint: install_source_info.update_hint.to_string(),
                update_install_path: install_source_info.resolved_path,
                update_is_package_managed: install_source_info.package_managed,
                update_latest_tag: None,
                update_check_in_progress: true,
                update_check_error: None,
                update_action_in_progress: false,
                update_notice: None,
                update_notice_is_error: false,
            },
            Task::batch(tasks),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        // Tab / Shift+Tab focus navigation (handled regardless of widget capture status)
        let tab_sub = event::listen_with(|ev, _status, _window| {
            if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Tab),
                modifiers,
                ..
            }) = &ev
            {
                Some(if modifiers.shift() {
                    Message::FocusPrevious
                } else {
                    Message::FocusNext
                })
            } else {
                None
            }
        });

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
        // Listen for window close requests (to hide instead of quit when tray is active)
        let close_sub = window::close_requests().map(Message::WindowCloseRequested);
        // Listen for window events (focus/unfocus to detect minimize)
        let window_events_sub = window::events().map(|(id, event)| match event {
            iced::window::Event::Focused => Message::WindowFocused(id),
            iced::window::Event::Unfocused => Message::WindowUnfocused(id),
            _ => Message::NoOp,
        });
        // Poll for tray commands
        let tray_sub = time::every(std::time::Duration::from_millis(100))
            .map(|_| Message::TrayCommandReceived);

        Subscription::batch(vec![
            tab_sub,
            page_sub,
            theme_sub,
            resize_sub,
            exit_sub,
            close_sub,
            window_events_sub,
            tray_sub,
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NoOp => {}
            Message::FocusNext => {
                return iced::widget::operation::focus_next();
            }
            Message::FocusPrevious => {
                return iced::widget::operation::focus_previous();
            }
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
                let clamped_size = iced::Size::new(
                    size.width.max(WINDOW_MIN_WIDTH),
                    size.height.max(WINDOW_MIN_HEIGHT),
                );
                self.window_width = clamped_size.width;
                if (clamped_size.width - size.width).abs() > 0.5
                    || (clamped_size.height - size.height).abs() > 0.5
                {
                    set_gui_text_scale(clamped_size);
                    return window::resize(id, clamped_size);
                }
                set_gui_text_scale(size);
            }
            Message::SystemThemeChanged(mode) => {
                self.system_theme = Some(mode);
                #[cfg(target_os = "linux")]
                if matches!(self.theme_mode, ThemeMode::System) {
                    self.system_kde_theme = kde_system_theme(mode);
                    let sidebar_colors = kde_system_sidebar_colors();
                    self.system_kde_sidebar_bg = sidebar_colors.sidebar_bg;
                    self.system_kde_sidebar_footer_bg = sidebar_colors.footer_bg;
                    self.system_kde_sidebar_border = sidebar_colors.border;
                    self.system_kde_window_bg = sidebar_colors.window_bg;
                    self.system_kde_window_alt_bg = sidebar_colors.window_alt_bg;
                    self.system_kde_view_bg = sidebar_colors.view_bg;
                    self.system_kde_view_alt_bg = sidebar_colors.view_alt_bg;
                    self.system_kde_nav_text = sidebar_colors.nav_text;
                    self.system_kde_nav_hover_bg = sidebar_colors.nav_hover_bg;
                    self.system_kde_nav_hover_text = sidebar_colors.nav_hover_text;
                    self.system_kde_nav_selected_bg = sidebar_colors.nav_selected_bg;
                    self.system_kde_nav_selected_text = sidebar_colors.nav_selected_text;
                }
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
                    return window::close(id).chain(iced::exit());
                }
                return iced::exit();
            }
            Message::WindowCloseRequested(id) => {
                // Keep app alive when tray is available.
                #[cfg(target_os = "linux")]
                if self.tray_handle.is_some() {
                    self.window_visible = false;
                    self.window_id = None;
                    if let Some(ref tray) = self.tray_handle {
                        tray.set_window_visible(false);
                    }
                    return window::close(id);
                }

                #[cfg(target_os = "macos")]
                if self.tray_handle.is_some() {
                    self.window_visible = false;
                    if let Some(ref tray) = self.tray_handle {
                        tray.set_window_visible(false);
                    }
                    // Remove from Dock when hidden (macOS only)
                    macos_app_icon::set_activation_policy(
                        macos_app_icon::ActivationPolicy::Accessory,
                    );
                    return window::set_mode(id, window::Mode::Hidden);
                }

                // On other platforms or if no tray on macOS, close normally
                #[cfg(not(target_os = "macos"))]
                {
                    self.state
                        .exit
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    return window::close(id);
                }

                // macOS without tray - also close normally
                #[cfg(target_os = "macos")]
                {
                    self.state
                        .exit
                        .store(true, std::sync::atomic::Ordering::SeqCst);
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
                                    macos_app_icon::set_activation_policy(
                                        macos_app_icon::ActivationPolicy::Regular,
                                    );
                                    macos_app_icon::apply_default_icon();
                                }
                                if let Some(id) = self.window_id {
                                    return Task::batch(vec![
                                        window::minimize(id, false), // Unminimize if minimized
                                        window::set_mode(id, window::Mode::Windowed),
                                        window::gain_focus(id),
                                    ]);
                                } else {
                                    let (id, open_task) = window::open(make_window_settings());
                                    self.window_id = Some(id);
                                    return open_task.map(|id| Message::WindowIdResolved(Some(id)));
                                }
                            }
                            tray::TrayCommand::Hide => {
                                self.window_visible = false;
                                tray.set_window_visible(false);
                                #[cfg(target_os = "linux")]
                                if let Some(id) = self.window_id.take() {
                                    return window::close(id);
                                }
                                // Remove from Dock when hidden
                                #[cfg(target_os = "macos")]
                                macos_app_icon::set_activation_policy(
                                    macos_app_icon::ActivationPolicy::Accessory,
                                );
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
                                macos_app_icon::set_activation_policy(
                                    macos_app_icon::ActivationPolicy::Accessory,
                                );

                                // Mark that we're in quit pending state
                                self.tray_quit_pending = true;

                                // Hide window immediately, then trigger phase 2 after a small delay
                                // to ensure the window system has time to actually hide the window
                                if let Some(id) = self.window_id {
                                    return window::set_mode(id, window::Mode::Hidden).chain(
                                        Task::perform(
                                            async {
                                                // Small delay to let the window hide visually
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(50),
                                                );
                                            },
                                            |_| Message::TrayQuitPhase2,
                                        ),
                                    );
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
                    self.state
                        .exit
                        .store(true, std::sync::atomic::Ordering::SeqCst);

                    // Exit the GUI - this allows gui::run() to return and main.rs
                    // will handle the graceful shutdown of all processes
                    if let Some(id) = self.window_id {
                        return Task::batch(vec![window::close(id), iced::exit()]);
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
                let cfg = self.state.config.load_full();
                self.cruma_auth_mode = cruma_mode_from_config(&cfg);
                let (id, key) = cruma_auth_from_config(&cfg);
                self.cruma_auth_id = id;
                self.cruma_auth_key = key;
            }
            Message::UpdatesCheck => {
                if self.update_check_in_progress || self.update_action_in_progress {
                    return Task::none();
                }
                self.update_check_in_progress = true;
                self.update_check_error = None;
                let tokio_handle = self.state.tokio_handle.clone();
                let include_pre =
                    should_include_prerelease_for_checks(&self.update_current_version);
                return Task::perform(
                    check_latest_release(tokio_handle, include_pre),
                    Message::UpdatesCheckResult,
                );
            }
            Message::UpdatesCheckResult(result) => {
                self.update_check_in_progress = false;
                match result {
                    Ok(tag) => {
                        self.update_latest_tag = Some(tag);
                        self.update_check_error = None;
                        self.update_notice = None;
                        self.update_notice_is_error = false;
                    }
                    Err(err) => {
                        self.update_check_error = Some(err.clone());
                        self.update_notice = Some(format!("Failed to check for updates: {err}"));
                        self.update_notice_is_error = true;
                    }
                }
            }
            Message::UpdatesRunSelfUpdate => {
                if self.update_action_in_progress || self.update_check_in_progress {
                    return Task::none();
                }
                self.update_action_in_progress = true;
                self.update_notice = None;
                self.update_notice_is_error = false;
                let tokio_handle = self.state.tokio_handle.clone();
                return Task::perform(
                    run_self_update(tokio_handle),
                    Message::UpdatesRunSelfUpdateResult,
                );
            }
            Message::UpdatesRunSelfUpdateResult(result) => {
                self.update_action_in_progress = false;
                match result {
                    Ok(crate::self_update::UpdateAction::Updated) => {
                        self.update_notice = Some(
                            "Self-update completed. Restart odd-box to run the updated binary."
                                .to_string(),
                        );
                        self.update_notice_is_error = false;
                    }
                    Ok(crate::self_update::UpdateAction::NoUpdateNeeded) => {
                        self.update_notice = Some(
                            "No update needed. You are already on this version (or a newer pre-release build)."
                                .to_string(),
                        );
                        self.update_notice_is_error = false;
                    }
                    Err(err) => {
                        self.update_notice = Some(err);
                        self.update_notice_is_error = true;
                    }
                }
                self.update_check_error = None;
                self.update_check_in_progress = true;
                let tokio_handle = self.state.tokio_handle.clone();
                let include_pre =
                    should_include_prerelease_for_checks(&self.update_current_version);
                return Task::perform(
                    check_latest_release(tokio_handle, include_pre),
                    Message::UpdatesCheckResult,
                );
            }
            Message::LogFilterTextChanged(text) => {
                self.log_filter.text = text;
                self.log_state.set_filter(self.log_filter.clone());
            }
            Message::LogLevelPresetChanged(preset) => {
                self.log_level_preset = preset;
                Self::apply_log_level_preset(&mut self.log_filter, preset);
                self.log_state.set_filter(self.log_filter.clone());

                // Also update the actual tracing subscriber filter so the app
                // starts/stops producing log messages at the selected level.
                let target_level = match preset {
                    LogLevelPreset::All => LogLevel::Trace,
                    LogLevelPreset::DebugAndAbove => LogLevel::Debug,
                    LogLevelPreset::InfoAndAbove => LogLevel::Info,
                    LogLevelPreset::WarnAndAbove => LogLevel::Warn,
                    LogLevelPreset::ErrorOnly => LogLevel::Error,
                };
                let state = self.state.clone();
                return Task::perform(
                    async move { apply_gui_log_level(&state, target_level).await },
                    |_| Message::NoOp,
                );
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
                self.dashboard_process_menu = None;
                self.state.process_registry.set_enabled(&name, true);
            }
            Message::ProcessStop(name) => {
                self.dashboard_process_menu = None;
                self.state.process_registry.set_enabled(&name, false);
            }
            Message::ProcessStartAll => {
                self.dashboard_startall_cooldown = Some(std::time::Instant::now());
                for proc in &self.cached_config.processes {
                    self.state.process_registry.set_enabled(&proc.name, true);
                }
            }
            Message::ProcessStopAll => {
                self.dashboard_stopall_cooldown = Some(std::time::Instant::now());
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
            Message::ProcessToggleAutoStart(name) => {
                let state = self.state.clone();
                return Task::perform(
                    toggle_process_auto_start(state, name),
                    Message::ProcessToggleAutoStartResult,
                );
            }
            Message::ProcessToggleAutoStartResult(result) => match result {
                Ok(_) => {
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
                Err(e) => {
                    tracing::error!("Failed to toggle auto-start: {e}");
                }
            },
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
            Message::GlobalEnvSaveResult(result) => match result {
                Ok(()) => {
                    self.global_env_notice =
                        Some("Global environment variables saved.".to_string());
                    self.global_env_dirty = false;
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
                Err(e) => {
                    self.global_env_notice = Some(format!("Error: {e}"));
                }
            },
            Message::DashboardToggleProcessMenu(name) => {
                if self.dashboard_process_menu.as_ref() == Some(&name) {
                    self.dashboard_process_menu = None;
                } else {
                    self.dashboard_process_menu = Some(name);
                    self.dashboard_menu_pos = self.dashboard_cursor_pos;
                }
            }
            Message::DashboardDismissMenu => {
                self.dashboard_process_menu = None;
            }
            Message::DashboardCursorMoved(x, y) => {
                self.dashboard_cursor_pos = (x, y);
            }
            Message::OpenInBrowser(url) => {
                self.dashboard_process_menu = None;
                open_url_in_browser(&url);
            }
            Message::ManageProcess(name) => {
                self.dashboard_process_menu = None;
                self.expanded_process = Some(name);
                self.current_page = Page::ManagedProcesses;
                return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
            }
            Message::OpenEditFrontend(name) => {
                self.edit_target = Some(name.clone());
                self.current_page = Page::EditFrontend;
                self.dashboard_process_menu = None;
                self.edit_frontend_notice = None;
                self.edit_frontend_original = Some(name.clone());
                self.edit_frontend_is_new = false;
                self.edit_frontend_confirm_delete = false;
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
                self.edit_frontend_confirm_delete = false;
            }
            Message::OpenNewFrontendForBackend(backend_name) => {
                self.edit_target = None;
                self.current_page = Page::EditFrontend;
                self.dashboard_process_menu = None;
                self.edit_frontend_notice = None;
                self.edit_frontend_form = EditFrontendForm {
                    backend: backend_name,
                    ..EditFrontendForm::default()
                };
                self.edit_frontend_original = None;
                self.edit_frontend_is_new = true;
                self.edit_frontend_confirm_delete = false;
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
            Message::EditFrontendEnableCrumaToggled(value) => {
                self.edit_frontend_form.enable_cruma = value;
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
                    self.edit_frontend_confirm_delete = false;
                }
                Err(err) => {
                    self.edit_frontend_notice = Some(err);
                }
            },
            Message::EditFrontendDelete => {
                if let Some(host) = self.edit_frontend_original.clone() {
                    if !self.edit_frontend_confirm_delete {
                        self.edit_frontend_notice =
                            Some("Are you sure? Click Delete again to confirm.".to_string());
                        self.edit_frontend_confirm_delete = true;
                        return Task::none();
                    }
                    self.edit_frontend_notice = None;
                    self.edit_frontend_confirm_delete = false;
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
                EditBackendField::Kind(v) => self.edit_backend_form.kind = v,
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
                EditBackendField::SpaFallback(v) => self.edit_backend_form.spa_fallback = v,
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
                if mode == CrumaAuthMode::Authenticated {
                    self.cruma_mode_notice = Some(
                        "Enter tunnel ID and key, then click Save Auth Credentials.".to_string(),
                    );
                    return Task::none();
                }
                return Task::perform(
                    save_cruma_mode(
                        self.state.clone(),
                        mode,
                        self.cruma_auth_id.clone(),
                        self.cruma_auth_key.clone(),
                    ),
                    Message::CrumaAuthModeSaveResult,
                );
            }
            Message::CrumaAuthIdChanged(value) => {
                self.cruma_auth_id = value;
                self.cruma_mode_notice = None;
            }
            Message::CrumaAuthKeyChanged(value) => {
                self.cruma_auth_key = value;
                self.cruma_mode_notice = None;
            }
            Message::CrumaAuthSave => {
                self.cruma_mode_notice = None;
                return Task::perform(
                    save_cruma_mode(
                        self.state.clone(),
                        self.cruma_auth_mode,
                        self.cruma_auth_id.clone(),
                        self.cruma_auth_key.clone(),
                    ),
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
                self.traffic_cached_body_preview = None;
                self.traffic_body_expanded = false;
            }
            Message::TrafficInspectionSelect(req_id) => {
                self.traffic_inspection_selected = req_id;
                self.traffic_cached_body_preview = None;
                self.traffic_body_expanded = false;
                if let Some(id) = req_id {
                    let store = self.state.http_capture_store.clone();
                    let snap = store.snapshot();
                    if let Some(entry) = snap.entries.get(&id).cloned() {
                        let store2 = store.clone();
                        return Task::perform(
                            blocking::unblock(move || {
                                pages::traffic_inspection::compute_body_previews_for_cache(
                                    id, &entry, &store2,
                                )
                            }),
                            Message::TrafficInspectionBodyPreviewReady,
                        );
                    }
                }
            }
            Message::TrafficInspectionBodyPreviewReady(cached) => {
                if self.traffic_inspection_selected == Some(cached.req_id) {
                    self.traffic_cached_body_preview = Some(cached);
                }
            }
            Message::TrafficInspectionExpandBody(side) => {
                self.traffic_body_expanded = true;
                if let Some(id) = self.traffic_inspection_selected {
                    let store = self.state.http_capture_store.clone();
                    let snap = store.snapshot();
                    if let Some(entry) = snap.entries.get(&id).cloned() {
                        let side_copy = side;
                        return Task::perform(
                            blocking::unblock(move || {
                                pages::traffic_inspection::compute_expanded_preview(
                                    id, &entry, &store, side_copy,
                                )
                            }),
                            move |expanded| {
                                Message::TrafficInspectionBodyExpandReady(side, expanded)
                            },
                        );
                    }
                }
            }
            Message::TrafficInspectionBodyExpandReady(side, expanded_text) => {
                if let Some(ref mut cached) = self.traffic_cached_body_preview {
                    let body = match side {
                        BodySide::Request => cached.req_body.as_mut(),
                        BodySide::Response => cached.resp_body.as_mut(),
                    };
                    if let Some(body) = body {
                        body.expanded_preview = expanded_text;
                    }
                }
            }
            Message::TrafficInspectionSaveBody(side) => {
                if let Some(id) = self.traffic_inspection_selected {
                    let store = self.state.http_capture_store.clone();
                    let snap = store.snapshot();
                    let entry = snap.entries.get(&id).cloned();

                    let ext = self
                        .traffic_cached_body_preview
                        .as_ref()
                        .and_then(|c| match side {
                            BodySide::Request => c.req_body.as_ref(),
                            BodySide::Response => c.resp_body.as_ref(),
                        })
                        .map(|b| pages::body_content::suggest_extension(b.kind))
                        .unwrap_or("bin");
                    let default_name = format!("body.{ext}");

                    let (filter_label, filter_exts) = self
                        .traffic_cached_body_preview
                        .as_ref()
                        .and_then(|c| match side {
                            BodySide::Request => c.req_body.as_ref(),
                            BodySide::Response => c.resp_body.as_ref(),
                        })
                        .map(|b| pages::body_content::suggest_save_filter(b.kind))
                        .unwrap_or(("All files", &["bin"]));

                    return Task::perform(
                        async move {
                            let path = rfd::FileDialog::new()
                                .set_title("Save body to file")
                                .set_file_name(&default_name)
                                .add_filter(filter_label, filter_exts)
                                .save_file();

                            let path = match path {
                                Some(p) => p,
                                None => return Err("Cancelled".to_string()),
                            };

                            let captured = store
                                .body_bytes(id)
                                .ok_or_else(|| "Body data no longer available".to_string())?;

                            let bytes = match side {
                                BodySide::Request => captured
                                    .req_body
                                    .ok_or_else(|| "No request body".to_string())?,
                                BodySide::Response => captured
                                    .resp_body
                                    .ok_or_else(|| "No response body".to_string())?,
                            };

                            let content_encoding = entry.as_ref().and_then(|e| {
                                let hdrs = match side {
                                    BodySide::Request => e.req_headers.as_ref(),
                                    BodySide::Response => e.resp_headers.as_ref(),
                                };
                                hdrs.and_then(|h| {
                                    h.iter()
                                        .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))
                                        .map(|(_, v)| v.clone())
                                })
                            });

                            let final_bytes = pages::traffic_inspection::try_decompress_for_save(
                                &bytes,
                                content_encoding.as_deref(),
                            );

                            std::fs::write(&path, &final_bytes).map_err(|e| e.to_string())?;
                            Ok(path.display().to_string())
                        },
                        Message::TrafficInspectionSaveBodyResult,
                    );
                }
            }
            Message::TrafficInspectionSaveBodyResult(result) => {
                match result {
                    Ok(_path) => {
                        // Could show a notification here in the future
                    }
                    Err(msg) => {
                        if msg != "Cancelled" {
                            tracing::warn!("Save failed: {msg}");
                        }
                    }
                }
            }
        }
        Task::none()
    }

    fn use_kde_system_styles(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            matches!(self.theme_mode, ThemeMode::System) && self.system_kde_theme.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub(in crate::gui) fn surface_page_bg(&self, theme: &Theme) -> Color {
        if self.use_kde_system_styles() {
            #[cfg(target_os = "linux")]
            if let Some(bg) = self.system_kde_window_bg.or(self.system_kde_view_bg) {
                return bg;
            }
        }

        let palette = theme.extended_palette();
        if palette.is_dark {
            palette.background.base.color
        } else {
            palette.background.weak.color
        }
    }

    pub(in crate::gui) fn surface_panel_bg(&self, theme: &Theme) -> Color {
        if self.use_kde_system_styles() {
            #[cfg(target_os = "linux")]
            if let Some(bg) = self
                .system_kde_view_bg
                .or(self.system_kde_window_alt_bg)
                .or(self.system_kde_window_bg)
            {
                return bg;
            }
        }

        let palette = theme.extended_palette();
        if palette.is_dark {
            theme::palette::mix(self.surface_page_bg(theme), Color::BLACK, 0.12)
        } else {
            palette.background.weaker.color
        }
    }

    pub(in crate::gui) fn surface_panel_alt_bg(&self, theme: &Theme) -> Color {
        if self.use_kde_system_styles() {
            #[cfg(target_os = "linux")]
            if let Some(bg) = self
                .system_kde_view_alt_bg
                .or(self.system_kde_window_alt_bg)
                .or(self.system_kde_view_bg)
            {
                return bg;
            }
        }

        let palette = theme.extended_palette();
        if palette.is_dark {
            theme::palette::mix(self.surface_page_bg(theme), Color::BLACK, 0.18)
        } else {
            palette.background.weak.color
        }
    }

    pub(in crate::gui) fn surface_border_color(&self, theme: &Theme) -> Color {
        if self.use_kde_system_styles() {
            #[cfg(target_os = "linux")]
            if let Some(color) = self
                .system_kde_window_alt_bg
                .or(self.system_kde_view_alt_bg)
                .or(self.system_kde_view_bg)
            {
                return color;
            }
        }

        let palette = theme.extended_palette();
        if palette.is_dark {
            theme::palette::mix(self.surface_page_bg(theme), Color::BLACK, 0.28)
        } else {
            palette.background.strong.color
        }
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
            container::Style {
                background: Some(Background::Color(if use_glass_effects() {
                    Color::TRANSPARENT
                } else {
                    platform_surface_color(bg, 1.0)
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
            Page::Updates,
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
        let nav_scroll = Scrollable::new(nav)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme, status| scrollable::Style {
                container: container::Style {
                    background: None,
                    ..Default::default()
                },
                ..scrollable::default(theme, status)
            });
        #[cfg(target_os = "linux")]
        let (kde_sidebar_bg, kde_sidebar_footer_bg, kde_sidebar_border) =
            if matches!(self.theme_mode, ThemeMode::System) {
                (
                    self.system_kde_sidebar_bg,
                    self.system_kde_sidebar_footer_bg,
                    self.system_kde_sidebar_border,
                )
            } else {
                (None, None, None)
            };
        #[cfg(not(target_os = "linux"))]
        let (kde_sidebar_bg, kde_sidebar_footer_bg, kde_sidebar_border): (
            Option<Color>,
            Option<Color>,
            Option<Color>,
        ) = (None, None, None);

        let current_tag = format!("v{}", self.update_current_version);
        let update_status = if self.update_action_in_progress {
            "Updating...".to_string()
        } else if self.update_check_in_progress {
            "Checking updates...".to_string()
        } else if let Some(err) = &self.update_check_error {
            format!("Check failed: {err}")
        } else if let Some(latest) = &self.update_latest_tag {
            match compare_release_versions(&self.update_current_version, latest) {
                Some(std::cmp::Ordering::Greater) => format!("Update available: {latest}"),
                Some(std::cmp::Ordering::Equal) => "Up to date".to_string(),
                Some(std::cmp::Ordering::Less) => {
                    format!("Current build is newer than {latest}")
                }
                None => format!("Latest release: {latest}"),
            }
        } else {
            "Update status unavailable".to_string()
        };

        let has_update_available = self
            .update_latest_tag
            .as_ref()
            .and_then(|latest| compare_release_versions(&self.update_current_version, latest))
            == Some(std::cmp::Ordering::Greater);

        let sidebar_status_color = if self.update_check_error.is_some() {
            self.theme().extended_palette().danger.strong.color
        } else if has_update_available {
            Color::from_rgb(0.90, 0.63, 0.22)
        } else {
            self.theme().extended_palette().background.weak.text
        };

        let sidebar_footer = container(
            column![
                text("Version").size(text_size(11)).style(|theme: &Theme| {
                    iced::widget::text::Style {
                        color: Some(theme.extended_palette().background.weak.text),
                        ..Default::default()
                    }
                }),
                text(current_tag).size(text_size(13)),
                text(update_status)
                    .size(text_size(11))
                    .style(move |_theme: &Theme| iced::widget::text::Style {
                        color: Some(sidebar_status_color),
                        ..Default::default()
                    })
            ]
            .spacing(4)
            .width(Length::Fill),
        )
        .padding(Padding {
            top: 10.0,
            right: 12.0,
            bottom: 12.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(move |theme: &Theme| {
            if let Some(background) = kde_sidebar_footer_bg {
                return container::Style {
                    background: Some(background.into()),
                    border: Border {
                        radius: 0.0.into(),
                        width: 1.0,
                        color: kde_sidebar_border.unwrap_or(background),
                    },
                    ..Default::default()
                };
            }
            let palette = theme.extended_palette();
            let base_bg = palette.background.base.color;
            let (background, border_color) = if palette.is_dark {
                (
                    theme::palette::mix(base_bg, Color::BLACK, 0.18),
                    theme::palette::mix(base_bg, Color::BLACK, 0.34),
                )
            } else {
                (
                    palette.background.weaker.color,
                    palette.background.strong.color,
                )
            };
            container::Style {
                background: Some(background.into()),
                border: Border {
                    radius: 0.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                ..Default::default()
            }
        });

        let sidebar_content = column![header, nav_scroll, sidebar_footer]
            .width(Length::Fixed(SIDEBAR_WIDTH))
            .height(Length::Fill);

        container(sidebar_content)
            .style(move |theme: &Theme| {
                if let Some(sidebar_bg) = kde_sidebar_bg {
                    return container::Style {
                        background: Some(Background::Color(platform_surface_color(
                            sidebar_bg,
                            if theme.extended_palette().is_dark {
                                0.48
                            } else {
                                0.24
                            },
                        ))),
                        border: Border {
                            width: if use_glass_effects() { 0.0 } else { 1.0 },
                            color: kde_sidebar_border.unwrap_or(sidebar_bg),
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                }
                let palette = theme.extended_palette();
                let base_bg = palette.background.base.color;
                let sidebar_bg = if palette.is_dark {
                    theme::palette::mix(base_bg, Color::BLACK, 0.24)
                } else {
                    palette.background.weak.color
                };
                let border_color = if palette.is_dark {
                    theme::palette::mix(base_bg, Color::BLACK, 0.40)
                } else {
                    palette.background.strong.color
                };
                container::Style {
                    background: Some(Background::Color(platform_surface_color(
                        sidebar_bg,
                        if palette.is_dark { 0.48 } else { 0.24 },
                    ))),
                    border: Border {
                        width: if use_glass_effects() { 0.0 } else { 1.0 },
                        color: border_color,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .width(Length::Fixed(SIDEBAR_WIDTH))
            .height(Length::Fill)
            .into()
    }

    fn nav_button(&self, page: Page) -> Element<'_, Message> {
        let is_active = self.current_page == page;
        #[cfg(target_os = "linux")]
        let use_system_selection =
            matches!(self.theme_mode, ThemeMode::System) && self.system_kde_theme.is_some();
        #[cfg(not(target_os = "linux"))]
        let use_system_selection = false;

        #[cfg(target_os = "linux")]
        let (
            kde_nav_text,
            kde_nav_hover_bg,
            kde_nav_hover_text,
            kde_nav_selected_bg,
            kde_nav_selected_text,
        ) = if use_system_selection {
            (
                self.system_kde_nav_text,
                self.system_kde_nav_hover_bg,
                self.system_kde_nav_hover_text,
                self.system_kde_nav_selected_bg,
                self.system_kde_nav_selected_text,
            )
        } else {
            (None, None, None, None, None)
        };
        #[cfg(not(target_os = "linux"))]
        let (
            kde_nav_text,
            kde_nav_hover_bg,
            kde_nav_hover_text,
            kde_nav_selected_bg,
            kde_nav_selected_text,
        ): (
            Option<Color>,
            Option<Color>,
            Option<Color>,
            Option<Color>,
            Option<Color>,
        ) = (None, None, None, None, None);

        let icon_width = text_size(16) * 1.5;
        let label = row![
            text(page.icon())
                .size(text_size(16))
                .width(Length::Fixed(icon_width)),
            text(page.title()).size(text_size(14)),
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
                let (background, text_color, border_color, border_width) = if is_active {
                    if use_system_selection {
                        let selected_bg = kde_nav_selected_bg.unwrap_or(palette.primary.base.color);
                        let selected_fg_preferred = kde_nav_selected_text
                            .or(kde_nav_text)
                            .unwrap_or(palette.primary.base.text);
                        let selected_fg_fallback = palette.background.base.text;
                        (
                            selected_bg,
                            readable_on(selected_bg, selected_fg_preferred, selected_fg_fallback),
                            Color::TRANSPARENT,
                            0.0,
                        )
                    } else if palette.is_dark {
                        (
                            theme::palette::mix(palette.background.base.color, Color::BLACK, 0.28),
                            palette.background.base.text,
                            theme::palette::mix(palette.background.base.color, Color::BLACK, 0.45),
                            1.0,
                        )
                    } else {
                        (
                            palette.primary.strong.color,
                            Color::WHITE,
                            Color::TRANSPARENT,
                            0.0,
                        )
                    }
                } else {
                    match status {
                        button::Status::Hovered => {
                            if use_system_selection {
                                let hover_bg = kde_nav_hover_bg.unwrap_or_else(|| {
                                    theme::palette::mix(
                                        kde_nav_selected_bg.unwrap_or(palette.primary.base.color),
                                        palette.background.base.color,
                                        0.82,
                                    )
                                });
                                let hover_fg_preferred = kde_nav_hover_text
                                    .or(kde_nav_text)
                                    .unwrap_or(palette.background.base.text);
                                let hover_fg_fallback = kde_nav_selected_text
                                    .or(kde_nav_text)
                                    .unwrap_or(palette.background.base.text);
                                (
                                    hover_bg,
                                    readable_on(hover_bg, hover_fg_preferred, hover_fg_fallback),
                                    Color::TRANSPARENT,
                                    0.0,
                                )
                            } else if palette.is_dark {
                                (
                                    theme::palette::mix(
                                        palette.background.base.color,
                                        Color::BLACK,
                                        0.18,
                                    ),
                                    palette.background.base.text,
                                    Color::TRANSPARENT,
                                    0.0,
                                )
                            } else {
                                (
                                    palette.background.weak.color,
                                    palette.background.weak.text,
                                    Color::TRANSPARENT,
                                    0.0,
                                )
                            }
                        }
                        _ => {
                            if use_system_selection {
                                (
                                    Color::TRANSPARENT,
                                    kde_nav_text.unwrap_or(palette.background.weak.text),
                                    Color::TRANSPARENT,
                                    0.0,
                                )
                            } else {
                                (
                                    Color::TRANSPARENT,
                                    palette.background.weak.text,
                                    Color::TRANSPARENT,
                                    0.0,
                                )
                            }
                        }
                    }
                };

                button::Style {
                    background: Some(background.into()),
                    text_color,
                    border: Border {
                        radius: 6.0.into(),
                        width: border_width,
                        color: border_color,
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
            Page::Updates => self.view_updates(),
            Page::EditFrontend => self.view_edit_frontend(),
            Page::EditBackend => self.view_edit_backend(),
        };

        // Traffic inspection handles its own top-level layout and background.
        if self.current_page == Page::TrafficInspection {
            page_content
        } else if self.current_page == Page::Monitoring {
            container(page_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |theme: &Theme| container::Style {
                    background: Some(Background::Color({
                        let bg = self.surface_page_bg(theme);
                        let palette = theme.extended_palette();
                        let alpha = if use_glass_effects() {
                            if palette.is_dark { 0.32 } else { 0.22 }
                        } else {
                            1.0
                        };
                        platform_surface_color(bg, alpha)
                    })),
                    ..Default::default()
                })
                .into()
        } else {
            let page_title = text(self.current_page.title()).size(text_size(20));
            let content = column![page_title, page_content]
                .spacing(20)
                .padding(30)
                .width(Length::Fill);

            Scrollable::new(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |theme: &Theme, status| scrollable::Style {
                    container: container::Style {
                        background: Some(Background::Color({
                            let bg = self.surface_page_bg(theme);
                            let palette = theme.extended_palette();
                            let alpha = if use_glass_effects() {
                                if palette.is_dark { 0.32 } else { 0.22 }
                            } else {
                                1.0
                            };
                            platform_surface_color(bg, alpha)
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
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::System => {
                #[cfg(target_os = "linux")]
                {
                    return self.system_kde_theme.clone().unwrap_or_else(|| {
                        match self.system_theme {
                            Some(theme::Mode::Light) => Theme::Light,
                            Some(theme::Mode::Dark) => Theme::Dark,
                            _ => Theme::Dark,
                        }
                    });
                }
                #[cfg(not(target_os = "linux"))]
                {
                    match self.system_theme {
                        Some(theme::Mode::Light) => Theme::Light,
                        Some(theme::Mode::Dark) => Theme::Dark,
                        _ => Theme::Dark,
                    }
                }
            }
        }
    }
}

fn load_window_icon() -> Option<window::Icon> {
    let icon_bytes = &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ob3.png"))[..];
    window::icon::from_file_data(icon_bytes, None).ok()
}

fn make_window_settings() -> iced::window::Settings {
    let use_glass_effects = use_glass_effects();
    let mut window_settings = iced::window::Settings {
        size: iced::Size::new(WINDOW_INITIAL_WIDTH, WINDOW_INITIAL_HEIGHT),
        min_size: Some(iced::Size::new(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT)),
        decorations: true, // Use native window decorations (KDE/GNOME title bar)
        blur: use_glass_effects,
        transparent: use_glass_effects,
        icon: load_window_icon(),
        // Disable default close behavior so we can intercept and hide instead
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        exit_on_close_request: false,
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    {
        window_settings.platform_specific.application_id = linux_application_id();
        // Force fully opaque windows on Linux.
        window_settings.blur = false;
        window_settings.transparent = false;
    }
    window_settings
}

#[cfg(target_os = "linux")]
fn linux_application_id() -> String {
    const DEFAULT_APPLICATION_ID: &str = "io.odd.box";
    std::env::var("ODD_BOX_LINUX_APPLICATION_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_APPLICATION_ID.to_string())
}

#[cfg(target_os = "linux")]
fn linux_data_home() -> Option<PathBuf> {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        let data_home = data_home.trim();
        if !data_home.is_empty() {
            return Some(PathBuf::from(data_home));
        }
    }
    let home = std::env::var("HOME").ok()?;
    let home = home.trim();
    if home.is_empty() {
        None
    } else {
        Some(PathBuf::from(home).join(".local/share"))
    }
}

#[cfg(target_os = "linux")]
fn desktop_exec_escape(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len());
    for ch in arg.chars() {
        match ch {
            ' ' | '\t' | '\n' | '"' | '\'' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn maybe_refresh_desktop_caches(data_home: &std::path::Path) {
    let applications_dir = data_home.join("applications");
    if let Ok(path) = std::env::var("PATH") {
        if path.split(':').any(|segment| {
            !segment.is_empty()
                && PathBuf::from(segment)
                    .join("update-desktop-database")
                    .exists()
        }) {
            let _ = std::process::Command::new("update-desktop-database")
                .arg(&applications_dir)
                .output();
        }
        if path.split(':').any(|segment| {
            !segment.is_empty()
                && PathBuf::from(segment)
                    .join("gtk-update-icon-cache")
                    .exists()
        }) {
            let _ = std::process::Command::new("gtk-update-icon-cache")
                .args(["-f", "-t"])
                .arg(data_home.join("icons/hicolor"))
                .output();
        }
    }
}

#[cfg(target_os = "linux")]
fn ensure_linux_desktop_entry() -> Result<(), String> {
    if std::env::var("ODD_BOX_DISABLE_AUTO_DESKTOP_ENTRY")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
    {
        return Ok(());
    }

    let desktop_id = linux_application_id();
    let data_home = match linux_data_home() {
        Some(path) => path,
        None => return Ok(()),
    };
    let applications_dir = data_home.join("applications");
    let icon_dir = data_home.join("icons/hicolor/256x256/apps");
    std::fs::create_dir_all(&applications_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&icon_dir).map_err(|e| e.to_string())?;

    let icon_path = icon_dir.join(format!("{desktop_id}.png"));
    let icon_bytes = &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ob3.png"))[..];
    if std::fs::read(&icon_path).ok().as_deref() != Some(icon_bytes) {
        std::fs::write(&icon_path, icon_bytes).map_err(|e| e.to_string())?;
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    let exec = desktop_exec_escape(&exe);
    let desktop_body = format!(
        "[Desktop Entry]\nType=Application\nName=ODD-BOX\nComment=odd-box GUI\nExec={exec}\nIcon={desktop_id}\nTerminal=false\nCategories=Network;Development;\nStartupWMClass={desktop_id}\nX-GNOME-WMClass={desktop_id}\n"
    );
    let desktop_path = applications_dir.join(format!("{desktop_id}.desktop"));
    if std::fs::read_to_string(&desktop_path).ok().as_deref() != Some(desktop_body.as_str()) {
        std::fs::write(&desktop_path, desktop_body).map_err(|e| e.to_string())?;
        maybe_refresh_desktop_caches(&data_home);
        tracing::info!(
            "Installed/updated linux desktop entry for GUI icon mapping: {}",
            desktop_path.display()
        );
    }

    Ok(())
}
