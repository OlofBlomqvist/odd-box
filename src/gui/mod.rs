pub mod logs;
mod pages;

use iced::widget::{
    Column, Scrollable, button, column, container, row, rule, text,
};
use iced::{
    Border, Color, Element, Font, Length, Padding, Subscription, Task, Theme, system, theme, time,
};
use std::sync::Arc;

use crate::global_state::GlobalState;
use logs::{LogFilter, SharedLogState};
use pages::{fetch_config, CachedConfig, CachedLogLine};

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
        min_size: Some(iced::Size::new(1200.0, 400.0)),
        decorations: true, // Use native window decorations (KDE/GNOME title bar)
        ..Default::default()
    };

    let state_clone = state.clone();
    let log_state_clone = log_state.clone();

    iced::application(
        move || OddBoxGui::new(state_clone.clone(), theme_mode, log_state_clone.clone()),
        OddBoxGui::update,
        OddBoxGui::view,
    )
        .theme(OddBoxGui::theme)
        .subscription(OddBoxGui::subscription)
        .window(window_settings)
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevelPreset {
    #[default]
    All,
    DebugAndAbove,
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

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(Page),
    SystemThemeChanged(theme::Mode),
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
}

pub struct OddBoxGui {
    pub(in crate::gui) state: Arc<GlobalState>,
    pub(in crate::gui) log_state: SharedLogState,
    current_page: Page,
    theme_mode: ThemeMode,
    system_theme: Option<theme::Mode>,
    // Log filtering
    pub(in crate::gui) log_filter: LogFilter,
    pub(in crate::gui) log_level_preset: LogLevelPreset,
    // Cached list of known sources
    pub(in crate::gui) known_sources: Vec<String>,
    // Cached filtered log lines for performance
    pub(in crate::gui) cached_log_lines: Vec<CachedLogLine>,
    pub(in crate::gui) last_log_count: usize,
    pub(in crate::gui) total_log_count: usize,
    // Track last seen log ID to avoid unnecessary rebuilds
    pub(in crate::gui) last_seen_log_id: Option<u64>,
    // Log display options
    pub(in crate::gui) log_wrap_enabled: bool,
    pub(in crate::gui) log_auto_tail: bool,
    // Cached config data
    pub(in crate::gui) cached_config: CachedConfig,
}

