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
        button::Status::Hovered | button::Status::Pressed => (accent, 2.0),
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

/// Build a site card
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

            // Route helpers
            let route_count_for = |backend_name: &str| -> usize {
                self.cached_config
                    .routes
                    .iter()
                    .filter(|r| r.backend == backend_name)
                    .count()
            };
            let route_hostname_for = |backend_name: &str| -> Option<&str> {
                self.cached_config
                    .routes
                    .iter()
                    .find(|r| r.backend == backend_name)
                    .map(|r| r.hostname.as_str())
            };
            let backend_exists = |name: &str| -> bool {
                self.cached_config.processes.iter().any(|p| p.name == name)
                    || self
                        .cached_config
                        .remote_backends
                        .iter()
                        .any(|r| r.name == name)
                    || self
                        .cached_config
                        .static_backends
                        .iter()
                        .any(|s| s.name == name)
            };

            // Summary counts (backend-centric)
            let mut total = 0usize;
            let mut running = 0usize;
            let mut stopped = 0usize;
            let mut faulty = 0usize;

            for proc in &self.cached_config.processes {
                total += 1;
                match proc.state {
                    ProcState::Running
                    | ProcState::Remote
                    | ProcState::DirServer
                    | ProcState::Docker => running += 1,
                    ProcState::Stopped | ProcState::Starting | ProcState::Stopping => stopped += 1,
                    ProcState::Faulty => faulty += 1,
                }
            }
            for remote in &self.cached_config.remote_backends {
                total += 1;
                match remote.state {
                    ProcState::Running
                    | ProcState::Remote
                    | ProcState::DirServer
                    | ProcState::Docker => running += 1,
                    ProcState::Stopped | ProcState::Starting | ProcState::Stopping => stopped += 1,
                    ProcState::Faulty => faulty += 1,
                }
            }
            for sb in &self.cached_config.static_backends {
                total += 1;
                match sb.state {
                    ProcState::Running
                    | ProcState::Remote
                    | ProcState::DirServer
                    | ProcState::Docker => running += 1,
                    ProcState::Stopped | ProcState::Starting | ProcState::Stopping => stopped += 1,
                    ProcState::Faulty => faulty += 1,
                }
            }
            let orphan_count = self
                .cached_config
                .routes
                .iter()
                .filter(|r| !backend_exists(&r.backend))
                .count();
            total += orphan_count;
            faulty += orphan_count;

            let summary_row = Row::with_children(vec![
                stat_box("Sites", total, Color::WHITE),
                stat_box("Running", running, Color::from_rgb(0.4, 0.8, 0.4)),
                stat_box("Stopped", stopped, Color::from_rgb(0.6, 0.6, 0.6)),
                stat_box("Faulty", faulty, Color::from_rgb(0.9, 0.3, 0.3)),
            ])
            .spacing(12);

            let mut sections: Vec<Element<'_, Message>> = Vec::new();
            let color_muted = Color::from_rgb(0.5, 0.5, 0.5);

            // Process backends
            if !self.cached_config.processes.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .processes
                    .iter()
                    .map(|proc| {
                        let rc = route_count_for(&proc.name);
                        let (card_name, subtitle) = match rc {
                            1 => (
                                route_hostname_for(&proc.name).unwrap_or(&proc.name),
                                format!(
                                    "backend: {} · {} · :{}",
                                    proc.name, proc.protocol, proc.port
                                ),
                            ),
                            0 => (
                                proc.name.as_str(),
                                format!("{} · :{} · no frontend", proc.protocol, proc.port),
                            ),
                            n => (
                                proc.name.as_str(),
                                format!("{} · :{} · {} routes", proc.protocol, proc.port, n),
                            ),
                        };
                        let accent = if rc == 0 { color_muted } else { COLOR_PROCESS };
                        site_card(
                            card_name,
                            &subtitle,
                            "Process",
                            accent,
                            &proc.state,
                            card_width,
                            Some(Message::OpenEditBackend(proc.name.clone())),
                        )
                    })
                    .collect();
                sections.push(
                    column![
                        section_header("Processes", cards.len(), COLOR_PROCESS),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Remote backends
            if !self.cached_config.remote_backends.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .remote_backends
                    .iter()
                    .map(|remote| {
                        let rc = route_count_for(&remote.name);
                        let (card_name, subtitle) = match rc {
                            1 => (
                                route_hostname_for(&remote.name).unwrap_or(&remote.name),
                                format!("backend: {} · {}", remote.name, remote.endpoints),
                            ),
                            0 => (
                                remote.name.as_str(),
                                format!("{} · no frontend", remote.endpoints),
                            ),
                            n => (
                                remote.name.as_str(),
                                format!("{} · {} routes", remote.endpoints, n),
                            ),
                        };
                        let accent = if rc == 0 { color_muted } else { COLOR_REMOTE };
                        site_card(
                            card_name,
                            &subtitle,
                            "Remote",
                            accent,
                            &remote.state,
                            card_width,
                            Some(Message::OpenEditBackend(remote.name.clone())),
                        )
                    })
                    .collect();
                sections.push(
                    column![
                        section_header("Remote Backends", cards.len(), COLOR_REMOTE),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Static backends
            if !self.cached_config.static_backends.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .static_backends
                    .iter()
                    .map(|sb| {
                        let rc = route_count_for(&sb.name);
                        let (card_name, subtitle) = match rc {
                            1 => (
                                route_hostname_for(&sb.name).unwrap_or(&sb.name),
                                format!("backend: {} · {}", sb.name, sb.dir),
                            ),
                            0 => (
                                sb.name.as_str(),
                                format!("{} · no frontend", sb.dir),
                            ),
                            n => (
                                sb.name.as_str(),
                                format!("{} · {} routes", sb.dir, n),
                            ),
                        };
                        let accent = if rc == 0 { color_muted } else { COLOR_DIR_SERVER };
                        site_card(
                            card_name,
                            &subtitle,
                            "Static",
                            accent,
                            &sb.state,
                            card_width,
                            Some(Message::OpenEditBackend(sb.name.clone())),
                        )
                    })
                    .collect();
                sections.push(
                    column![
                        section_header("Static Backends", cards.len(), COLOR_DIR_SERVER),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Orphan routes (pointing to missing backends)
            let orphan_cards: Vec<Element<'_, Message>> = self
                .cached_config
                .routes
                .iter()
                .filter(|r| !backend_exists(&r.backend))
                .map(|route| {
                    site_card(
                        &route.hostname,
                        &format!("Missing backend: {}", route.backend),
                        "Faulty",
                        Color::from_rgb(0.9, 0.3, 0.3),
                        &ProcState::Faulty,
                        card_width,
                        Some(Message::OpenEditFrontend(route.hostname.clone())),
                    )
                })
                .collect();
            if !orphan_cards.is_empty() {
                let count = orphan_cards.len();
                sections.push(
                    column![
                        section_header(
                            "Faulty Routes",
                            count,
                            Color::from_rgb(0.9, 0.3, 0.3)
                        ),
                        rows_from_cards(orphan_cards, cols),
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
