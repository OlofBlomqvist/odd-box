use iced::widget::{Column, Scrollable, button, column, container, row, text, toggler};
use iced::{Color, Element, Font, Length, Padding, Theme};
use iced::widget::text::Wrapping;

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_traffic_inspection(&self) -> Element<'_, Message> {
        let enabled = self
            .state
            .enable_global_traffic_inspection
            .load(std::sync::atomic::Ordering::Relaxed);

        let capture_store = &self.state.http_capture_store;
        let snapshot = capture_store.snapshot();

        // ── Controls row ─────────────────────────────────────────────────
        let toggle = toggler(enabled)
            .label(if enabled {
                "Capture enabled"
            } else {
                "Capture disabled"
            })
            .on_toggle(Message::TrafficInspectionToggled)
            .text_size(super::super::text_size(13));

        let clear_btn = button(text("Clear").size(super::super::text_size(13)))
            .padding(Padding {
                top: 6.0,
                right: 12.0,
                bottom: 6.0,
                left: 12.0,
            })
            .style(|theme: &Theme, status| {
                use iced::widget::button;
                let palette = theme.extended_palette();
                let bg = match status {
                    button::Status::Hovered => palette.danger.strong.color,
                    button::Status::Disabled => palette.background.weak.color,
                    _ => palette.danger.weak.color,
                };
                let fg = match status {
                    button::Status::Disabled => palette.background.weak.text,
                    _ => Color::WHITE,
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: fg,
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: palette.danger.strong.color,
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::TrafficInspectionClear);

        let count_label = text(format!("{} captured exchanges", snapshot.order.len()))
            .font(Font::MONOSPACE)
            .size(super::super::text_size(12));

        let controls_row = row![toggle, count_label, clear_btn]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        let controls_box = container(controls_row)
            .padding(16)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                iced::widget::container::Style {
                    background: Some(palette.background.weaker.color.into()),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: palette.background.strong.color,
                    },
                    ..Default::default()
                }
            });

        // ── Header row ───────────────────────────────────────────────────
        let header = container(
            row![
                container(
                    text("Method")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(60.0)),
                container(
                    text("Status")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(50.0)),
                container(
                    text("Kind")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(40.0)),
                container(
                    text("Duration")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(80.0)),
                container(
                    text("Size")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(100.0)),
                container(
                    text("Host")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::FillPortion(3)),
                container(
                    text("Path")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::FillPortion(5)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding(Padding {
            top: 8.0,
            right: 12.0,
            bottom: 8.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(palette.background.strong.color.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });

        // ── Exchange rows ────────────────────────────────────────────────
        let mut exchange_rows = Column::new().spacing(0);

        if snapshot.order.is_empty() {
            let placeholder_msg = if enabled {
                "No captured requests yet. Requests will appear here as they flow through the proxy."
            } else {
                "Traffic inspection is disabled. Enable capture above to start recording requests."
            };
            exchange_rows = exchange_rows.push(
                container(
                    text(placeholder_msg.to_string())
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(12))
                        .wrapping(Wrapping::WordOrGlyph),
                )
                .padding(16)
                .width(Length::Fill),
            );
        } else {
            // Iterate newest-first
            for req_id in snapshot.order.iter().rev() {
                if let Some(exchange) = snapshot.entries.get(req_id) {
                    let method_color = match exchange.method.as_str() {
                        "GET" => Color::from_rgb(0.2, 0.7, 0.3),
                        "POST" => Color::from_rgb(0.9, 0.7, 0.1),
                        "PUT" => Color::from_rgb(0.3, 0.5, 0.9),
                        "DELETE" => Color::from_rgb(0.9, 0.3, 0.3),
                        "PATCH" => Color::from_rgb(0.7, 0.3, 0.8),
                        "HEAD" => Color::from_rgb(0.5, 0.5, 0.5),
                        "OPTIONS" => Color::from_rgb(0.4, 0.7, 0.7),
                        _ => Color::from_rgb(0.6, 0.6, 0.6),
                    };

                    let status_str = match exchange.status {
                        Some(s) => format!("{}", s),
                        None if exchange.is_inflight => "···".to_string(),
                        None => "—".to_string(),
                    };
                    let status_color = match exchange.status {
                        Some(s) if s < 300 => Color::from_rgb(0.2, 0.7, 0.3),
                        Some(s) if s < 400 => Color::from_rgb(0.3, 0.7, 0.8),
                        Some(s) if s < 500 => Color::from_rgb(0.9, 0.7, 0.1),
                        Some(_) => Color::from_rgb(0.9, 0.3, 0.3),
                        None => Color::from_rgb(0.5, 0.5, 0.5),
                    };

                    let duration_str = match exchange.duration_ms {
                        Some(d) if d < 1 => "<1ms".to_string(),
                        Some(d) if d < 1000 => format!("{}ms", d),
                        Some(d) if d < 60_000 => format!("{:.1}s", d as f64 / 1000.0),
                        Some(d) => format!("{:.0}m{:.0}s", d / 60_000, (d % 60_000) as f64 / 1000.0),
                        None if exchange.is_inflight => "···".to_string(),
                        None => "—".to_string(),
                    };

                    let size_str = {
                        let req_sz = exchange.req_body_size.map(|s| fmt_body_size(s)).unwrap_or_default();
                        let resp_sz = exchange.resp_body_size.map(|s| fmt_body_size(s)).unwrap_or_default();
                        let req_trunc = if exchange.req_body_truncated { "+" } else { "" };
                        let resp_trunc = if exchange.resp_body_truncated { "+" } else { "" };
                        if req_sz.is_empty() && resp_sz.is_empty() {
                            if exchange.is_inflight { "···".to_string() } else { "—".to_string() }
                        } else {
                            format!("{}{}↑ {}{}↓", req_sz, req_trunc, resp_sz, resp_trunc)
                        }
                    };

                    let host_str = exchange.host.clone().unwrap_or_default();
                    let kind_str = format!("{}", exchange.kind);
                    let path_str = exchange.path.clone();

                    let inflight_suffix = if exchange.is_inflight { " ●" } else { "" };
                    let method_display = format!("{}{}", &exchange.method, inflight_suffix);

                    let exchange_row = container(
                        row![
                            container(
                                text(method_display)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(12))
                                    .color(method_color)
                            )
                            .width(Length::Fixed(60.0)),
                            container(
                                text(status_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(12))
                                    .color(status_color)
                            )
                            .width(Length::Fixed(50.0)),
                            container(
                                text(kind_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(11))
                            )
                            .width(Length::Fixed(40.0)),
                            container(
                                text(duration_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(11))
                            )
                            .width(Length::Fixed(80.0)),
                            container(
                                text(size_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(11))
                            )
                            .width(Length::Fixed(100.0)),
                            container(
                                text(host_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(11))
                                    .wrapping(Wrapping::None)
                            )
                            .width(Length::FillPortion(3)),
                            container(
                                text(path_str)
                                    .font(Font::MONOSPACE)
                                    .size(super::super::text_size(11))
                                    .wrapping(Wrapping::None)
                            )
                            .width(Length::FillPortion(5)),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding(Padding {
                        top: 4.0,
                        right: 12.0,
                        bottom: 4.0,
                        left: 12.0,
                    })
                    .width(Length::Fill);

                    exchange_rows = exchange_rows.push(exchange_row);

                    // Separator line
                    exchange_rows = exchange_rows.push(
                        container(text(""))
                            .height(Length::Fixed(1.0))
                            .width(Length::Fill)
                            .style(|theme: &Theme| {
                                let palette = theme.extended_palette();
                                iced::widget::container::Style {
                                    background: Some(
                                        palette.background.strong.color.into(),
                                    ),
                                    ..Default::default()
                                }
                            }),
                    );
                }
            }
        }

        let exchange_list = container(
            Scrollable::new(exchange_rows)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(palette.background.weaker.color.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });

        column![controls_box, header, exchange_list]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn fmt_body_size(n: usize) -> String {
    if n == 0 {
        "0B".to_string()
    } else if n < 1024 {
        format!("{}B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{:.1}M", n as f64 / (1024.0 * 1024.0))
    }
}