use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Row, Scrollable, button, column, container, image as iced_image, row, text, toggler,
};
use iced::{Background, Color, Element, Font, Length, Padding, Theme};

use super::super::{
    BodySide, CachedBody, CachedBodyPreview, Message, OddBoxGui, scaled, text_size,
};
use super::body_content::{
    detect_content_kind, format_size, hex_preview, text_preview, try_decode_image,
};

use cruma_proxy_lib::proxying::HttpRequestKind;
use cruma_proxy_lib::proxying::capture_store::{CapturedExchange, HttpCaptureStore};

use flate2::read::{DeflateDecoder, GzDecoder};
use std::io::Read;
use std::sync::Arc;

/// Short preview character limit (shown by default).
const PREVIEW_SHORT: usize = 512;
/// Expanded preview character limit (shown after "Load more").
const PREVIEW_EXPANDED: usize = 4096;

fn themed_text_color(theme: &Theme, light_alpha: f32, dark_alpha: f32) -> Color {
    let palette = theme.extended_palette();
    let base = palette.background.base.text;
    let alpha = if palette.is_dark {
        dark_alpha
    } else {
        light_alpha
    };
    Color::from_rgba(base.r, base.g, base.b, alpha)
}

fn meta_text_style(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(themed_text_color(theme, 0.86, 0.72)),
        ..Default::default()
    }
}

fn dim_text_style(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(themed_text_color(theme, 0.78, 0.62)),
        ..Default::default()
    }
}

fn themed_overlay_alpha(theme: &Theme, light_alpha: f32, dark_alpha: f32) -> f32 {
    if theme.extended_palette().is_dark {
        dark_alpha
    } else {
        light_alpha
    }
}

// ─── Public helpers called from mod.rs update() ──────────────────────────────

/// Compute both body previews and wrap them in a [`CachedBodyPreview`].
///
/// This is intended to be called from a background thread
/// (`tokio::task::spawn_blocking`) so the UI stays responsive while
/// decompressing large bodies.
pub(in crate::gui) fn compute_body_previews_for_cache(
    req_id: u64,
    entry: &CapturedExchange,
    capture_store: &Arc<HttpCaptureStore>,
) -> CachedBodyPreview {
    let (req_body, resp_body) = compute_body_previews(entry, capture_store);
    CachedBodyPreview {
        req_id,
        req_body,
        resp_body,
    }
}

/// Compute an expanded preview for one side of the exchange.
pub(in crate::gui) fn compute_expanded_preview(
    _req_id: u64,
    entry: &CapturedExchange,
    capture_store: &Arc<HttpCaptureStore>,
    side: BodySide,
) -> Option<String> {
    let captured = capture_store.body_bytes(entry.req_id)?;
    let (raw_bytes, headers) = match side {
        BodySide::Request => (captured.req_body.as_ref()?, entry.req_headers.as_ref()),
        BodySide::Response => (captured.resp_body.as_ref()?, entry.resp_headers.as_ref()),
    };
    let enc = headers.and_then(|h| find_header_value(h, "content-encoding"));
    let decompressed = try_decompress_full(raw_bytes, enc.as_deref());
    let ct = headers.and_then(|h| find_header_value(h, "content-type"));
    let kind = detect_content_kind(ct.as_deref(), &decompressed);
    if kind.is_text() {
        Some(text_preview(&decompressed, PREVIEW_EXPANDED))
    } else {
        Some(hex_preview(&decompressed, Some(1024)))
    }
}

/// Decompress body bytes for saving to a file — no size cap so the user
/// gets the full content.
pub(in crate::gui) fn try_decompress_for_save(
    bytes: &[u8],
    content_encoding: Option<&str>,
) -> Vec<u8> {
    let encoding = match content_encoding {
        Some(e) => e.trim().to_ascii_lowercase(),
        None => return try_decompress_full_magic(bytes),
    };
    match encoding.as_str() {
        "gzip" | "x-gzip" => decompress_gzip_full(bytes).unwrap_or_else(|| bytes.to_vec()),
        "deflate" => decompress_deflate_full(bytes).unwrap_or_else(|| bytes.to_vec()),
        "identity" | "" => bytes.to_vec(),
        _ => bytes.to_vec(),
    }
}

// ─── View ────────────────────────────────────────────────────────────────────

impl OddBoxGui {
    pub(in crate::gui) fn view_traffic_inspection(&self) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();
        let page_title = text("Traffic Inspection").size(text_size(20));

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
            .text_size(text_size(13));

