use iced::widget::{Column, Row, Scrollable, button, column, container, row, text, toggler};
use iced::{Background, Color, Element, Font, Length, Padding, Theme};
use iced::widget::text::Wrapping;

use super::super::{Message, OddBoxGui};

use cruma_proxy_lib::proxying::HttpRequestKind;
use cruma_proxy_lib::proxying::capture_store::{CapturedExchange, HttpCaptureStore};

use flate2::read::{DeflateDecoder, GzDecoder};
use std::io::Read;
use std::sync::Arc;

impl OddBoxGui {
    pub(in crate::gui) fn view_traffic_inspection(&self) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();
        let page_title = text("Traffic Inspection").size(super::super::text_size(20));

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
            .style(move |theme: &Theme, status| {
                use iced::widget::button;
                if use_kde_buttons {
                    return super::super::kde_danger_button_style(theme, status);
                }
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

        let mut controls_items = row![toggle, count_label, clear_btn]
            .spacing(16)
            .align_y(iced::Alignment::Center);

        // Show a close-detail button when a row is selected
        if self.traffic_inspection_selected.is_some() {
            let close_btn = button(text("✕ Close Detail").size(super::super::text_size(12)))
                .padding(Padding {
                    top: 6.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                })
                .style(move |theme: &Theme, status| {
                    use iced::widget::button;
                    if use_kde_buttons {
                        return super::super::kde_primary_button_style(theme, status);
                    }
                    let palette = theme.extended_palette();
                    let bg = match status {
                        button::Status::Hovered => palette.primary.strong.color,
                        _ => palette.background.strong.color,
                    };
                    button::Style {
                        background: Some(bg.into()),
                        text_color: palette.background.base.text,
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: palette.background.strong.color,
                        },
                        ..Default::default()
                    }
                })
                .on_press(Message::TrafficInspectionSelect(None));
            controls_items = controls_items.push(close_btn);
        }

        let controls_box = container(controls_items)
            .padding(16)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                iced::widget::container::Style {
                    background: Some(self.surface_panel_bg(theme).into()),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
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
            iced::widget::container::Style {
                background: Some(self.surface_panel_alt_bg(theme).into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
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
            // Iterate oldest-first
            for req_id in snapshot.order.iter() {
                if let Some(exchange) = snapshot.entries.get(req_id) {
                    let is_selected = self.traffic_inspection_selected == Some(*req_id);
                    let rid = *req_id;
                    let row_element = build_exchange_row(exchange, is_selected);

                    let row_btn = button(row_element)
                        .padding(0)
                        .width(Length::Fill)
                        .style(move |theme: &Theme, status| {
                            use iced::widget::button;
                            let palette = theme.extended_palette();
                            let bg = if is_selected {
                                palette.primary.weak.color
                            } else {
                                match status {
                                    button::Status::Hovered => {
                                        let mut c = palette.primary.weak.color;
                                        c.a = 0.15;
                                        c
                                    }
                                    _ => Color::TRANSPARENT,
                                }
                            };
                            button::Style {
                                background: Some(bg.into()),
                                text_color: palette.background.base.text,
                                border: iced::Border::default(),
                                ..Default::default()
                            }
                        })
                        .on_press(Message::TrafficInspectionSelect(Some(rid)));

                    exchange_rows = exchange_rows.push(row_btn);

                    // Separator line
                    exchange_rows = exchange_rows.push(
                        container(text(""))
                            .height(Length::Fixed(1.0))
                            .width(Length::Fill)
                            .style(|theme: &Theme| {
                                iced::widget::container::Style {
                                    background: Some(self.surface_border_color(theme).into()),
                                    ..Default::default()
                                }
                            }),
                    );
                }
            }
        }

        // Determine if we have a selected exchange to show the detail panel
        let has_detail = self.traffic_inspection_selected.is_some()
            && self
                .traffic_inspection_selected
                .and_then(|id| snapshot.entries.get(&id))
                .is_some();

        if has_detail {
            // Split layout: list on top (40%), detail panel on bottom (60%)
            let exchange_list = container(
                Scrollable::new(exchange_rows)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::FillPortion(2))
            .style(|theme: &Theme| {
                iced::widget::container::Style {
                    background: Some(self.surface_panel_bg(theme).into()),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
                    },
                    ..Default::default()
                }
            });

            let selected_id = self.traffic_inspection_selected.unwrap();
            let exchange = snapshot.entries.get(&selected_id).unwrap();

            let detail_panel = build_detail_panel(exchange, capture_store);

            let detail_container = container(
                Scrollable::new(detail_panel)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::FillPortion(3))
            .style(|theme: &Theme| {
                iced::widget::container::Style {
                    background: Some(self.surface_panel_bg(theme).into()),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
                    },
                    ..Default::default()
                }
            });

            let inner = column![page_title, controls_box, header, exchange_list, detail_container]
                .spacing(4)
                .padding(30)
                .width(Length::Fill)
                .height(Length::Fill);

            container(inner)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    let bg = self.surface_page_bg(theme);
                    let alpha = if super::super::use_glass_effects() {
                        if palette.is_dark { 0.32 } else { 0.22 }
                    } else {
                        1.0
                    };
                    iced::widget::container::Style {
                        background: Some(Background::Color(super::super::platform_surface_color(
                            bg, alpha,
                        ))),
                        ..Default::default()
                    }
                })
                .into()
        } else {
            // No detail selected: full-height list
            let exchange_list = container(
                Scrollable::new(exchange_rows)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| {
                iced::widget::container::Style {
                    background: Some(self.surface_panel_bg(theme).into()),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
                    },
                    ..Default::default()
                }
            });

            let inner = column![page_title, controls_box, header, exchange_list]
                .spacing(4)
                .padding(30)
                .width(Length::Fill)
                .height(Length::Fill);

            container(inner)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    let bg = self.surface_page_bg(theme);
                    let alpha = if super::super::use_glass_effects() {
                        if palette.is_dark { 0.32 } else { 0.22 }
                    } else {
                        1.0
                    };
                    iced::widget::container::Style {
                        background: Some(Background::Color(super::super::platform_surface_color(
                            bg, alpha,
                        ))),
                        ..Default::default()
                    }
                })
                .into()
        }
    }
}

