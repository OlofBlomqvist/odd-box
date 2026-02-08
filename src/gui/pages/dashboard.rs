use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Stack, button, column, container, mouse_area, responsive, row, text};
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

/// Shared card container style
fn card_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let bg = palette.background.weaker.color;
    container::Style {
        background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.8).into()),
        border: Border {
            radius: 8.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
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
) -> Element<'a, Message> {
    let dot_color = status_color(state);

    let name_row = row![
        text("●").color(dot_color).size(14),
        text(name.to_string())
            .size(15)
            .wrapping(Wrapping::None)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            }),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let subtitle_row = text(subtitle.to_string())
        .size(13)
        .wrapping(Wrapping::None)
        .style(muted_text);

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

    container(card_content)
        .padding(16)
        .width(Length::Fixed(card_width))
        .clip(true)
        .style(card_style)
        .into()
}

/// Build a process card — clickable for context menu
fn process_card<'a>(
    name: &str,
    protocol: &str,
    port: &str,
    bin: &str,
    state: &ProcState,
    card_width: f32,
) -> Element<'a, Message> {
    let dot_color = status_color(state);
    let proc_name = name.to_string();

    let name_row = row![
        text("●").color(dot_color).size(14),
        text(name.to_string())
            .size(15)
            .wrapping(Wrapping::None)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            }),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let subtitle_row = text(format!("{} · :{}", protocol, port))
        .size(13)
        .wrapping(Wrapping::None)
        .style(muted_text);

    let bin_row = text(bin.to_string())
        .size(11)
        .wrapping(Wrapping::Word)
        .style(muted_text);

    let category_row = row![
        text("▎").color(COLOR_PROCESS).size(16),
        text("Process")
            .size(12)
            .wrapping(Wrapping::None)
            .style(muted_text),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let card_content = column![name_row, subtitle_row, bin_row, category_row]
        .spacing(4)
        .width(Length::Fill);

    let card = container(card_content)
        .padding(16)
        .width(Length::Fixed(card_width))
        .clip(true)
        .style(card_style);

    mouse_area(card)
        .on_press(Message::DashboardToggleProcessMenu(proc_name))
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

/// Build the popup context menu for a process
fn process_popup_menu<'a>(
    name: &str,
    state: &ProcState,
) -> Element<'a, Message> {
    let proc_name = name.to_string();
    let dot_color = status_color(state);

    let header = row![
        text("●").color(dot_color).size(16),
        text(name.to_string())
            .size(16)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            }),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let can_start = matches!(state, ProcState::Stopped | ProcState::Faulty);
    let can_stop = matches!(state, ProcState::Running);

    let menu_btn_style = |hover_color: Color| {
        move |theme: &Theme, status: button::Status| {
            let palette = theme.extended_palette();
            let (bg, text_color) = match status {
                button::Status::Hovered => (hover_color, Color::WHITE),
                button::Status::Disabled => {
                    let c = palette.background.strong.color;
                    (
                        Color::from_rgba(c.r, c.g, c.b, 0.4),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                    )
                }
                _ => (
                    palette.background.strong.color,
                    palette.background.strong.text,
                ),
            };
            button::Style {
                background: Some(bg.into()),
                text_color,
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    };

    let btn_padding = Padding {
        top: 8.0,
        right: 16.0,
        bottom: 8.0,
        left: 16.0,
    };

    let mut start_btn = button(text("▶  Start").size(14))
        .padding(btn_padding)
        .width(Length::Fill)
        .style(menu_btn_style(Color::from_rgb(0.3, 0.7, 0.3)));
    if can_start {
        start_btn = start_btn.on_press(Message::ProcessStart(proc_name.clone()));
    }

    let mut stop_btn = button(text("■  Stop").size(14))
        .padding(btn_padding)
        .width(Length::Fill)
        .style(menu_btn_style(Color::from_rgb(0.8, 0.3, 0.3)));
    if can_stop {
        stop_btn = stop_btn.on_press(Message::ProcessStop(proc_name.clone()));
    }

    let edit_frontend_btn = button(text("✎  Edit Frontend").size(14))
        .padding(btn_padding)
        .width(Length::Fill)
        .style(menu_btn_style(Color::from_rgb(0.35, 0.55, 0.9)))
        .on_press(Message::OpenEditFrontend(proc_name.clone()));

    let edit_backend_btn = button(text("✎  Edit Backend").size(14))
        .padding(btn_padding)
        .width(Length::Fill)
        .style(menu_btn_style(Color::from_rgb(0.45, 0.6, 0.85)))
        .on_press(Message::OpenEditBackend(proc_name));

    let actions = column![start_btn, stop_btn, edit_frontend_btn, edit_backend_btn].spacing(4);

    let menu_content = column![header, actions].spacing(12);

    container(menu_content)
        .padding(16)
        .width(Length::Fixed(240.0))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.base.color.into()),
                border: Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        })
        .into()
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
        text(title)
            .size(17)
            .font(iced::Font {
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
    let card_width =
        (available_width - CARD_GAP * (cols.saturating_sub(1)) as f32) / cols as f32;
    (cols, card_width)
}

fn rows_from_cards<'a>(
    mut cards: Vec<Element<'a, Message>>,
    cols: usize,
) -> Column<'a, Message> {
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
            let total = self.cached_config.processes.len()
                + self.cached_config.remote_backends.len()
                + self.cached_config.static_backends.len();

            let running = self
                .cached_config
                .processes
                .iter()
                .filter(|p| matches!(p.state, ProcState::Running))
                .count()
                + self
                    .cached_config
                    .remote_backends
                    .iter()
                    .filter(|r| matches!(r.state, ProcState::Running | ProcState::Remote))
                    .count()
                + self
                    .cached_config
                    .static_backends
                    .iter()
                    .filter(|s| matches!(s.state, ProcState::Running | ProcState::DirServer))
                    .count();

            let stopped = self
                .cached_config
                .processes
                .iter()
                .filter(|p| matches!(p.state, ProcState::Stopped))
                .count();

            let faulty = self
                .cached_config
                .processes
                .iter()
                .filter(|p| matches!(p.state, ProcState::Faulty))
                .count();

            let summary_row = Row::with_children(vec![
                stat_box("Total", total, Color::WHITE),
                stat_box("Running", running, Color::from_rgb(0.4, 0.8, 0.4)),
                stat_box("Stopped", stopped, Color::from_rgb(0.6, 0.6, 0.6)),
                stat_box("Faulty", faulty, Color::from_rgb(0.9, 0.3, 0.3)),
            ])
            .spacing(12);

            // --- Category sections ---
            let mut sections: Vec<Element<'_, Message>> = Vec::new();

            // Managed Processes
            if !self.cached_config.processes.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .processes
                    .iter()
                    .map(|p| {
                        process_card(
                            &p.name,
                            &p.protocol,
                            &p.port,
                            &p.bin,
                            &p.state,
                            card_width,
                        )
                    })
                    .collect();

                sections.push(
                    column![
                        section_header(
                            "Managed Processes",
                            self.cached_config.processes.len(),
                            COLOR_PROCESS,
                        ),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Remote Backends
            if !self.cached_config.remote_backends.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .remote_backends
                    .iter()
                    .map(|r| {
                        let subtitle = format!("{} · {}", r.protocol, r.endpoints);
                        site_card(&r.name, &subtitle, "Remote", COLOR_REMOTE, &r.state, card_width)
                    })
                    .collect();

                sections.push(
                    column![
                        section_header(
                            "Remote Backends",
                            self.cached_config.remote_backends.len(),
                            COLOR_REMOTE,
                        ),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Static File Servers
            if !self.cached_config.static_backends.is_empty() {
                let cards: Vec<Element<'_, Message>> = self
                    .cached_config
                    .static_backends
                    .iter()
                    .map(|s| {
                        site_card(&s.name, &s.dir, "Dir Server", COLOR_DIR_SERVER, &s.state, card_width)
                    })
                    .collect();

                sections.push(
                    column![
                        section_header(
                            "Static File Servers",
                            self.cached_config.static_backends.len(),
                            COLOR_DIR_SERVER,
                        ),
                        rows_from_cards(cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // Assemble the dashboard content
            let mut content_children: Vec<Element<'_, Message>> = vec![
                uptime_bar.into(),
                summary_row.into(),
            ];
            content_children.extend(sections);

            let dashboard_column = Column::with_children(content_children).spacing(20);

            mouse_area(dashboard_column)
                .on_move(|p| Message::DashboardCursorMoved(p.x, p.y))
                .into()
        })
        .into();

        // --- Popup menu overlay ---
        let selected_proc = self.dashboard_process_menu.as_ref().and_then(|name| {
            self.cached_config
                .processes
                .iter()
                .find(|p| &p.name == name)
        });

        if let Some(proc) = selected_proc {
            let menu = process_popup_menu(&proc.name, &proc.state);

            // Invisible dismiss layer — clicking anywhere outside the menu closes it
            let dismiss_name = proc.name.clone();
            let dismiss = mouse_area(
                container(column![])
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::DashboardToggleProcessMenu(dismiss_name));

            // Position the menu at the stored cursor location
            let (mx, my) = self.dashboard_menu_pos;
            let positioned_menu = container(menu)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Start)
                .align_y(Alignment::Start)
                .padding(Padding {
                    top: my,
                    left: mx,
                    bottom: 0.0,
                    right: 0.0,
                });

            Stack::with_children(vec![
                dashboard_content,
                dismiss.into(),
                positioned_menu.into(),
            ])
            .width(Length::Fill)
            .into()
        } else {
            dashboard_content
        }
    }
}
