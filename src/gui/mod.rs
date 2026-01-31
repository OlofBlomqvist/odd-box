pub mod logs;

use iced::widget::{
    Column, Id, Row, Scrollable, Space, button, checkbox, column, container, row, rule, scrollable,
    text, text_input,
};
use iced::{
    Border, Color, Element, Font, Length, Padding, Subscription, Task, Theme, system, theme, time,
};
use std::sync::Arc;
use tracing::Level;

use crate::configuration::{self};
use crate::global_state::GlobalState;
use crate::types::app_state::ProcState;
use logs::{LogFilter, SharedLogState};

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
        min_size: Some(iced::Size::new(600.0, 400.0)),
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

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(Page),
    SystemThemeChanged(theme::Mode),
    // Log filter messages
    LogFilterTextChanged(String),
    LogFilterToggleTrace(bool),
    LogFilterToggleDebug(bool),
    LogFilterToggleInfo(bool),
    LogFilterToggleWarn(bool),
    LogFilterToggleError(bool),
    LogFilterToggleSource(String, bool),
    LogFilterClearSources,
    LogsClear,
    // Tick for refreshing log view
    Tick,
    // Virtual scroll tracking
    LogScrolled(scrollable::Viewport),
    // Config data updated
    ConfigUpdated(CachedConfig),
}

/// Cached/pre-rendered log line for performance
#[derive(Clone)]
struct CachedLogLine {
    id: u64,
    level_str: &'static str,
    level_color: Color,
    source: String,
    message: String,
}

/// Cached process info for display
#[derive(Clone, Debug)]
struct CachedProcess {
    name: String,
    bin: String,
    port: String,
    protocol: String,
    state: ProcState,
    auto_start: bool,
}

/// Cached remote backend info for display
#[derive(Clone, Debug)]
struct CachedRemoteBackend {
    name: String,
    endpoints: String,
    protocol: String,
    https: bool,
}

/// Cached static backend info for display
#[derive(Clone, Debug)]
struct CachedStaticBackend {
    name: String,
    dir: String,
    list_dir: bool,
}

/// Cached route info for display
#[derive(Clone, Debug)]
struct CachedRoute {
    hostname: String,
    backend: String,
    https_redirect: bool,
    capture_subdomains: bool,
}

/// All cached config data
#[derive(Clone, Debug, Default)]
struct CachedConfig {
    processes: Vec<CachedProcess>,
    remote_backends: Vec<CachedRemoteBackend>,
    static_backends: Vec<CachedStaticBackend>,
    routes: Vec<CachedRoute>,
}

/// Constants for virtual scrolling
const LOG_ROW_HEIGHT: f32 = 18.0;
const LOG_VIEWPORT_BUFFER: usize = 5; // Extra rows above/below visible area

