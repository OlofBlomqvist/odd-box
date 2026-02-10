use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, button, column, container, mouse_area, responsive, row, text};
use iced::{Alignment, Border, Color, Element, Length, Padding, Size, Theme};

use crate::global_state::ProcState;

use super::super::{Message, OddBoxGui};

/// Minimum card width for responsive layout
const CARD_MIN_WIDTH: f32 = 240.0;
const CARD_GAP: f32 = 12.0;

/// Category accent colors
const COLOR_PROCESS: Color = Color::from_rgb(0.4, 0.6, 1.0);
const COLOR_REMOTE: Color = Color::from_rgb(0.7, 0.5, 1.0);
const COLOR_DIR_SERVER: Color = Color::from_rgb(0.3, 0.8, 0.7);
const COLOR_FRONTEND: Color = Color::from_rgb(0.35, 0.8, 0.6);

/// Status dot color based on process state
fn status_color(state: &ProcState) -> Color {
    match state {
        ProcState::Running | ProcState::Remote | ProcState::DirServer | ProcState::Docker => {
            Color::from_rgb(0.4, 0.8, 0.4)
        }
        ProcState::Starting | ProcState::Stopping => Color::from_rgb(0.9, 0.7, 0.2),
        ProcState::Stopped => Color::from_rgb(0.6, 0.6, 0.6),
        ProcState::Faulty => Color::from_rgb(0.9, 0.3, 0.3),
    }
}

/// Shared card button style (adds hover border)
fn card_button_style_with_accent(
    theme: &Theme,
    status: button::Status,
    accent: Color,
) -> button::Style {
    let palette = theme.extended_palette();
    let bg = palette.background.weaker.color;
    let (border_color, border_width) = match status {
        button::Status::Hovered | button::Status::Pressed => {
            (accent, 2.0)
        }
        _ => (palette.background.strong.color, 1.0),
    };
    button::Style {
        background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.8).into()),
        text_color: palette.background.base.text,
        border: Border {
            radius: 8.0.into(),
            width: border_width,
            color: border_color,
        },
        ..Default::default()
    }
}

/// Muted text style
fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.extended_palette().background.weak.text),
        ..Default::default()
    }
}

/// Build a site card for remote/static backends
fn site_card<'a>(
    name: &str,
    subtitle: &str,
    category_label: &'a str,
    accent_color: Color,
    state: &ProcState,
    card_width: f32,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let dot_color = status_color(state);

    let name_row = row![
        text("●").color(dot_color).size(14),
        container(
            text(name.to_string())
                .size(15)
                .wrapping(Wrapping::Word)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..iced::Font::MONOSPACE
                })
        )
        .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let subtitle_row = container(
        text(subtitle.to_string())
            .size(13)
            .wrapping(Wrapping::Word)
            .style(muted_text),
    )
    .width(Length::Fill);

    let category_row = row![
        text("▎").color(accent_color).size(16),
        text(category_label)
            .size(12)
            .wrapping(Wrapping::None)
            .style(muted_text),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let card_content = column![name_row, subtitle_row, category_row]
        .spacing(6)
        .width(Length::Fill);

    let mut card = button(card_content)
        .padding(16)
        .width(Length::Fixed(card_width))
        .clip(true)
        .style(move |theme, status| card_button_style_with_accent(theme, status, accent_color))
        .on_press(Message::NoOp);

    if let Some(msg) = on_press {
        card = card.on_press(msg);
    }

    card.into()
}

fn frontend_card<'a>(
    hostname: &str,
    subtitle: &str,
    state: &ProcState,
    card_width: f32,
) -> Element<'a, Message> {
    site_card(
        hostname,
        subtitle,
        "Frontend",
        COLOR_FRONTEND,
        state,
        card_width,
        Some(Message::OpenEditFrontend(hostname.to_string())),
    )
}

/// Build a small stat box for the summary row
fn stat_box<'a>(label: &'a str, value: usize, color: Color) -> Element<'a, Message> {
    let content = column![
        text(value.to_string())
            .size(22)
            .color(color)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            }),
        text(label).size(13).style(muted_text),
    ]
    .spacing(2)
    .align_x(Alignment::Center);

    container(content)
        .padding(Padding {
            top: 10.0,
            right: 20.0,
            bottom: 10.0,
            left: 20.0,
        })
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            let bg = palette.background.weaker.color;
            container::Style {
                background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.6).into()),
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        })
        .into()
}