// ─── Exchange row (table row) ────────────────────────────────────────────────

fn build_exchange_row<'a>(
    exchange: &CapturedExchange,
    _is_selected: bool,
) -> Element<'a, Message> {
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
        Some(d) => format!(
            "{:.0}m{:.0}s",
            d / 60_000,
            (d % 60_000) as f64 / 1000.0
        ),
        None if exchange.is_inflight => "···".to_string(),
        None => "—".to_string(),
    };

    let size_str = {
        let req_sz = exchange
            .req_body_size
            .map(|s| fmt_body_size(s))
            .unwrap_or_default();
        let resp_sz = exchange
            .resp_body_size
            .map(|s| fmt_body_size(s))
            .unwrap_or_default();
        let req_trunc = if exchange.req_body_truncated {
            "+"
        } else {
            ""
        };
        let resp_trunc = if exchange.resp_body_truncated {
            "+"
        } else {
            ""
        };
        if req_sz.is_empty() && resp_sz.is_empty() {
            if exchange.is_inflight {
                "···".to_string()
            } else {
                "—".to_string()
            }
        } else {
            format!("{}{}↑ {}{}↓", req_sz, req_trunc, resp_sz, resp_trunc)
        }
    };

    let host_str = exchange.host.clone().unwrap_or_default();
    let kind_str = format!("{}", exchange.kind);
    let path_str = exchange.path.clone();

    let inflight_suffix = if exchange.is_inflight { " ●" } else { "" };
    let method_display = format!("{}{}", &exchange.method, inflight_suffix);

    container(
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
    .width(Length::Fill)
    .into()
}

// ─── Detail panel ────────────────────────────────────────────────────────────