impl OddBoxGui {
    fn new(
        state: Arc<GlobalState>,
        theme_mode: ThemeMode,
        log_state: SharedLogState,
    ) -> (Self, Task<Message>) {
        let state_clone = state.clone();
        let mut tasks: Vec<Task<Message>> =
            vec![Task::perform(fetch_config(state_clone), Message::ConfigUpdated)];

        // On system mode, grab current OS theme (winit-powered)
        if matches!(theme_mode, ThemeMode::System) {
            tasks.push(system::theme().map(Message::SystemThemeChanged));
        }

        (
            Self {
                state,
                log_state,
                current_page: Page::Dashboard,
                theme_mode,
                system_theme: None,
                log_filter: LogFilter::new(),
                log_level_preset: LogLevelPreset::All,
                known_sources: Vec::new(),
                cached_log_lines: Vec::new(),
                last_log_count: 0,
                total_log_count: 0,
                last_seen_log_id: None,
                log_wrap_enabled: false,
                log_auto_tail: true, // Auto-tail enabled by default
                cached_config: CachedConfig::default(),
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
            _ => Subscription::none(),
        };

        let theme_sub = system::theme_changes().map(Message::SystemThemeChanged);

        Subscription::batch(vec![page_sub, theme_sub])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(page) => {
                self.current_page = page;
                if page == Page::Monitoring {
                    self.refresh_log_cache(true);
                }
                // Trigger config refresh for config-related pages
                if matches!(
                    page,
                    Page::ManagedProcesses | Page::Backends | Page::Frontends | Page::Dashboard
                ) {
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
            }
            Message::SystemThemeChanged(mode) => {
                self.system_theme = Some(mode);
            }
            Message::Tick => {
                if self.current_page == Page::Monitoring {
                    self.refresh_log_cache(false);
                }
                // Refresh config for config-related pages
                if matches!(
                    self.current_page,
                    Page::ManagedProcesses | Page::Backends | Page::Frontends | Page::Dashboard
                ) {
                    return Task::perform(fetch_config(self.state.clone()), Message::ConfigUpdated);
                }
            }
            Message::ConfigUpdated(config) => {
                self.cached_config = config;
            }
            Message::LogFilterTextChanged(text) => {
                self.log_filter.text = text;
                self.refresh_log_cache(true);
            }
            Message::LogLevelPresetChanged(preset) => {
                self.log_level_preset = preset;
                // Apply preset to log filter
                match preset {
                    LogLevelPreset::All => {
                        self.log_filter.show_trace = true;
                        self.log_filter.show_debug = true;
                        self.log_filter.show_info = true;
                        self.log_filter.show_warn = true;
                        self.log_filter.show_error = true;
                    }
                    LogLevelPreset::DebugAndAbove => {
                        self.log_filter.show_trace = false;
                        self.log_filter.show_debug = true;
                        self.log_filter.show_info = true;
                        self.log_filter.show_warn = true;
                        self.log_filter.show_error = true;
                    }
                    LogLevelPreset::InfoAndAbove => {
                        self.log_filter.show_trace = false;
                        self.log_filter.show_debug = false;
                        self.log_filter.show_info = true;
                        self.log_filter.show_warn = true;
                        self.log_filter.show_error = true;
                    }
                    LogLevelPreset::WarnAndAbove => {
                        self.log_filter.show_trace = false;
                        self.log_filter.show_debug = false;
                        self.log_filter.show_info = false;
                        self.log_filter.show_warn = true;
                        self.log_filter.show_error = true;
                    }
                    LogLevelPreset::ErrorOnly => {
                        self.log_filter.show_trace = false;
                        self.log_filter.show_debug = false;
                        self.log_filter.show_info = false;
                        self.log_filter.show_warn = false;
                        self.log_filter.show_error = true;
                    }
                }
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
                self.cached_log_lines.clear();
                self.last_log_count = 0;
                self.total_log_count = 0;
                self.last_seen_log_id = None;
            }
            Message::LogToggleWrap(enabled) => {
                self.log_wrap_enabled = enabled;
            }
            Message::LogToggleAutoTail(enabled) => {
                self.log_auto_tail = enabled;
            }
            Message::ProcessStart(name) => {
                let _ = self
                    .state
                    .proc_broadcaster
                    .send(crate::control::ProcMessage::Start(name));
            }
            Message::ProcessStop(name) => {
                let _ = self
                    .state
                    .proc_broadcaster
                    .send(crate::control::ProcMessage::Stop(name));
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let divider = rule::vertical(1);
        let content = self.view_content();

        row![sidebar, divider, content]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let header = container(
            column![
                text("odd-box").font(Font::MONOSPACE),
                text("reverse proxy").color(self.theme().extended_palette().background.strong.text),
            ]
            .spacing(4),
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
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
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

                let background = if is_active {
                    palette.primary.weak.color
                } else {
                    match status {
                        button::Status::Hovered => palette.background.weak.color,
                        _ => Color::TRANSPARENT,
                    }
                };

                let text_color = if is_active {
                    palette.primary.weak.text
                } else {
                    palette.background.base.text
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
        };

        // Monitoring page handles its own layout (no extra scrollable wrapper)
        if self.current_page == Page::Monitoring {
            page_content
        } else {
            let page_title = text(self.current_page.title());
            let content = column![page_title, page_content]
                .spacing(20)
                .padding(30)
                .width(Length::Fill);

            Scrollable::new(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    fn view_placeholder(&self, description: &'static str) -> Element<'_, Message> {
        container(text(description).color(self.theme().extended_palette().background.strong.text))
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
