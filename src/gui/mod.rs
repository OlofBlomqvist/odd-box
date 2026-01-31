use iced::widget::{button, column, container, row, text, vertical_rule, Column, Scrollable};
use iced::{Background, Border, Color, Element, Length, Padding, Task, Theme};
use std::sync::Arc;

use crate::global_state::GlobalState;

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

    fn resolve(&self) -> Theme {
        match self {
            ThemeMode::Light => Theme::Light,
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::System => {
                match dark_light::detect() {
                    Ok(dark_light::Mode::Dark) => Theme::Dark,
                    Ok(dark_light::Mode::Light) => Theme::Light,
                    _ => Theme::Light, // Default to light on error or unknown
                }
            }
        }
    }
}

pub fn run(state: Arc<GlobalState>, theme_mode: ThemeMode) -> iced::Result {
    let window_settings = iced::window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        min_size: Some(iced::Size::new(600.0, 400.0)),
        ..Default::default()
    };

    iced::application("odd-box", OddBoxGui::update, OddBoxGui::view)
        .theme(OddBoxGui::theme)
        .window(window_settings)
        .run_with(move || OddBoxGui::new(state, theme_mode))
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
}

pub struct OddBoxGui {
    state: Arc<GlobalState>,
    current_page: Page,
    theme_mode: ThemeMode,
}

impl OddBoxGui {
    fn new(state: Arc<GlobalState>, theme_mode: ThemeMode) -> (Self, Task<Message>) {
        (
            Self {
                state,
                current_page: Page::Dashboard,
                theme_mode,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(page) => {
                self.current_page = page;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let divider = vertical_rule(1);
        let content = self.view_content();

        row![sidebar, divider, content]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let header = container(
            column![
                text("odd-box").size(20).font(iced::Font::MONOSPACE),
                text("reverse proxy").size(11).color(Color::from_rgb(0.5, 0.5, 0.5)),
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

        let nav = Column::with_children(nav_buttons).spacing(4).padding(Padding {
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
                    background: Some(Background::Color(palette.background.weak.color)),
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
            text(page.icon()).size(14).width(Length::Fixed(24.0)),
            text(page.title()).size(13),
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
                        button::Status::Hovered => palette.background.strong.color,
                        _ => Color::TRANSPARENT,
                    }
                };

                let text_color = if is_active {
                    palette.primary.weak.text
                } else {
                    palette.background.base.text
                };

                button::Style {
                    background: Some(Background::Color(background)),
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
        let page_title = text(self.current_page.title()).size(24);

        let page_content: Element<'_, Message> = match self.current_page {
            Page::Dashboard => self.view_dashboard(),
            Page::CrumaIngress => self.view_placeholder("Cruma Ingress configuration"),
            Page::Monitoring => self.view_placeholder("Real-time monitoring and logs"),
            Page::Statistics => self.view_placeholder("Traffic statistics and metrics"),
            Page::Backends => self.view_placeholder("Backend server configuration"),
            Page::Frontends => self.view_placeholder("Frontend routing configuration"),
            Page::ManagedProcesses => self.view_placeholder("Process management"),
        };

        let content = column![page_title, page_content]
            .spacing(20)
            .padding(30)
            .width(Length::Fill);

        Scrollable::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| format!("{:.0?}", d))
            .unwrap_or_else(|_| "Unknown".to_string());

        let status_items = column![
            text("Status: Running").size(14),
            text(format!("Uptime: {}", uptime)).size(14),
        ]
        .spacing(8);

        container(status_items)
            .padding(20)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(palette.background.weak.color)),
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
        container(
            text(description)
                .size(14)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        )
        .padding(20)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
    }

    fn theme(&self) -> Theme {
        self.theme_mode.resolve()
    }
}