fn build_detail_panel<'a>(
    exchange: &CapturedExchange,
    capture_store: &Arc<HttpCaptureStore>,
) -> Element<'a, Message> {
    let ts = super::super::text_size;

    let mut detail_col = Column::new().spacing(4).padding(16).width(Length::Fill);

    // ── Title bar ────────────────────────────────────────────────────
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

    let status_color = match exchange.status {
        Some(s) if s < 300 => Color::from_rgb(0.2, 0.7, 0.3),
        Some(s) if s < 400 => Color::from_rgb(0.3, 0.7, 0.8),
        Some(s) if s < 500 => Color::from_rgb(0.9, 0.7, 0.1),
        Some(_) => Color::from_rgb(0.9, 0.3, 0.3),
        None => Color::from_rgb(0.5, 0.5, 0.5),
    };

    let kind_badge = kind_badge_str(&exchange.kind);

    // Request summary line
    let mut title_items: Vec<Element<'_, Message>> = Vec::new();

    // Kind badge
    if exchange.kind != HttpRequestKind::Regular {
        let (badge_text, badge_color) = kind_badge;
        title_items.push(
            container(
                text(format!(" {} ", badge_text))
                    .font(Font::MONOSPACE)
                    .size(ts(11))
                    .color(Color::WHITE),
            )
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(badge_color.into()),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into(),
        );
    }

    // HTTP version
    if let Some(ref ver) = exchange.http_version {
        title_items.push(
            text(format!(" {} ", ver))
                .font(Font::MONOSPACE)
                .size(ts(11))
                .color(Color::from_rgb(0.5, 0.5, 0.5))
                .into(),
        );
    }

    // Method
    title_items.push(
        text(format!(" {} ", &exchange.method))
            .font(Font::MONOSPACE)
            .size(ts(14))
            .color(method_color)
            .into(),
    );

    // Host + path
    let host = exchange.host.clone().unwrap_or_else(|| "<no-host>".into());
    title_items.push(
        text(format!("{}{}", host, &exchange.path))
            .font(Font::MONOSPACE)
            .size(ts(13))
            .wrapping(Wrapping::WordOrGlyph)
            .into(),
    );

    let title_row = Row::with_children(title_items)
        .spacing(4)
        .align_y(iced::Alignment::Center);

    detail_col = detail_col.push(title_row);

    // ── Response summary ─────────────────────────────────────────────
    let is_streaming = exchange.is_inflight
        && matches!(
            exchange.kind,
            HttpRequestKind::SSE | HttpRequestKind::WebSocket
        );

    let response_summary = if is_streaming {
        let stream_label = match exchange.kind {
            HttpRequestKind::SSE => "⇣ streaming (SSE)",
            HttpRequestKind::WebSocket => "⇅ open (WS)",
            _ => "⇣ streaming",
        };
        let mut parts: Vec<Element<'_, Message>> = vec![text(stream_label)
            .font(Font::MONOSPACE)
            .size(ts(12))
            .color(Color::from_rgb(0.2, 0.8, 0.3))
            .into()];
        if let Some(status) = exchange.status {
            parts.push(
                text(format!("  Status: {}", status))
                    .font(Font::MONOSPACE)
                    .size(ts(12))
                    .color(status_color)
                    .into(),
            );
        }
        Row::with_children(parts)
            .spacing(8)
            .align_y(iced::Alignment::Center)
    } else {
        let status_str = match exchange.status {
            Some(s) => format!("{}", s),
            None => "pending...".to_string(),
        };
        let duration_str = match exchange.duration_ms {
            Some(d) if d < 1 => "<1 ms".to_string(),
            Some(d) if d < 1000 => format!("{} ms", d),
            Some(d) if d < 60_000 => format!("{:.1} s", d as f64 / 1000.0),
            Some(d) => format!(
                "{:.0}m {:.0}s",
                d / 60_000,
                (d % 60_000) as f64 / 1000.0
            ),
            None => "—".to_string(),
        };
        row![
            text(format!("Status: {}", status_str))
                .font(Font::MONOSPACE)
                .size(ts(12))
                .color(status_color),
            text(format!("  Duration: {}", duration_str))
                .font(Font::MONOSPACE)
                .size(ts(12))
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
    };

    // Client address
    if let Some(ref addr) = exchange.client_addr {
        detail_col = detail_col.push(
            text(format!("Client: {}", addr))
                .font(Font::MONOSPACE)
                .size(ts(11))
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        );
    }

    detail_col = detail_col.push(response_summary);

    // ── Separator ────────────────────────────────────────────────────
    detail_col = detail_col.push(section_separator());

    // ── Request Headers ──────────────────────────────────────────────
    if let Some(headers) = &exchange.req_headers {
        if !headers.is_empty() {
            detail_col = detail_col.push(section_title(
                "REQUEST HEADERS",
                Color::from_rgb(0.3, 0.5, 0.9),
            ));
            for (key, value) in headers {
                detail_col = detail_col.push(header_line(
                    key,
                    value,
                    Color::from_rgb(0.3, 0.5, 0.9),
                    Color::from_rgb(0.5, 0.6, 0.8),
                ));
            }
            detail_col = detail_col.push(section_separator());
        }
    }

    // ── Request Body ─────────────────────────────────────────────────
    if exchange.req_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if exchange.req_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = exchange
            .req_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        detail_col = detail_col.push(section_title(
            &format!(
                "REQUEST BODY  ({} bytes{})",
                exchange.req_body_size.unwrap_or(0),
                truncated_marker
            ),
            Color::from_rgb(0.2, 0.6, 0.8),
        ));
        if let Some(preview) = read_body_preview(
            capture_store,
            exchange.req_id,
            true,
            content_encoding.as_deref(),
        ) {
            detail_col = detail_col.push(body_text_block(
                &preview,
                Color::from_rgb(0.3, 0.6, 0.8),
            ));
        } else {
            detail_col = detail_col.push(
                text(format!(
                    "  [binary data, {} bytes{}]",
                    exchange.req_body_size.unwrap_or(0),
                    truncated_marker
                ))
                .font(Font::MONOSPACE)
                .size(super::super::text_size(11))
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
            );
        }
        detail_col = detail_col.push(section_separator());
    }

    // ── Response Headers ─────────────────────────────────────────────
    if let Some(headers) = &exchange.resp_headers {
        if !headers.is_empty() {
            detail_col = detail_col.push(section_title(
                "RESPONSE HEADERS",
                Color::from_rgb(0.7, 0.3, 0.7),
            ));
            for (key, value) in headers {
                detail_col = detail_col.push(header_line(
                    key,
                    value,
                    Color::from_rgb(0.7, 0.3, 0.7),
                    Color::from_rgb(0.7, 0.4, 0.7),
                ));
            }
            detail_col = detail_col.push(section_separator());
        }
    }

    // ── Response Body ────────────────────────────────────────────────
    if exchange.resp_body_size.unwrap_or(0) > 0 {
        let truncated_marker = if exchange.resp_body_truncated {
            " (truncated)"
        } else {
            ""
        };
        let content_encoding = exchange
            .resp_headers
            .as_ref()
            .and_then(|h| find_header_value(h, "content-encoding"));
        detail_col = detail_col.push(section_title(
            &format!(
                "RESPONSE BODY  ({} bytes{})",
                exchange.resp_body_size.unwrap_or(0),
                truncated_marker
            ),
            Color::from_rgb(0.7, 0.3, 0.7),
        ));
        if let Some(preview) = read_body_preview(
            capture_store,
            exchange.req_id,
            false,
            content_encoding.as_deref(),
        ) {
            detail_col = detail_col.push(body_text_block(
                &preview,
                Color::from_rgb(0.7, 0.4, 0.7),
            ));
        } else {
            detail_col = detail_col.push(
                text(format!(
                    "  [binary data, {} bytes{}]",
                    exchange.resp_body_size.unwrap_or(0),
                    truncated_marker
                ))
                .font(Font::MONOSPACE)
                .size(super::super::text_size(11))
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
            );
        }
        detail_col = detail_col.push(section_separator());
    }

    // ── WebSocket Messages ───────────────────────────────────────────
    if exchange.kind == HttpRequestKind::WebSocket {
        if let Some(snap) = capture_store.ws_messages(exchange.req_id) {
            use cruma_proxy_lib::proxying::ws_capture::{WsDirection, WsMessageKind};

            let c2o = snap.total_client_to_origin;
            let o2c = snap.total_origin_to_client;
            let total = snap.total_message_count;
            let shown = snap.messages.len();
            let summary = if shown as u64 == total {
                format!(
                    "WEBSOCKET MESSAGES  ({} messages  ↑{} B  ↓{} B)",
                    total, c2o, o2c
                )
            } else {
                format!(
                    "WEBSOCKET MESSAGES  ({}/{} messages, oldest evicted  ↑{} B  ↓{} B)",
                    shown, total, c2o, o2c
                )
            };
            detail_col = detail_col.push(section_title(
                &summary,
                Color::from_rgb(0.7, 0.3, 0.7),
            ));

            for msg in &snap.messages {
                let (arrow, dir_label, arrow_color) = match msg.direction {
                    WsDirection::ClientToOrigin => {
                        ("↑", "send", Color::from_rgb(0.2, 0.6, 0.8))
                    }
                    WsDirection::OriginToClient => {
                        ("↓", "recv", Color::from_rgb(0.7, 0.3, 0.7))
                    }
                };
                let kind_label = match msg.kind {
                    WsMessageKind::Text => "text",
                    WsMessageKind::Binary => "bin",
                    WsMessageKind::Ping => "ping",
                    WsMessageKind::Pong => "pong",
                    WsMessageKind::Close => "close",
                };
                let size_info = if msg.original_len != msg.payload.len() {
                    format!(
                        "{} B (truncated from {} B)",
                        msg.payload.len(),
                        msg.original_len
                    )
                } else {
                    format!("{} B", msg.original_len)
                };

                detail_col = detail_col.push(
                    row![
                        text(format!("  {} {} ", arrow, dir_label))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .color(arrow_color),
                        text(format!("[{}] ", kind_label))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .color(Color::from_rgb(0.5, 0.5, 0.5)),
                        text(size_info)
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .color(Color::from_rgb(0.5, 0.5, 0.5)),
                    ]
                    .spacing(0)
                    .align_y(iced::Alignment::Center),
                );

                // Show payload content
                let payload_str = if msg.kind == WsMessageKind::Text {
                    match std::str::from_utf8(&msg.payload) {
                        Ok(s) => sanitize(s),
                        Err(_) => format!("[binary, {} B]", msg.original_len),
                    }
                } else if msg.payload.is_empty() {
                    String::new()
                } else {
                    format!("[binary, {} B]", msg.original_len)
                };
                if !payload_str.is_empty() {
                    detail_col = detail_col.push(body_text_block(&payload_str, arrow_color));
                }
            }

            detail_col = detail_col.push(section_separator());
        }
    }

    // ── SSE Events ───────────────────────────────────────────────────
    if exchange.kind == HttpRequestKind::SSE {
        if let Some(snap) = capture_store.sse_events(exchange.req_id) {
            let total = snap.total_event_count;
            let shown = snap.events.len();
            let total_bytes = snap.total_bytes;
            let summary = if shown as u64 == total {
                format!(
                    "SSE EVENTS  ({} events  ↓{} B)",
                    total, total_bytes
                )
            } else {
                format!(
                    "SSE EVENTS  ({}/{} events, oldest evicted  ↓{} B)",
                    shown, total, total_bytes
                )
            };
            let sse_color = Color::from_rgb(0.2, 0.8, 0.3);
            detail_col = detail_col.push(section_title(&summary, sse_color));

            for evt in &snap.events {
                let event_type = evt
                    .event_type
                    .as_deref()
                    .unwrap_or("message");
                let is_comment_only = evt.data.is_empty() && !evt.comments.is_empty();
                let label = if is_comment_only {
                    "comment".to_string()
                } else {
                    sanitize(event_type)
                };
                let size_info = if evt.truncated {
                    format!(
                        "{} B (truncated from {} B)",
                        evt.data.len(),
                        evt.original_data_len
                    )
                } else if !evt.data.is_empty() {
                    format!("{} B", evt.data.len())
                } else {
                    String::new()
                };

                let mut info_parts: Vec<Element<'_, Message>> = vec![
                    text(format!("  ↓ recv "))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                        .color(sse_color)
                        .into(),
                    text(format!("[{}] ", label))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                        .color(Color::from_rgb(0.5, 0.5, 0.5))
                        .into(),
                ];

                if let Some(id) = &evt.id {
                    info_parts.push(
                        text(format!("id={} ", sanitize(id)))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .color(Color::from_rgb(0.5, 0.5, 0.5))
                            .into(),
                    );
                }

                if !size_info.is_empty() {
                    info_parts.push(
                        text(format!("({}) ", size_info))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .color(Color::from_rgb(0.5, 0.5, 0.5))
                            .into(),
                    );
                }

                detail_col = detail_col.push(
                    Row::with_children(info_parts)
                        .spacing(0)
                        .align_y(iced::Alignment::Center),
                );

                // Show content
                if is_comment_only {
                    for comment in &evt.comments {
                        detail_col = detail_col.push(
                            text(format!("    : {}", sanitize(comment)))
                                .font(Font::MONOSPACE)
                                .size(super::super::text_size(11))
                                .color(sse_color),
                        );
                    }
                } else if !evt.data.is_empty() {
                    detail_col =
                        detail_col.push(body_text_block(&sanitize(&evt.data), sse_color));
                }
            }

            detail_col = detail_col.push(section_separator());
        }
    }

    detail_col.into()
}