        let clear_btn = button(text("Clear").size(text_size(13)))
            .padding(Padding {
                top: scaled(6.0),
                right: scaled(12.0),
                bottom: scaled(6.0),
                left: scaled(12.0),
            })
            .style(move |theme: &Theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    super::super::KdeButtonRole::Danger,
                    use_kde_buttons,
                )
            })
            .on_press(Message::TrafficInspectionClear);

        let count_label = text(format!("{} captured exchanges", snapshot.order.len()))
            .font(Font::MONOSPACE)
            .size(text_size(12));

        let mut controls_items = row![toggle, count_label, clear_btn]
            .spacing(scaled(16.0))
            .align_y(iced::Alignment::Center);

        // Show a close-detail button when a row is selected
        if self.traffic_inspection_selected.is_some() {
            let close_btn = button(text("✕ Close Detail").size(text_size(12)))
                .padding(Padding {
                    top: scaled(6.0),
                    right: scaled(12.0),
                    bottom: scaled(6.0),
                    left: scaled(12.0),
                })
                .style(move |theme: &Theme, status| {
                    super::super::themed_button_style(
                        theme,
                        status,
                        super::super::KdeButtonRole::Neutral,
                        use_kde_buttons,
                    )
                })
                .on_press(Message::TrafficInspectionSelect(None));
            controls_items = controls_items.push(close_btn);
        }

        let controls_box = container(controls_items)
            .padding(scaled(16.0))
            .width(Length::Fill)
            .style(|theme: &Theme| iced::widget::container::Style {
                background: Some(self.surface_panel_bg(theme).into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
                },
                ..Default::default()
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
                .width(Length::Fixed(56.0)),
                container(
                    text("Kind")
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                )
                .width(Length::Fixed(48.0)),
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
                .width(Length::Fixed(80.0)),
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
        .style(|theme: &Theme| iced::widget::container::Style {
            background: Some(self.surface_panel_alt_bg(theme).into()),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: self.surface_border_color(theme),
            },
            ..Default::default()
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
            // Limit to most recent 500 to keep the widget tree small
            let skip = snapshot.order.len().saturating_sub(500);
            for req_id in snapshot.order.iter().skip(skip) {
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
                            .style(|theme: &Theme| iced::widget::container::Style {
                                background: Some(self.surface_border_color(theme).into()),
                                ..Default::default()
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

        let page_bg_style = |theme: &Theme| {
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
        };

        let panel_style = |theme: &Theme| iced::widget::container::Style {
            background: Some(self.surface_panel_bg(theme).into()),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: self.surface_border_color(theme),
            },
            ..Default::default()
        };

        if has_detail {
            let selected_id = self.traffic_inspection_selected.unwrap();
            let exchange = snapshot.entries.get(&selected_id).unwrap();

            let detail_panel = build_detail_panel(
                exchange,
                capture_store,
                self.traffic_cached_body_preview.as_ref(),
                self.traffic_body_expanded,
            );

            let detail_container = container(
                Scrollable::new(detail_panel)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel_style);

            // Wide layout (>1100px): list on left, detail on right (side-by-side)
            // Narrow layout: list on top, detail on bottom (stacked)
            let wide = self.window_width > 1100.0;

            if wide {
                let exchange_list = container(
                    Scrollable::new(exchange_rows)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .style(panel_style);

                let left_panel = column![header, exchange_list]
                    .spacing(4)
                    .width(Length::FillPortion(3))
                    .height(Length::Fill);

                let right_panel = container(detail_container)
                    .width(Length::FillPortion(2))
                    .height(Length::Fill);

                let split = row![left_panel, right_panel]
                    .spacing(4)
                    .width(Length::Fill)
                    .height(Length::Fill);

                let inner = column![page_title, controls_box, split]
                    .spacing(4)
                    .padding(30)
                    .width(Length::Fill)
                    .height(Length::Fill);

                container(inner)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(page_bg_style)
                    .into()
            } else {
                let exchange_list = container(
                    Scrollable::new(exchange_rows)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::FillPortion(2))
                .style(panel_style);

                let detail_container = detail_container.height(Length::FillPortion(3));

                let inner = column![
                    page_title,
                    controls_box,
                    header,
                    exchange_list,
                    detail_container
                ]
                .spacing(4)
                .padding(30)
                .width(Length::Fill)
                .height(Length::Fill);

                container(inner)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(page_bg_style)
                    .into()
            }
        } else {
            // No detail selected: full-height list, no detail pane
            let exchange_list = container(
                Scrollable::new(exchange_rows)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(panel_style);

            let inner = column![page_title, controls_box, header, exchange_list]
                .spacing(4)
                .padding(30)
                .width(Length::Fill)
                .height(Length::Fill);

            container(inner)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(page_bg_style)
                .into()
        }
    }
}

// ─── Exchange row (table row) ────────────────────────────────────────────────

fn build_exchange_row<'a>(exchange: &CapturedExchange, _is_selected: bool) -> Element<'a, Message> {
    let method_color = method_color_value(&exchange.method);
    let status_color = status_code_color(exchange.status);

    let is_streaming = exchange.is_inflight
        && matches!(
            exchange.kind,
            HttpRequestKind::SSE | HttpRequestKind::WebSocket
        );

    let status_str = if is_streaming {
        match exchange.status {
            Some(s) => format!("{s} ⇣"),
            None => "⇣···".to_string(),
        }
    } else {
        match exchange.status {
            Some(s) => format!("{}", s),
            None if exchange.is_inflight => "···".to_string(),
            None => "—".to_string(),
        }
    };
    let status_display_color = if is_streaming {
        Color::from_rgb(0.3, 0.85, 0.4)
    } else {
        status_color
    };

    let duration_str = if is_streaming {
        match exchange.kind {
            HttpRequestKind::SSE => "⇣ stream".to_string(),
            HttpRequestKind::WebSocket => "⇅ open".to_string(),
            _ => "⇣ stream".to_string(),
        }
    } else {
        format_latency(exchange.duration_ms)
    };
    let duration_color = if is_streaming {
        Color::from_rgb(0.3, 0.85, 0.4)
    } else {
        latency_color_value(exchange.duration_ms)
    };

    let size_str = {
        let resp_sz = exchange.resp_body_size.unwrap_or(0);
        if resp_sz > 0 {
            format_size(resp_sz)
        } else if exchange.is_inflight {
            "···".to_string()
        } else {
            "—".to_string()
        }
    };

    let host_str = exchange.host.clone().unwrap_or_default();
    let path_str = exchange.path.clone();

    let (kind_label, kind_color) = kind_badge_info(&exchange.kind);
    // Also show HTTP version for HTTP/2+ next to the kind
    let kind_display = if let Some(ref ver) = exchange.http_version {
        match ver.as_str() {
            "HTTP/2" => format!("{kind_label} h2"),
            "HTTP/3" => format!("{kind_label} h3"),
            _ => kind_label.to_string(),
        }
    } else {
        kind_label.to_string()
    };

    container(
        row![
            container(
                text(exchange.method.clone())
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Font::MONOSPACE
                    })
                    .size(super::super::text_size(12))
                    .color(method_color)
            )
            .width(Length::Fixed(60.0)),
            container(
                text(status_str)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Font::MONOSPACE
                    })
                    .size(super::super::text_size(12))
                    .color(status_display_color)
            )
            .width(Length::Fixed(56.0)),
            container(
                text(kind_display)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Font::MONOSPACE
                    })
                    .size(super::super::text_size(10))
                    .color(kind_color)
            )
            .width(Length::Fixed(48.0)),
            container(
                text(duration_str)
                    .font(Font::MONOSPACE)
                    .size(super::super::text_size(11))
                    .color(duration_color)
            )
            .width(Length::Fixed(80.0)),
            container(
                text(size_str)
                    .font(Font::MONOSPACE)
                    .size(super::super::text_size(11))
            )
            .width(Length::Fixed(80.0)),
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
                    .style(dim_text_style)
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
    cached_preview: Option<&CachedBodyPreview>,
    body_expanded: bool,
) -> Element<'a, Message> {
    let ts = super::super::text_size;

    let mut detail_col = Column::new().spacing(4).padding(16).width(Length::Fill);

    // ── Title bar ────────────────────────────────────────────────────
    let method_color = method_color_value(&exchange.method);
    let status_color = status_code_color(exchange.status);

    let (badge_text, badge_color) = kind_badge_info(&exchange.kind);

    // Request summary line
    let mut title_items: Vec<Element<'_, Message>> = Vec::new();

    // Kind badge
    if exchange.kind != HttpRequestKind::Regular {
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
                .style(meta_text_style)
                .into(),
        );
    }

    // Method
    title_items.push(
        text(format!(" {} ", &exchange.method))
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::MONOSPACE
            })
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
        let mut parts: Vec<Element<'_, Message>> = vec![
            text(stream_label)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::MONOSPACE
                })
                .size(ts(12))
                .color(Color::from_rgb(0.3, 0.85, 0.4))
                .into(),
        ];
        if let Some(status) = exchange.status {
            parts.push(
                text(format!(
                    "  Status: {} {}",
                    status,
                    status_reason_phrase(status)
                ))
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
            Some(s) => format!("{} {}", s, status_reason_phrase(s)),
            None => "pending...".to_string(),
        };
        let duration_str = format_latency(exchange.duration_ms);
        row![
            text(format!("Status: {}", status_str))
                .font(Font::MONOSPACE)
                .size(ts(12))
                .color(status_color),
            text(format!("  Duration: {}", duration_str))
                .font(Font::MONOSPACE)
                .size(ts(12))
                .style(meta_text_style),
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
                .style(meta_text_style),
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
                Color::from_rgb(0.4, 0.7, 0.92),
            ));
            for (key, value) in headers.iter().take(30) {
                detail_col = detail_col.push(header_line(
                    key,
                    value,
                    Color::from_rgb(0.4, 0.7, 0.92),
                    Color::from_rgb(0.5, 0.6, 0.8),
                ));
            }
            if headers.len() > 30 {
                detail_col = detail_col.push(
                    text(format!("  … {} more headers", headers.len() - 30))
                        .font(Font::MONOSPACE)
                        .size(ts(11))
                        .style(meta_text_style),
                );
            }
            detail_col = detail_col.push(section_separator());
        }
    }

    // ── Request Body ─────────────────────────────────────────────────
    if exchange.req_body_size.unwrap_or(0) > 0 {
        let body_size = exchange.req_body_size.unwrap_or(0);
        let truncated = exchange.req_body_truncated;

        let cached_req = cached_preview.and_then(|c| c.req_body.as_ref());

        detail_col = detail_col.push(rich_body_section(
            "REQUEST BODY",
            body_size,
            truncated,
            cached_req,
            body_expanded,
            BodySide::Request,
            Color::from_rgb(0.3, 0.75, 0.9),
        ));
        detail_col = detail_col.push(section_separator());
    }

    // ── Response Headers ─────────────────────────────────────────────
    if let Some(headers) = &exchange.resp_headers {
        if !headers.is_empty() {
            detail_col = detail_col.push(section_title(
                "RESPONSE HEADERS",
                Color::from_rgb(0.75, 0.45, 0.9),
            ));
            for (key, value) in headers.iter().take(30) {
                detail_col = detail_col.push(header_line(
                    key,
                    value,
                    Color::from_rgb(0.75, 0.45, 0.9),
                    Color::from_rgb(0.7, 0.5, 0.8),
                ));
            }
            if headers.len() > 30 {
                detail_col = detail_col.push(
                    text(format!("  … {} more headers", headers.len() - 30))
                        .font(Font::MONOSPACE)
                        .size(ts(11))
                        .style(meta_text_style),
                );
            }
            detail_col = detail_col.push(section_separator());
        }
    }

    // ── Response Body ────────────────────────────────────────────────
    if exchange.resp_body_size.unwrap_or(0) > 0 {
        let body_size = exchange.resp_body_size.unwrap_or(0);
        let truncated = exchange.resp_body_truncated;

        let cached_resp = cached_preview.and_then(|c| c.resp_body.as_ref());

        detail_col = detail_col.push(rich_body_section(
            "RESPONSE BODY",
            body_size,
            truncated,
            cached_resp,
            body_expanded,
            BodySide::Response,
            Color::from_rgb(0.75, 0.45, 0.9),
        ));
        detail_col = detail_col.push(section_separator());
    } else if !is_streaming
        && exchange.status.is_some()
        && exchange.kind == HttpRequestKind::Regular
    {
        detail_col = detail_col.push(
            text("No response body")
                .font(Font::MONOSPACE)
                .size(ts(11))
                .style(meta_text_style),
        );
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
                    "WEBSOCKET MESSAGES  ({} messages  ↑{}  ↓{})",
                    total,
                    format_size(c2o as usize),
                    format_size(o2c as usize)
                )
            } else {
                format!(
                    "WEBSOCKET MESSAGES  ({}/{} messages, oldest evicted  ↑{}  ↓{})",
                    shown,
                    total,
                    format_size(c2o as usize),
                    format_size(o2c as usize)
                )
            };
            detail_col = detail_col.push(section_title(&summary, Color::from_rgb(0.75, 0.45, 0.9)));

            // Cap displayed messages
            let msg_skip = snap.messages.len().saturating_sub(50);
            if msg_skip > 0 {
                detail_col = detail_col.push(
                    text(format!("  … {msg_skip} older messages not shown"))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                        .style(meta_text_style),
                );
            }

            for msg in snap.messages.iter().skip(msg_skip) {
                let (arrow, dir_label, arrow_color) = match msg.direction {
                    WsDirection::ClientToOrigin => ("↑", "send", Color::from_rgb(0.3, 0.75, 0.9)),
                    WsDirection::OriginToClient => ("↓", "recv", Color::from_rgb(0.75, 0.45, 0.9)),
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
                        "{} (truncated from {})",
                        format_size(msg.payload.len()),
                        format_size(msg.original_len)
                    )
                } else {
                    format_size(msg.original_len)
                };

                detail_col = detail_col.push(
                    row![
                        text(format!("  {} {} ", arrow, dir_label))
                            .font(Font {
                                weight: iced::font::Weight::Bold,
                                ..Font::MONOSPACE
                            })
                            .size(super::super::text_size(11))
                            .color(arrow_color),
                        text(format!("[{}] ", kind_label))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .style(meta_text_style),
                        text(size_info)
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .style(meta_text_style),
                    ]
                    .spacing(0)
                    .align_y(iced::Alignment::Center),
                );

                // Show payload content
                let payload_str = if msg.kind == WsMessageKind::Text {
                    match std::str::from_utf8(&msg.payload) {
                        Ok(s) => sanitize(s),
                        Err(_) => format!("[binary, {}]", format_size(msg.original_len)),
                    }
                } else if msg.payload.is_empty() {
                    String::new()
                } else {
                    text_preview(&msg.payload, 512)
                };
                if !payload_str.is_empty() {
                    detail_col = detail_col.push(body_code_block(&payload_str));
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
                    "SSE EVENTS  ({} events  ↓{})",
                    total,
                    format_size(total_bytes as usize)
                )
            } else {
                format!(
                    "SSE EVENTS  ({}/{} events, oldest evicted  ↓{})",
                    shown,
                    total,
                    format_size(total_bytes as usize)
                )
            };
            let sse_color = Color::from_rgb(0.3, 0.85, 0.4);
            detail_col = detail_col.push(section_title(&summary, sse_color));

            let evt_skip = snap.events.len().saturating_sub(50);
            if evt_skip > 0 {
                detail_col = detail_col.push(
                    text(format!("  … {evt_skip} older events not shown"))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                        .style(meta_text_style),
                );
            }

            for evt in snap.events.iter().skip(evt_skip) {
                let event_type = evt.event_type.as_deref().unwrap_or("message");
                let is_comment_only = evt.data.is_empty() && !evt.comments.is_empty();
                let label = if is_comment_only {
                    "comment".to_string()
                } else {
                    sanitize(event_type)
                };
                let size_info = if evt.truncated {
                    format!(
                        "{} (truncated from {})",
                        format_size(evt.data.len()),
                        format_size(evt.original_data_len)
                    )
                } else if !evt.data.is_empty() {
                    format_size(evt.data.len())
                } else {
                    String::new()
                };

                let mut info_parts: Vec<Element<'_, Message>> = vec![
                    text("  ↓ recv ".to_string())
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Font::MONOSPACE
                        })
                        .size(super::super::text_size(11))
                        .color(sse_color)
                        .into(),
                    text(format!("[{}] ", label))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(11))
                        .style(meta_text_style)
                        .into(),
                ];

                if let Some(id) = &evt.id {
                    info_parts.push(
                        text(format!("id={} ", sanitize(id)))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .style(meta_text_style)
                            .into(),
                    );
                }

                if !size_info.is_empty() {
                    info_parts.push(
                        text(format!("({}) ", size_info))
                            .font(Font::MONOSPACE)
                            .size(super::super::text_size(11))
                            .style(meta_text_style)
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
                        detail_col =
                            detail_col.push(body_code_block(&format!(": {}", sanitize(comment))));
                    }
                } else if !evt.data.is_empty() {
                    detail_col = detail_col.push(body_code_block(&sanitize(&evt.data)));
                }
            }

            detail_col = detail_col.push(section_separator());
        }
    }

    detail_col.into()
}