/// Section header with category name and count badge
fn section_header<'a>(title: &'a str, count: usize, accent: Color) -> Element<'a, Message> {
    let badge = container(
        text(count.to_string())
            .size(13)
            .color(Color::WHITE)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
    )
    .padding(Padding {
        top: 2.0,
        right: 8.0,
        bottom: 2.0,
        left: 8.0,
    })
    .style(move |_theme: &Theme| container::Style {
        background: Some(accent.into()),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    row![
        text(title).size(17).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        badge,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn card_layout(size: Size) -> (usize, f32) {
    let available_width = size.width.max(1.0);
    let cols = ((available_width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP))
        .floor()
        .max(1.0) as usize;
    let card_width = (available_width - CARD_GAP * (cols.saturating_sub(1)) as f32) / cols as f32;
    (cols, card_width)
}

fn rows_from_cards<'a>(mut cards: Vec<Element<'a, Message>>, cols: usize) -> Column<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    while !cards.is_empty() {
        let take = cols.min(cards.len());
        let row_cards: Vec<Element<'a, Message>> = cards.drain(..take).collect();
        rows.push(
            Row::with_children(row_cards)
                .spacing(CARD_GAP)
                .width(Length::Fill)
                .into(),
        );
    }
    Column::with_children(rows).spacing(CARD_GAP)
}

impl OddBoxGui {
    pub(in crate::gui) fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| format!("{:.0?}", d))
            .unwrap_or_else(|_| "Unknown".to_string());

        let dashboard_content: Element<'_, Message> = responsive(move |size| {
            let (cols, card_width) = card_layout(size);

            // --- Uptime bar ---
            let uptime_bar = container(
                row![
                    text("●").color(Color::from_rgb(0.4, 0.8, 0.4)).size(14),
                    text("Status: Running")
                        .size(15)
                        .color(Color::from_rgb(0.4, 0.8, 0.4)),
                    text(format!("  Uptime: {}", &uptime))
                        .size(15)
                        .style(muted_text),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding(14)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weaker.color.into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: palette.background.strong.color,
                    },
                    ..Default::default()
                }
            });

            // --- Summary counts ---
            let total = self.cached_config.routes.len();
            let mut running = 0usize;
            let mut stopped = 0usize;
            let mut faulty = 0usize;

            for route in &self.cached_config.routes {
                let state = self
                    .cached_config
                    .processes
                    .iter()
                    .find(|p| p.name == route.backend)
                    .map(|p| p.state.clone())
                    .or_else(|| {
                        self.cached_config
                            .remote_backends
                            .iter()
                            .find(|b| b.name == route.backend)
                            .map(|b| b.state.clone())
                    })
                    .or_else(|| {
                        self.cached_config
                            .static_backends
                            .iter()
                            .find(|b| b.name == route.backend)
                            .map(|b| b.state.clone())
                    })
                    .unwrap_or(ProcState::Faulty);

                match state {
                    ProcState::Running | ProcState::Remote | ProcState::DirServer | ProcState::Docker => {
                        running += 1;
                    }
                    ProcState::Stopped | ProcState::Starting | ProcState::Stopping => {
                        stopped += 1;
                    }
                    ProcState::Faulty => faulty += 1,
                }
            }

            let summary_row = Row::with_children(vec![
                stat_box("Routes", total, Color::WHITE),
                stat_box("Running", running, Color::from_rgb(0.4, 0.8, 0.4)),
                stat_box("Stopped", stopped, Color::from_rgb(0.6, 0.6, 0.6)),
                stat_box("Faulty", faulty, Color::from_rgb(0.9, 0.3, 0.3)),
            ])
            .spacing(12);

            // --- Category sections ---
            let mut sections: Vec<Element<'_, Message>> = Vec::new();

            if !self.cached_config.routes.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .routes
                    .iter()
                    .map(|route| {
                        if let Some(proc_backend) = self
                            .cached_config
                            .processes
                            .iter()
                            .find(|p| p.name == route.backend)
                        {
                            let subtitle =
                                format!("Process · {} · :{}", proc_backend.protocol, proc_backend.port);
                            frontend_card(&route.hostname, &subtitle, &proc_backend.state, card_width)
                        } else if let Some(remote) = self
                            .cached_config
                            .remote_backends
                            .iter()
                            .find(|r| r.name == route.backend)
                        {
                            let subtitle = format!("Remote · {}", remote.endpoints);
                            frontend_card(&route.hostname, &subtitle, &remote.state, card_width)
                        } else if let Some(dir) = self
                            .cached_config
                            .static_backends
                            .iter()
                            .find(|s| s.name == route.backend)
                        {
                            let subtitle = format!("Static · {}", dir.dir);
                            frontend_card(&route.hostname, &subtitle, &dir.state, card_width)
                        } else {
                            let subtitle = format!("Missing backend · {}", route.backend);
                            frontend_card(&route.hostname, &subtitle, &ProcState::Faulty, card_width)
                        }
                    })
                    .collect();

                sections.push(
                    column![
                        section_header("Frontends", self.cached_config.routes.len(), COLOR_FRONTEND),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Assemble the dashboard content
            let mut content_children: Vec<Element<'_, Message>> =
                vec![uptime_bar.into(), summary_row.into()];
            content_children.extend(sections);

            let dashboard_column = Column::with_children(content_children).spacing(20);

            mouse_area(dashboard_column)
                .on_move(|p| Message::DashboardCursorMoved(p.x, p.y))
                .into()
        })
        .into();
        dashboard_content
    }
}