// ─── UI building helpers ─────────────────────────────────────────────────────

fn section_title<'a>(label: &str, color: Color) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .font(Font::MONOSPACE)
            .size(super::super::text_size(11))
            .color(color),
    )
    .padding(Padding {
        top: 6.0,
        right: 0.0,
        bottom: 2.0,
        left: 0.0,
    })
    .into()
}

fn section_separator<'a>() -> Element<'a, Message> {
    container(text(""))
        .height(Length::Fixed(1.0))
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(palette.background.strong.color.into()),
                ..Default::default()
            }
        })
        .into()
}

fn header_line<'a>(
    key: &str,
    value: &str,
    key_color: Color,
    value_color: Color,
) -> Element<'a, Message> {
    row![
        text(format!("  {}: ", key))
            .font(Font::MONOSPACE)
            .size(super::super::text_size(11))
            .color(key_color),
        text(sanitize(value))
            .font(Font::MONOSPACE)
            .size(super::super::text_size(11))
            .color(value_color)
            .wrapping(Wrapping::WordOrGlyph),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Start)
    .into()
}

fn body_text_block<'a>(content: &str, color: Color) -> Element<'a, Message> {
    let mut col = Column::new().spacing(0);
    for line in content.lines() {
        col = col.push(
            text(format!("    {}", line))
                .font(Font::MONOSPACE)
                .size(super::super::text_size(11))
                .color(color)
                .wrapping(Wrapping::WordOrGlyph),
        );
    }
    col.into()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn kind_badge_str(kind: &HttpRequestKind) -> (&'static str, Color) {
    match kind {
        HttpRequestKind::Regular => ("HTTP", Color::from_rgb(0.5, 0.5, 0.5)),
        HttpRequestKind::SSE => ("SSE", Color::from_rgb(0.2, 0.8, 0.3)),
        HttpRequestKind::WebSocket => ("WS", Color::from_rgb(0.7, 0.3, 0.7)),
        HttpRequestKind::H2cUpgrade => ("H2C", Color::from_rgb(0.3, 0.7, 0.8)),
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

/// Sanitize control characters that would cause display issues.
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' | '\r' | '\t' => out.push(ch),
            c if (c as u32) < 0x20 => {
                out.push(char::from_u32(0x2400 + c as u32).unwrap_or('.'));
            }
            '\x7f' => out.push('.'),
            '\u{feff}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{fffe}' => out.push('.'),
            c => out.push(c),
        }
    }
    out
}