// ─── Rich body section ───────────────────────────────────────────────────────

/// Rich body section that renders differently depending on the detected
/// content kind — text preview with "Load more" / "Save to file" buttons,
/// inline image, or hex dump.
fn rich_body_section<'a>(
    title: &str,
    body_size: usize,
    truncated: bool,
    cached_body: Option<&CachedBody>,
    body_expanded: bool,
    side: BodySide,
    accent_color: Color,
) -> Element<'a, Message> {
    let ts = super::super::text_size;
    let truncated_marker = if truncated { " (truncated)" } else { "" };
    let size_str = format!("{}{}", format_size(body_size), truncated_marker);

    let mut col = Column::new().spacing(4).width(Length::Fill);

    // Check if body data is being loaded
    if cached_body.is_none() {
        // Loading state
        col = col.push(
            row![
                text(title.to_string())
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Font::MONOSPACE
                    })
                    .size(ts(11))
                    .color(accent_color),
                text(format!("  ({})", size_str))
                    .font(Font::MONOSPACE)
                    .size(ts(11))
                    .style(meta_text_style),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        );

        col = col.push(
            container(
                text("⏳ Loading body preview…")
                    .font(Font::MONOSPACE)
                    .size(ts(11))
                    .style(meta_text_style),
            )
            .padding(Padding {
                top: 6.0,
                right: 8.0,
                bottom: 6.0,
                left: 8.0,
            })
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                let weak = palette.background.weak.color;
                iced::widget::container::Style {
                    background: Some(
                        Color {
                            a: themed_overlay_alpha(theme, 0.55, 0.20),
                            ..weak
                        }
                        .into(),
                    ),
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: Color {
                            a: themed_overlay_alpha(theme, 0.26, 0.10),
                            ..palette.background.strong.color
                        },
                    },
                    ..Default::default()
                }
            }),
        );

        return col.into();
    }

    let body = cached_body.unwrap();
    let kind_label = body.kind.label().to_string();

    // ── Header row: title + kind badge + size ────────────────────
    col = col.push(
        row![
            text(title.to_string())
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::MONOSPACE
                })
                .size(ts(11))
                .color(accent_color),
            container(text(kind_label).font(Font::MONOSPACE).size(ts(10)).style(
                dim_text_style
            ))
            .padding(Padding {
                top: 1.0,
                right: 6.0,
                bottom: 1.0,
                left: 6.0,
            })
            .style(|theme: &Theme| {
                let weak = theme.extended_palette().background.weak.color;
                iced::widget::container::Style {
                    background: Some(
                        Color {
                            a: themed_overlay_alpha(theme, 0.60, 0.30),
                            ..weak
                        }
                        .into(),
                    ),
                    border: iced::Border {
                        radius: 3.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            }),
            text(format!("  {}", size_str))
                .font(Font::MONOSPACE)
                .size(ts(11))
                .style(meta_text_style),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    );

    // ── Inline image ─────────────────────────────────────────────
    if let Some(ref decoded) = body.image {
        let img_widget = iced_image(decoded.handle.clone())
            .content_fit(iced::ContentFit::ScaleDown)
            .width(Length::Fill);

        col = col.push(
            container(img_widget)
                .max_width(480.0)
                .padding(8)
                .style(|theme: &Theme| {
                    let weak = theme.extended_palette().background.weak.color;
                    iced::widget::container::Style {
                        background: Some(
                            Color {
                                a: themed_overlay_alpha(theme, 0.52, 0.20),
                                ..weak
                            }
                            .into(),
                        ),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: Color {
                                a: themed_overlay_alpha(theme, 0.30, 0.15),
                                ..theme.extended_palette().background.strong.color
                            },
                        },
                        ..Default::default()
                    }
                }),
        );

        col = col.push(
            text(format!(
                "{}×{} px",
                decoded.original_width, decoded.original_height
            ))
            .font(Font::MONOSPACE)
            .size(ts(10))
            .style(meta_text_style),
        );
    } else {
        // ── Text / hex code block ────────────────────────────────
        let display_text = if body_expanded {
            body.expanded_preview.as_deref().unwrap_or(&body.preview)
        } else {
            &body.preview
        };

        col = col.push(body_code_block(display_text));
    }

    // ── Action buttons: Load more + Save to file ─────────────────
    let mut actions = Row::new().spacing(8).align_y(iced::Alignment::Center);

    // "Load more" — only for text bodies that were truncated in the preview
    if body.image.is_none() {
        let display_text = if body_expanded {
            body.expanded_preview.as_deref().unwrap_or(&body.preview)
        } else {
            &body.preview
        };
        let could_have_more = body.kind.is_text()
            && !body_expanded
            && (display_text.ends_with('…') || body.raw_size > display_text.len());
        if could_have_more {
            actions = actions.push(
                button(text("⤵ Load more").font(Font::MONOSPACE).size(ts(11)))
                    .on_press(Message::TrafficInspectionExpandBody(side))
                    .padding(Padding {
                        top: 3.0,
                        right: 8.0,
                        bottom: 3.0,
                        left: 8.0,
                    })
                    .style(action_link_button_style),
            );
        }
    }

    // "Save to file" — always available
    actions = actions.push(
        button(text("💾 Save to file").font(Font::MONOSPACE).size(ts(11)))
            .on_press(Message::TrafficInspectionSaveBody(side))
            .padding(Padding {
                top: 3.0,
                right: 8.0,
                bottom: 3.0,
                left: 8.0,
            })
            .style(action_link_button_style),
    );

    col = col.push(actions);

    col.into()
}

