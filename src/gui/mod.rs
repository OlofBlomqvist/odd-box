pub mod components;
pub mod logs;
mod pages;

use iced::widget::{
    Column, Id, Scrollable, button, column, container, image, row, scrollable, text,
};
use iced::{
    Application, Background, Border, Color, Element, Font, Length, Padding, Subscription, Task, Theme, system, theme, time
};
use std::sync::Arc;
use std::sync::LazyLock;

use crate::global_state::GlobalState;
use logs::{LogFilter, SharedLogState};
use pages::{CachedConfig, CachedLogLine, fetch_config};

static SIDEBAR_LOGO: LazyLock<iced::widget::image::Handle> = LazyLock::new(|| {
    iced::widget::image::Handle::from_bytes(
        &include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ob3.png"))[..],
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
        min_size: Some(iced::Size::new(1200.0, 400.0)),
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
    .style(|state: _, theme: &Theme| {
        theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    })
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

#[derive(Debug, Clone)]
pub enum Message {
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
}

fn log_scroll_id() -> Id {
    Id::new("odd_box_log_scroll")
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
                todo!("somehow get a hold of whichever proc_host is responsible for this process and tell it to enable+start it")
            }
            Message::ProcessStop(name) => {
                todo!("somehow get a hold of whichever proc_host is responsible for this process and tell it to disable+stop it")
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
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),//Some(palette.background.base.color.into()),
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let logo = image(SIDEBAR_LOGO.clone()).expand(true);

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
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)), //Some(palette.background.weaker.color.into()),
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
                    (palette.primary.strong.color, palette.primary.strong.text)
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