/// Read a body preview from the capture store, decompressing if needed.
fn read_body_preview(
    capture_store: &Arc<HttpCaptureStore>,
    req_id: u64,
    is_request: bool,
    content_encoding: Option<&str>,
) -> Option<String> {
    let captured = capture_store.body_bytes(req_id)?;
    let raw = if is_request {
        captured.req_body.as_ref()?
    } else {
        captured.resp_body.as_ref()?
    };
    if raw.is_empty() {
        return None;
    }
    let bytes = try_decompress(raw, content_encoding);
    body_preview_string(&bytes, 8192)
}

/// Try to interpret bytes as UTF-8 text and return a sanitized preview.
fn body_preview_string(bytes: &[u8], max_len: usize) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let text_content = std::str::from_utf8(bytes).ok()?;
    let sanitized = sanitize(text_content);
    if sanitized.len() <= max_len {
        Some(sanitized)
    } else {
        let mut preview: String = sanitized.chars().take(max_len.saturating_sub(1)).collect();
        preview.push('…');
        Some(preview)
    }
}

/// Case-insensitive header lookup.
fn find_header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

/// Try to decompress bytes according to the Content-Encoding value.
fn try_decompress(bytes: &[u8], content_encoding: Option<&str>) -> Vec<u8> {
    let encoding = match content_encoding {
        Some(e) => e.trim().to_ascii_lowercase(),
        None => return try_decompress_magic(bytes),
    };

    match encoding.as_str() {
        "gzip" | "x-gzip" => decompress_gzip(bytes).unwrap_or_else(|| bytes.to_vec()),
        "deflate" => decompress_deflate(bytes).unwrap_or_else(|| bytes.to_vec()),
        "identity" | "" => bytes.to_vec(),
        _ => bytes.to_vec(),
    }
}

/// Sniff for gzip magic bytes when there's no Content-Encoding header.
fn try_decompress_magic(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        decompress_gzip(bytes).unwrap_or_else(|| bytes.to_vec())
    } else {
        bytes.to_vec()
    }
}

fn decompress_gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn decompress_deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}