// ─── UI building helpers ─────────────────────────────────────────────────────

fn section_title<'a>(label: &str, color: Color) -> Element<'a, Message> {
    container(
        text(label.to_string())
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::MONOSPACE
            })
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
                background: Some(
                    Color {
                        a: 0.25,
                        ..palette.background.strong.color
                    }
                    .into(),
                ),
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

fn body_code_block<'a>(content: &str) -> Element<'a, Message> {
    let owned = content.to_string();
    container(
        text(owned)
            .font(Font::MONOSPACE)
            .size(super::super::text_size(11))
            .wrapping(Wrapping::WordOrGlyph),
    )
    .padding(Padding {
        top: 6.0,
        right: 8.0,
        bottom: 6.0,
        left: 8.0,
    })
    .width(Length::Fill)
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        let weak = palette.background.weak.color;
        iced::widget::container::Style {
            background: Some(
                Color {
                    a: themed_overlay_alpha(theme, 0.70, 0.35),
                    ..weak
                }
                .into(),
            ),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: Color {
                    a: themed_overlay_alpha(theme, 0.34, 0.15),
                    ..palette.background.strong.color
                },
            },
            ..Default::default()
        }
    })
    .into()
}

/// Subtle ghost-style for action link buttons in the body section.
fn action_link_button_style(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let text_color = match status {
        iced::widget::button::Status::Hovered => palette.primary.base.color,
        _ => themed_text_color(theme, 0.86, 0.66),
    };
    let bg = match status {
        iced::widget::button::Status::Hovered => Some(
            Color {
                a: 0.15,
                ..palette.primary.weak.color
            }
            .into(),
        ),
        _ => None,
    };
    iced::widget::button::Style {
        background: bg,
        text_color,
        border: iced::Border {
            radius: 4.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn kind_badge_info(kind: &HttpRequestKind) -> (&'static str, Color) {
    match kind {
        HttpRequestKind::Regular => ("HTTP", Color::from_rgba(0.5, 0.5, 0.5, 0.55)),
        HttpRequestKind::SSE => ("SSE", Color::from_rgb(0.3, 0.85, 0.4)),
        HttpRequestKind::WebSocket => ("WS", Color::from_rgb(0.75, 0.45, 0.9)),
        HttpRequestKind::H2cUpgrade => ("H2C", Color::from_rgb(0.3, 0.75, 0.9)),
    }
}

fn method_color_value(method: &str) -> Color {
    match method.to_uppercase().as_str() {
        "GET" => Color::from_rgb(0.40, 0.70, 0.92),
        "POST" => Color::from_rgb(0.30, 0.78, 0.45),
        "PUT" => Color::from_rgb(0.92, 0.70, 0.20),
        "PATCH" => Color::from_rgb(0.70, 0.55, 0.90),
        "DELETE" => Color::from_rgb(0.90, 0.35, 0.35),
        "HEAD" => Color::from_rgb(0.55, 0.75, 0.70),
        "OPTIONS" => Color::from_rgb(0.65, 0.65, 0.65),
        _ => Color::from_rgb(0.60, 0.60, 0.60),
    }
}

fn status_code_color(status: Option<u16>) -> Color {
    match status {
        None => Color::from_rgba(0.55, 0.55, 0.55, 0.7),
        Some(code) if code < 200 => Color::from_rgb(0.55, 0.55, 0.55),
        Some(code) if code < 300 => Color::from_rgb(0.30, 0.75, 0.40),
        Some(code) if code < 400 => Color::from_rgb(0.40, 0.65, 0.90),
        Some(code) if code < 500 => Color::from_rgb(0.92, 0.70, 0.20),
        Some(_) => Color::from_rgb(0.90, 0.30, 0.30),
    }
}

fn latency_color_value(ms: Option<u128>) -> Color {
    match ms {
        None => Color::from_rgb(0.5, 0.5, 0.5),
        Some(ms) if ms < 100 => Color::from_rgb(0.30, 0.78, 0.45),
        Some(ms) if ms < 500 => Color::from_rgb(0.92, 0.70, 0.20),
        Some(_) => Color::from_rgb(0.90, 0.35, 0.35),
    }
}

fn format_latency(ms: Option<u128>) -> String {
    match ms {
        None => "—".to_string(),
        Some(d) if d < 1 => "<1 ms".to_string(),
        Some(d) if d < 1000 => format!("{d} ms"),
        Some(d) if d < 60_000 => format!("{:.1} s", d as f64 / 1000.0),
        Some(d) => format!("{:.0}m {:.0}s", d / 60_000, (d % 60_000) as f64 / 1000.0),
    }
}

fn status_reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "",
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

// ─── Body preview computation (called on background threads) ─────────────────

/// Compute both request and response body previews for the given entry.
fn compute_body_previews(
    entry: &CapturedExchange,
    capture_store: &Arc<HttpCaptureStore>,
) -> (
    Option<super::super::CachedBody>,
    Option<super::super::CachedBody>,
) {
    if let Some(captured) = capture_store.body_bytes(entry.req_id) {
        let req_body = captured
            .req_body
            .as_ref()
            .map(|b| build_cached_body(b, entry.req_headers.as_ref(), captured.req_body_truncated));
        let resp_body = captured.resp_body.as_ref().map(|b| {
            build_cached_body(b, entry.resp_headers.as_ref(), captured.resp_body_truncated)
        });
        (req_body, resp_body)
    } else {
        (None, None)
    }
}

/// Build a [`CachedBody`] for one side of the exchange.
fn build_cached_body(
    raw_bytes: &[u8],
    headers: Option<&Vec<(String, String)>>,
    truncated: bool,
) -> super::super::CachedBody {
    let enc = headers.and_then(|h| find_header_value(h, "content-encoding"));
    let decompressed = try_decompress_limited(raw_bytes, enc.as_deref());

    let ct = headers.and_then(|h| find_header_value(h, "content-type"));
    let kind = detect_content_kind(ct.as_deref(), &decompressed);

    let preview;
    let image;

    if kind.is_inline_image() {
        // For images, try to decode from the FULL raw bytes (the limited
        // decompression might have cut the stream short). Images are
        // rarely compressed with Content-Encoding on top of their native
        // compression, so raw_bytes is usually the complete image.
        image = try_decode_image(raw_bytes);
        preview = format!("[{} image, {}]", kind.label(), format_size(raw_bytes.len()));
    } else if kind.is_text() {
        preview = text_preview(&decompressed, PREVIEW_SHORT);
        image = None;
    } else {
        // Binary — show hex dump
        preview = hex_preview(&decompressed, None);
        image = None;
    }

    super::super::CachedBody {
        kind,
        preview,
        expanded_preview: None,
        image,
        raw_size: raw_bytes.len(),
        truncated,
    }
}

// ─── Decompression helpers ───────────────────────────────────────────────────

/// Case-insensitive header lookup.
fn find_header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

/// Maximum number of decompressed bytes for a UI preview.
const DECOMPRESS_LIMIT: usize = 4096;

/// Try to decompress `bytes` according to the Content-Encoding value,
/// capping output at [`DECOMPRESS_LIMIT`] bytes.
fn try_decompress_limited(bytes: &[u8], content_encoding: Option<&str>) -> Vec<u8> {
    let encoding = match content_encoding {
        Some(e) => e.trim().to_ascii_lowercase(),
        None => return try_decompress_magic_limited(bytes),
    };

    match encoding.as_str() {
        "gzip" | "x-gzip" => {
            decompress_gzip_limited(bytes).unwrap_or_else(|| truncate_bytes(bytes))
        }
        "deflate" => decompress_deflate_limited(bytes).unwrap_or_else(|| truncate_bytes(bytes)),
        "identity" | "" => truncate_bytes(bytes),
        _ => truncate_bytes(bytes),
    }
}

/// When there's no Content-Encoding header, sniff for gzip magic bytes.
fn try_decompress_magic_limited(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        decompress_gzip_limited(bytes).unwrap_or_else(|| truncate_bytes(bytes))
    } else {
        truncate_bytes(bytes)
    }
}

/// Return at most [`DECOMPRESS_LIMIT`] bytes.
fn truncate_bytes(bytes: &[u8]) -> Vec<u8> {
    let end = bytes.len().min(DECOMPRESS_LIMIT);
    bytes[..end].to_vec()
}

fn decompress_gzip_limited(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = vec![0u8; DECOMPRESS_LIMIT];
    let mut filled = 0;
    loop {
        match decoder.read(&mut out[filled..]) {
            Ok(0) => break,
            Ok(n) => {
                filled += n;
                if filled >= DECOMPRESS_LIMIT {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    out.truncate(filled);
    Some(out)
}

fn decompress_deflate_limited(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = vec![0u8; DECOMPRESS_LIMIT];
    let mut filled = 0;
    loop {
        match decoder.read(&mut out[filled..]) {
            Ok(0) => break,
            Ok(n) => {
                filled += n;
                if filled >= DECOMPRESS_LIMIT {
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    out.truncate(filled);
    Some(out)
}

/// Decompress without a byte-count cap (for save-to-file / expanded preview).
fn try_decompress_full(bytes: &[u8], content_encoding: Option<&str>) -> Vec<u8> {
    let encoding = match content_encoding {
        Some(e) => e.trim().to_ascii_lowercase(),
        None => return try_decompress_full_magic(bytes),
    };
    match encoding.as_str() {
        "gzip" | "x-gzip" => decompress_gzip_full(bytes).unwrap_or_else(|| bytes.to_vec()),
        "deflate" => decompress_deflate_full(bytes).unwrap_or_else(|| bytes.to_vec()),
        "identity" | "" => bytes.to_vec(),
        _ => bytes.to_vec(),
    }
}

fn try_decompress_full_magic(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        decompress_gzip_full(bytes).unwrap_or_else(|| bytes.to_vec())
    } else {
        bytes.to_vec()
    }
}

fn decompress_gzip_full(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn decompress_deflate_full(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}