pub struct OddBoxGui {
    state: Arc<GlobalState>,
    log_state: SharedLogState,
    current_page: Page,
    theme_mode: ThemeMode,
    system_theme: Option<theme::Mode>,
    // Log filtering
    log_filter: LogFilter,
    // Cached list of known sources
    known_sources: Vec<String>,
    // Cached filtered log lines for performance
    cached_log_lines: Vec<CachedLogLine>,
    last_log_count: usize,
    total_log_count: usize,
    // Virtual scroll state
    log_scroll_offset: f32,
    log_viewport_height: f32,
    // Cached config data
    cached_config: CachedConfig,
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
                known_sources: Vec::new(),
                cached_log_lines: Vec::new(),
                last_log_count: 0,
                total_log_count: 0,
                log_scroll_offset: 0.0,
                log_viewport_height: 600.0, // Default, updated on scroll
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
                    self.refresh_log_cache();
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
                    self.refresh_log_cache();
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
                self.refresh_log_cache();
            }
            Message::LogFilterToggleTrace(enabled) => {
                self.log_filter.show_trace = enabled;
                self.refresh_log_cache();
            }
            Message::LogFilterToggleDebug(enabled) => {
                self.log_filter.show_debug = enabled;
                self.refresh_log_cache();
            }
            Message::LogFilterToggleInfo(enabled) => {
                self.log_filter.show_info = enabled;
                self.refresh_log_cache();
            }
            Message::LogFilterToggleWarn(enabled) => {
                self.log_filter.show_warn = enabled;
                self.refresh_log_cache();
            }
            Message::LogFilterToggleError(enabled) => {
                self.log_filter.show_error = enabled;
                self.refresh_log_cache();
            }
            Message::LogFilterToggleSource(source, enabled) => {
                if enabled {
                    self.log_filter.sources.insert(source);
                } else {
                    self.log_filter.sources.remove(&source);
                }
                self.refresh_log_cache();
            }
            Message::LogFilterClearSources => {
                self.log_filter.sources.clear();
                self.refresh_log_cache();
            }
            Message::LogsClear => {
                self.log_state.write().clear();
                self.cached_log_lines.clear();
                self.last_log_count = 0;
                self.total_log_count = 0;
                self.log_scroll_offset = 0.0;
            }
            Message::LogScrolled(viewport) => {
                self.log_scroll_offset = viewport.absolute_offset().y;
                self.log_viewport_height = viewport.bounds().height;
            }
        }
        Task::none()
    }

    fn refresh_log_cache(&mut self) {
        let state = self.log_state.read();

        // Update known sources
        let sources = state.known_sources().clone();
        let mut sources_vec: Vec<String> = sources.into_iter().collect();
        sources_vec.sort();
        self.known_sources = sources_vec;

        self.total_log_count = state.len();

        // Apply filter and cache results (no limit - virtual scroll handles large lists)
        let filtered = self.log_filter.apply(state.entries());

        self.cached_log_lines = filtered
            .into_iter()
            .map(|entry| {
                let (level_str, level_color) = Self::level_display(entry.level);
                let source = entry
                    .thread
                    .as_ref()
                    .filter(|t| !t.is_empty())
                    .cloned()
                    .or_else(|| {
                        if entry.source.is_empty() {
                            None
                        } else {
                            Some(entry.source.clone())
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                CachedLogLine {
                    id: entry.id,
                    level_str,
                    level_color,
                    source,
                    message: entry.message.clone(),
                }
            })
            .collect();

        self.last_log_count = self.cached_log_lines.len();
    }

    fn level_display(level: Level) -> (&'static str, Color) {
        match level {
            Level::TRACE => ("TRC", Color::from_rgb(0.6, 0.6, 0.6)),
            Level::DEBUG => ("DBG", Color::from_rgb(0.4, 0.7, 1.0)),
            Level::INFO => ("INF", Color::from_rgb(0.4, 0.85, 0.4)),
            Level::WARN => ("WRN", Color::from_rgb(1.0, 0.8, 0.3)),
            Level::ERROR => ("ERR", Color::from_rgb(1.0, 0.4, 0.4)),
        }
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

    fn view_monitoring(&self) -> Element<'_, Message> {
        let title_row = row![
            text("Monitoring"),
            Space::new().width(Length::Fill),
            button(text("Clear Logs"))
                .padding(Padding {
                    top: 6.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                })
                .on_press(Message::LogsClear),
        ]
        .align_y(iced::Alignment::Center);

        // Filter controls
        let filter_bar = self.view_log_filter_bar();

        // Source filter
        let source_filter = self.view_source_filter();

        // Log entries
        let log_entries = self.view_log_entries();

        let content = column![title_row, filter_bar, source_filter, log_entries]
            .spacing(15)
            .padding(30)
            .width(Length::Fill)
            .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_log_filter_bar(&self) -> Element<'_, Message> {
        let search_input = text_input("Search logs...", &self.log_filter.text)
            .on_input(Message::LogFilterTextChanged)
            .padding(8)
            .width(Length::Fixed(250.0));

        let level_filters = row![
            checkbox(self.log_filter.show_trace)
                .label("TRC")
                .on_toggle(Message::LogFilterToggleTrace),
            checkbox(self.log_filter.show_debug)
                .label("DBG")
                .on_toggle(Message::LogFilterToggleDebug),
            checkbox(self.log_filter.show_info)
                .label("INF")
                .on_toggle(Message::LogFilterToggleInfo),
            checkbox(self.log_filter.show_warn)
                .label("WRN")
                .on_toggle(Message::LogFilterToggleWarn),
            checkbox(self.log_filter.show_error)
                .label("ERR")
                .on_toggle(Message::LogFilterToggleError),
        ]
        .spacing(12);

        let log_count = if self.has_active_filter() {
            format!("{} / {} logs", self.last_log_count, self.total_log_count)
        } else {
            format!("{} logs", self.total_log_count)
        };

        row![
            search_input,
            Space::new().width(Length::Fill),
            level_filters,
            Space::new().width(Length::Fixed(20.0)),
            text(log_count).color(self.theme().extended_palette().background.strong.text),
        ]
        .spacing(15)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_source_filter(&self) -> Element<'_, Message> {
        if self.known_sources.is_empty() {
            return Space::new().height(Length::Fixed(0.0)).into();
        }

        let mut source_chips: Vec<Element<'_, Message>> = Vec::new();

        // Add "Clear" button if any sources are selected
        if !self.log_filter.sources.is_empty() {
            source_chips.push(
                button(text("Clear"))
                    .padding(Padding {
                        top: 4.0,
                        right: 8.0,
                        bottom: 4.0,
                        left: 8.0,
                    })
                    .style(|theme: &Theme, _status| {
                        let palette = theme.extended_palette();
                        button::Style {
                            background: Some(palette.background.weak.color.into()),
                            text_color: palette.background.base.text,
                            border: Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .on_press(Message::LogFilterClearSources)
                    .into(),
            );
        }

        // Add source chips
        for source in &self.known_sources {
            let is_selected = self.log_filter.sources.contains(source);
            let source_clone = source.clone();
            let chip = button(text(source.as_str()))
                .padding(Padding {
                    top: 4.0,
                    right: 8.0,
                    bottom: 4.0,
                    left: 8.0,
                })
                .style(move |theme: &Theme, _status| {
                    let palette = theme.extended_palette();
                    let (bg, fg) = if is_selected {
                        (palette.primary.strong.color, palette.primary.strong.text)
                    } else {
                        (
                            palette.background.strong.color,
                            palette.background.strong.text,
                        )
                    };
                    button::Style {
                        background: Some(bg.into()),
                        text_color: fg,
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })
                .on_press(Message::LogFilterToggleSource(source_clone, !is_selected));
            source_chips.push(chip.into());
        }

        let chips_row = Row::with_children(source_chips).spacing(6).wrap();

        container(
            column![
                text("Filter by source:")
                    .color(self.theme().extended_palette().background.strong.text),
                chips_row,
            ]
            .spacing(6),
        )
        .padding(Padding {
            top: 5.0,
            right: 0.0,
            bottom: 5.0,
            left: 0.0,
        })
        .into()
    }

    fn view_log_entries(&self) -> Element<'_, Message> {
        if self.cached_log_lines.is_empty() {
            let msg = if self.total_log_count == 0 {
                "No logs yet..."
            } else {
                "No logs match the current filter"
            };
            return container(
                text(msg).color(self.theme().extended_palette().background.strong.text),
            )
            .padding(20)
            .width(Length::Fill)
            .into();
        }

        let total_items = self.cached_log_lines.len();

        // Calculate visible range
        let first_visible = (self.log_scroll_offset / LOG_ROW_HEIGHT).floor() as usize;
        let visible_count = (self.log_viewport_height / LOG_ROW_HEIGHT).ceil() as usize + 1;

        // Add buffer and clamp to valid range
        let start_idx = first_visible.saturating_sub(LOG_VIEWPORT_BUFFER);
        let end_idx = (first_visible + visible_count + LOG_VIEWPORT_BUFFER).min(total_items);

        // Space above visible items
        let top_space_height = start_idx as f32 * LOG_ROW_HEIGHT;
        // Space below visible items
        let bottom_space_height = (total_items - end_idx) as f32 * LOG_ROW_HEIGHT;

        // Build only the visible rows
        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(end_idx - start_idx + 2);

        // Top spacer
        if top_space_height > 0.0 {
            rows.push(Space::new().height(Length::Fixed(top_space_height)).into());
        }

        // Visible log rows
        for line in &self.cached_log_lines[start_idx..end_idx] {
            let row_content = row![
                text(line.level_str)
                    .font(Font::MONOSPACE)
                    .color(line.level_color)
                    .width(Length::Fixed(36.0)),
                text(truncate_str(&line.source, 18))
                    .font(Font::MONOSPACE)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
                    .width(Length::Fixed(160.0)),
                text(&line.message).font(Font::MONOSPACE),
            ]
            .spacing(8)
            .height(Length::Fixed(LOG_ROW_HEIGHT));

            rows.push(row_content.into());
        }

        // Bottom spacer
        if bottom_space_height > 0.0 {
            rows.push(Space::new().height(Length::Fixed(bottom_space_height)).into());
        }

        let log_column = Column::with_children(rows).width(Length::Fill);

        scrollable(
            container(log_column)
                .padding(Padding {
                    top: 10.0,
                    right: 15.0,
                    bottom: 10.0,
                    left: 15.0,
                })
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(palette.background.weak.color.into()),
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        )
        .on_scroll(Message::LogScrolled)
        .id(Id::new("logs"))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn has_active_filter(&self) -> bool {
        !self.log_filter.text.is_empty()
            || !self.log_filter.sources.is_empty()
            || !self.log_filter.show_trace
            || !self.log_filter.show_debug
            || !self.log_filter.show_info
            || !self.log_filter.show_warn
            || !self.log_filter.show_error
    }

    fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| format!("{:.0?}", d))
            .unwrap_or_else(|_| "Unknown".to_string());

        let status_items =
            column![text("Status: Running"), text(format!("Uptime: {}", uptime)),].spacing(8);

        container(status_items)
            .padding(20)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_processes(&self) -> Element<'_, Message> {
        if self.cached_config.processes.is_empty() {
            return text("No managed processes configured").into();
        }

        // Table header
        let header = row![
            text("Name")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("Binary")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("Port")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(80.0)),
            text("Protocol")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(80.0)),
            text("Status")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(100.0)),
            text("Auto")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(60.0)),
        ]
        .spacing(10)
        .padding(Padding {
            top: 8.0,
            right: 10.0,
            bottom: 8.0,
            left: 10.0,
        });

        let header_container = container(header)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    ..Default::default()
                }
            });

        // Table rows
        let rows: Vec<Element<'_, Message>> = self
            .cached_config
            .processes
            .iter()
            .map(|proc| {
                let status_color = match proc.state {
                    ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                    ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                    ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                    ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                    _ => Color::from_rgb(0.6, 0.6, 0.6),
                };

                row![
                    text(&proc.name)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(&proc.bin)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(&proc.port)
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(80.0)),
                    text(&proc.protocol)
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(80.0)),
                    text(format!("{:?}", proc.state))
                        .font(Font::MONOSPACE)
                        .color(status_color)
                        .width(Length::Fixed(100.0)),
                    text(if proc.auto_start { "Yes" } else { "No" })
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(60.0)),
                ]
                .spacing(10)
                .padding(Padding {
                    top: 6.0,
                    right: 10.0,
                    bottom: 6.0,
                    left: 10.0,
                })
                .into()
            })
            .collect();

        let table = column![header_container]
            .push(Column::with_children(rows).spacing(2))
            .width(Length::Fill);

        container(table)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();

        // Remote backends section
        if !self.cached_config.remote_backends.is_empty() {
            let header = row![
                text("Name")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(2)),
                text("Endpoints")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(3)),
                text("Protocol")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(80.0)),
                text("HTTPS")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(60.0)),
            ]
            .spacing(10)
            .padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 8.0,
                left: 10.0,
            });

            let header_container = container(header)
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(palette.background.weak.color.into()),
                        ..Default::default()
                    }
                });

            let rows: Vec<Element<'_, Message>> = self
                .cached_config
                .remote_backends
                .iter()
                .map(|backend| {
                    row![
                        text(&backend.name)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2)),
                        text(&backend.endpoints)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(3)),
                        text(&backend.protocol)
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(80.0)),
                        text(if backend.https { "Yes" } else { "No" })
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(60.0)),
                    ]
                    .spacing(10)
                    .padding(Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 6.0,
                        left: 10.0,
                    })
                    .into()
                })
                .collect();

            let table = column![header_container]
                .push(Column::with_children(rows).spacing(2))
                .width(Length::Fill);

            sections.push(
                column![
                    text("Remote Backends"),
                    container(table).width(Length::Fill).style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Style {
                            background: Some(palette.background.weak.color.into()),
                            border: Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                ]
                .spacing(10)
                .into(),
            );
        }

        // Static backends section
        if !self.cached_config.static_backends.is_empty() {
            let header = row![
                text("Name")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(2)),
                text("Directory")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(4)),
                text("List Dir")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(80.0)),
            ]
            .spacing(10)
            .padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 8.0,
                left: 10.0,
            });

            let header_container = container(header)
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(palette.background.weak.color.into()),
                        ..Default::default()
                    }
                });

            let rows: Vec<Element<'_, Message>> = self
                .cached_config
                .static_backends
                .iter()
                .map(|backend| {
                    row![
                        text(&backend.name)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2)),
                        text(&backend.dir)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(4)),
                        text(if backend.list_dir { "Yes" } else { "No" })
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(80.0)),
                    ]
                    .spacing(10)
                    .padding(Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 6.0,
                        left: 10.0,
                    })
                    .into()
                })
                .collect();

            let table = column![header_container]
                .push(Column::with_children(rows).spacing(2))
                .width(Length::Fill);

            sections.push(
                column![
                    text("Static File Backends"),
                    container(table).width(Length::Fill).style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Style {
                            background: Some(palette.background.weak.color.into()),
                            border: Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                ]
                .spacing(10)
                .into(),
            );
        }

        if sections.is_empty() {
            return text("No backends configured")
                .color(self.theme().extended_palette().background.strong.text)
                .into();
        }

        Column::with_children(sections).spacing(20).into()
    }

    fn view_frontends(&self) -> Element<'_, Message> {
        if self.cached_config.routes.is_empty() {
            return text("No routes configured")
                .color(self.theme().extended_palette().background.strong.text)
                .into();
        }

        // Table header
        let header = row![
            text("Hostname")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(3)),
            text("Backend")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("HTTPS Redirect")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(120.0)),
            text("Subdomains")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(100.0)),
        ]
        .spacing(10)
        .padding(Padding {
            top: 8.0,
            right: 10.0,
            bottom: 8.0,
            left: 10.0,
        });

        let header_container = container(header)
            .width(Length::Fill)
            .style(container::rounded_box);

        // Table rows
        let rows: Vec<Element<'_, Message>> = self
            .cached_config
            .routes
            .iter()
            .map(|route| {
                row![
                    text(&route.hostname)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(3)),
                    text(&route.backend)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(if route.https_redirect { "Yes" } else { "No" })
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(120.0)),
                    text(if route.capture_subdomains {
                        "Yes"
                    } else {
                        "No"
                    })
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(100.0)),
                ]
                .spacing(10)
                .padding(Padding {
                    top: 6.0,
                    right: 10.0,
                    bottom: 6.0,
                    left: 10.0,
                })
                .into()
            })
            .collect();

        let table = column![header_container]
            .push(Column::with_children(rows).spacing(2))
            .width(Length::Fill);

        container(table)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn view_placeholder(&self, description: &'static str) -> Element<'_, Message> {
        container(text(description).color(self.theme().extended_palette().background.strong.text))
            .padding(20)
            .width(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
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

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Async function to fetch configuration data
async fn fetch_config(state: Arc<GlobalState>) -> CachedConfig {
    let config_guard = state.config.read().await;
    let status_map = &state.app_state.site_status_map;

    // Fetch processes
    let mut processes: Vec<CachedProcess> = config_guard
        .hosted_processes
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let proc = entry.value();
            let state = status_map
                .get(&name)
                .map(|v| v.value().clone())
                .unwrap_or(ProcState::Stopped);
            let port = proc
                .active_port
                .or(proc.port)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            CachedProcess {
                name,
                bin: proc.bin.clone(),
                port,
                protocol: format!("{:?}", proc.protocol),
                state,
                auto_start: proc.auto_start.unwrap_or(true),
            }
        })
        .collect();
    processes.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch remote backends
    let mut remote_backends: Vec<CachedRemoteBackend> = config_guard
        .remote_sites
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let remote = entry.value();
            let endpoints = remote
                .endpoints
                .iter()
                .map(|e| format!("{}:{}", e.addr, e.port))
                .collect::<Vec<_>>()
                .join(", ");
            CachedRemoteBackend {
                name,
                endpoints,
                protocol: format!("{:?}", remote.protocol),
                https: remote.https,
            }
        })
        .collect();
    remote_backends.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch static backends
    let mut static_backends: Vec<CachedStaticBackend> = config_guard
        .static_sites
        .iter()
        .map(|entry| {
            let name = entry.key().clone();
            let static_site = entry.value();
            CachedStaticBackend {
                name,
                dir: static_site.dir.clone(),
                list_dir: static_site.list_dir,
            }
        })
        .collect();
    static_backends.sort_by(|a, b| a.name.cmp(&b.name));

    // Fetch routes from frontends
    let mut routes: Vec<CachedRoute> = Vec::new();

    // Get routes from HTTP frontend
    if let Some(http) = &config_guard.frontends.http {
        for (hostname, target) in &http.routes {
            routes.push(CachedRoute {
                hostname: hostname.clone(),
                backend: target.backend_id().to_string(),
                https_redirect: target.redirect_to_https(),
                capture_subdomains: target.capture_subdomains(),
            });
        }
    }

    // Add any HTTPS-only routes
    if let Some(https) = &config_guard.frontends.https {
        if let Some(configuration::HttpsRoutes::Explicit(https_routes)) = &https.routes {
            for (hostname, target) in https_routes {
                // Only add if not already in the list from HTTP
                if !routes.iter().any(|r| r.hostname == *hostname) {
                    routes.push(CachedRoute {
                        hostname: hostname.clone(),
                        backend: target.backend_id().to_string(),
                        https_redirect: false,
                        capture_subdomains: target.capture_subdomains(),
                    });
                }
            }
        }
    }
    routes.sort_by(|a, b| a.hostname.cmp(&b.hostname));

    CachedConfig {
        processes,
        remote_backends,
        static_backends,
        routes,
    }
}
