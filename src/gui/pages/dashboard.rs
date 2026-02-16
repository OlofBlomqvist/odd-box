use iced::theme;
use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Row, Space, button, column, container, mouse_area, responsive, row, text, tooltip,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Size, Theme};

use crate::global_state::ProcState;

use super::super::{Message, OddBoxGui};

/// Minimum card width for responsive layout
const CARD_MIN_WIDTH: f32 = 220.0;
const CARD_GAP: f32 = 12.0;
/// Base card height – scaled by the global text scale factor so cards
/// grow proportionally with font size when the window is resized.
const CARD_BASE_HEIGHT: f32 = 72.0;

fn card_height() -> f32 {
    CARD_BASE_HEIGHT * super::super::gui_scale()
}

/// Estimate how many characters fit in the given pixel width for a
/// monospace-ish font at the specified base size, then truncate with "…"
/// if the string is too long.  Returns the (possibly truncated) string.
fn truncate_to_fit(s: &str, available_px: f32, font_base_size: u16) -> String {
    // Approximate character width: ~0.62 × font pixel size for monospace bold
    let char_w = super::super::text_size(font_base_size) * 0.62;
    if char_w <= 0.0 {
        return s.to_string();
    }
    let max_chars = (available_px / char_w).floor() as usize;
    if max_chars == 0 {
        return String::new();
    }
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

/// Category accent colors
const COLOR_PROCESS: Color = Color::from_rgb(0.4, 0.6, 1.0);
const COLOR_REMOTE: Color = Color::from_rgb(0.7, 0.5, 1.0);
const COLOR_DIR_SERVER: Color = Color::from_rgb(0.3, 0.8, 0.7);
const COLOR_UNBOUND: Color = Color::from_rgb(0.75, 0.55, 0.25);

fn is_light_theme(theme: &Theme) -> bool {
    let bg = theme.extended_palette().background.base.color;
    (0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b) > 0.5
}

fn text_on_color(bg: Color) -> Color {
    let luma = 0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b;
    if luma > 0.58 {
        Color::from_rgb(0.08, 0.08, 0.08)
    } else {
        Color::WHITE
    }
}

/// Status dot color based on process state
fn status_color(state: &ProcState) -> Color {
    match state {
        ProcState::Running | ProcState::Remote | ProcState::DirServer | ProcState::Docker => {
            Color::from_rgb(0.22, 0.66, 0.30)
        }
        ProcState::Starting | ProcState::Stopping => Color::from_rgb(0.82, 0.56, 0.18),
        ProcState::Stopped => Color::from_rgb(0.45, 0.45, 0.45),
        ProcState::Faulty => Color::from_rgb(0.78, 0.22, 0.22),
    }
}

/// Shared card button style (adds hover border)
fn card_button_style_with_accent(
    theme: &Theme,
    status: button::Status,
    accent: Color,
    surface_bg: Color,
    surface_border: Color,
) -> button::Style {
    let palette = theme.extended_palette();
    let (border_color, border_width) = match status {
        button::Status::Hovered | button::Status::Pressed => (accent, 2.0),
        _ => (surface_border, 1.0),
    };
    button::Style {
        background: Some(surface_bg.into()),
        text_color: palette.background.base.text,
        border: Border {
            radius: 8.0.into(),
            width: border_width,
            color: border_color,
        },
        ..Default::default()
    }
}

/// Selected card container style
fn selected_card_style(_theme: &Theme, accent: Color, surface_bg: Color) -> container::Style {
    container::Style {
        background: Some(surface_bg.into()),
        border: Border {
            radius: 8.0.into(),
            width: 2.0,
            color: accent,
        },
        ..Default::default()
    }
}

/// Muted text style
fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    let palette = theme.extended_palette();
    let color = if is_light_theme(theme) {
        let c = palette.background.base.text;
        Color::from_rgba(c.r, c.g, c.b, 0.78)
    } else {
        palette.background.weak.text
    };
    iced::widget::text::Style {
        color: Some(color),
        ..Default::default()
    }
}

/// Build a compact action button for the card context menu
fn action_btn<'a>(
    label: &'a str,
    msg: Option<Message>,
    bg_normal: Color,
    bg_hover: Color,
    fg: Color,
    use_kde_buttons: bool,
    kde_role: super::super::KdeButtonRole,
) -> Element<'a, Message> {
    let mut btn = button(
        text(label)
            .size(super::super::text_size(11))
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
    )
    .padding(Padding {
        top: 3.0,
        right: 10.0,
        bottom: 3.0,
        left: 10.0,
    })
    .style(move |theme: &Theme, status| {
        if use_kde_buttons {
            return super::super::kde_button_style(theme, status, kde_role);
        }
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => bg_hover,
            button::Status::Disabled => {
                Color::from_rgba(bg_normal.r, bg_normal.g, bg_normal.b, 0.3)
            }
            _ => bg_normal,
        };
        let text_color = match status {
            button::Status::Disabled => Color::from_rgba(fg.r, fg.g, fg.b, 0.4),
            _ => fg,
        };
        button::Style {
            background: Some(bg.into()),
            text_color,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    if let Some(m) = msg {
        btn = btn.on_press(m);
    }

    btn.into()
}

/// Build a site card – shows hostname (or backend name for unbound), with status dot.
/// When selected, shows action buttons instead of subtitle.
fn site_card<'a>(
    display_name: &str,
    subtitle: &str,
    accent_color: Color,
    surface_bg: Color,
    surface_border: Color,
    state: &ProcState,
    card_width: f32,
    is_selected: bool,
    actions: Vec<Element<'a, Message>>,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let dot_color = status_color(state);

    // Available text width: card_width minus horizontal padding (14×2),
    // dot width (~text_size(14)), and spacing (6).
    let text_avail = card_width - 28.0 - super::super::text_size(14) - 6.0;

    let name_truncated = truncate_to_fit(display_name, text_avail, 14);
    let name_is_truncated = name_truncated.len() != display_name.len();

    let subtitle_truncated = truncate_to_fit(subtitle, text_avail, 11);
    let subtitle_is_truncated = subtitle_truncated.len() != subtitle.len();

    let name_row = row![
        text("●").color(dot_color).size(super::super::text_size(14)),
        container(
            text(name_truncated)
                .size(super::super::text_size(14))
                .wrapping(Wrapping::None)
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

    if is_selected {
        // Selected state: show action buttons instead of subtitle
        let actions_row = Row::with_children(actions)
            .spacing(4)
            .align_y(Alignment::Center);

        let card_content = column![name_row, actions_row]
            .spacing(8)
            .width(Length::Fill);

        let accent = accent_color;
        let card_widget: Element<'a, Message> = container(card_content)
            .padding(Padding {
                top: 12.0,
                right: 14.0,
                bottom: 12.0,
                left: 14.0,
            })
            .width(Length::Fixed(card_width))
            .height(Length::Shrink)
            .clip(false)
            .style(move |theme: &Theme| selected_card_style(theme, accent, surface_bg))
            .into();

        if name_is_truncated {
            tooltip(
                card_widget,
                text(display_name.to_string()).size(super::super::text_size(12)),
                tooltip::Position::Top,
            )
            .style(|theme: &Theme| {
                let p = theme.extended_palette();
                container::Style {
                    background: Some(p.background.strong.color.into()),
                    border: Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: p.background.weak.color,
                    },
                    ..Default::default()
                }
            })
            .into()
        } else {
            card_widget
        }
    } else {
        // Normal state: clickable card with subtitle
        let subtitle_row = container(
            text(subtitle_truncated)
                .size(super::super::text_size(11))
                .wrapping(Wrapping::None)
                .style(muted_text),
        )
        .width(Length::Fill);

        let card_content = column![name_row, subtitle_row]
            .spacing(6)
            .width(Length::Fill);

        let mut card = button(card_content)
            .padding(Padding {
                top: 12.0,
                right: 14.0,
                bottom: 12.0,
                left: 14.0,
            })
            .width(Length::Fixed(card_width))
            .height(Length::Fixed(card_height()))
            .clip(true)
            .style(move |theme, status| {
                card_button_style_with_accent(
                    theme,
                    status,
                    accent_color,
                    surface_bg,
                    surface_border,
                )
            });

        if let Some(msg) = on_press {
            card = card.on_press(msg);
        }

        let card_widget: Element<'a, Message> = card.into();

        // Show a tooltip with full name + subtitle when either was truncated
        if name_is_truncated || subtitle_is_truncated {
            let tip = if name_is_truncated && subtitle_is_truncated {
                format!("{}\n{}", display_name, subtitle)
            } else if name_is_truncated {
                display_name.to_string()
            } else {
                subtitle.to_string()
            };
            tooltip(
                card_widget,
                text(tip).size(super::super::text_size(12)),
                tooltip::Position::Top,
            )
            .style(|theme: &Theme| {
                let p = theme.extended_palette();
                container::Style {
                    background: Some(p.background.strong.color.into()),
                    border: Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: p.background.weak.color,
                    },
                    ..Default::default()
                }
            })
            .into()
        } else {
            card_widget
        }
    }
}

/// Pick a color that works on both light and dark backgrounds.
fn theme_aware_color(is_light: bool, light_variant: Color, dark_variant: Color) -> Color {
    if is_light {
        dark_variant
    } else {
        light_variant
    }
}

/// Build a small stat box for the summary row
fn stat_box<'a>(
    label: &'a str,
    value: usize,
    color: Color,
    surface_bg: Color,
    surface_border: Color,
) -> Element<'a, Message> {
    let content = column![
        text(value.to_string())
            .size(super::super::text_size(22))
            .color(color)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            }),
        text(label)
            .size(super::super::text_size(13))
            .style(muted_text),
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
        .style(move |_theme: &Theme| container::Style {
            background: Some(surface_bg.into()),
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: surface_border,
            },
            ..Default::default()
        })
        .into()
}

/// Count badge element
fn count_badge<'a>(count: usize, accent: Color) -> Element<'a, Message> {
    container(
        text(count.to_string())
            .size(super::super::text_size(13))
            .color(text_on_color(accent))
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
    })
    .into()
}

/// Section header with category name and count badge
fn section_header<'a>(
    title: &'a str,
    count: usize,
    accent: Color,
    _is_light: bool,
) -> Element<'a, Message> {
    row![
        text(title)
            .size(super::super::text_size(17))
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        count_badge(count, accent),
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
        // Pad the last row with invisible spacers so cards stay left-aligned and same width
        let remaining = cols - row_cards.len();
        let mut row_children: Vec<Element<'a, Message>> = row_cards;
        for _ in 0..remaining {
            // Use an empty container with the same fixed width to maintain grid alignment
            row_children.push(
                Space::new()
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .into(),
            );
        }
        rows.push(
            Row::with_children(row_children)
                .spacing(CARD_GAP)
                .width(Length::Fill)
                .into(),
        );
    }
    Column::with_children(rows).spacing(CARD_GAP)
}

/// Extract just the filename from a path (e.g. "/usr/bin/node" -> "node")
fn basename(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(path)
}

/// Check whether a backend is "bound" (has at least one frontend route)
fn is_bound(backend_name: &str, routes: &[super::CachedRoute]) -> bool {
    routes.iter().any(|r| r.backend == backend_name)
}

/// Determine the accent color for a backend by looking it up in the cached config.
fn accent_for_backend(backend_name: &str, gui: &OddBoxGui) -> Color {
    if gui
        .cached_config
        .processes
        .iter()
        .any(|p| p.name == backend_name)
    {
        COLOR_PROCESS
    } else if gui
        .cached_config
        .remote_backends
        .iter()
        .any(|r| r.name == backend_name)
    {
        COLOR_REMOTE
    } else if gui
        .cached_config
        .static_backends
        .iter()
        .any(|s| s.name == backend_name)
    {
        COLOR_DIR_SERVER
    } else {
        Color::from_rgb(0.9, 0.3, 0.3) // missing backend
    }
}

/// Look up the state of a backend by name.
fn state_for_backend(backend_name: &str, gui: &OddBoxGui) -> ProcState {
    if let Some(p) = gui
        .cached_config
        .processes
        .iter()
        .find(|p| p.name == backend_name)
    {
        p.state.clone()
    } else if let Some(r) = gui
        .cached_config
        .remote_backends
        .iter()
        .find(|r| r.name == backend_name)
    {
        r.state.clone()
    } else if let Some(s) = gui
        .cached_config
        .static_backends
        .iter()
        .find(|s| s.name == backend_name)
    {
        s.state.clone()
    } else {
        ProcState::Faulty
    }
}

/// Build a subtitle describing the backend for a frontend card.
fn subtitle_for_backend(backend_name: &str, gui: &OddBoxGui) -> String {
    if let Some(p) = gui
        .cached_config
        .processes
        .iter()
        .find(|p| p.name == backend_name)
    {
        format!("⚙ {} · :{}", basename(&p.bin), p.port)
    } else if let Some(r) = gui
        .cached_config
        .remote_backends
        .iter()
        .find(|r| r.name == backend_name)
    {
        let proto = if r.https { "https" } else { "http" };
        format!("⇄ {}://{}", proto, r.endpoints)
    } else if let Some(s) = gui
        .cached_config
        .static_backends
        .iter()
        .find(|s| s.name == backend_name)
    {
        format!("📁 dir: {}", s.dir)
    } else {
        format!("⚠ missing: {}", backend_name)
    }
}

/// Whether a backend is a process (supports start/stop).
fn is_process_backend(backend_name: &str, gui: &OddBoxGui) -> bool {
    gui.cached_config
        .processes
        .iter()
        .any(|p| p.name == backend_name)
}

/// Build a browser URL for a given hostname using the configured ports.
fn build_url(hostname: &str, http_port: Option<u16>, https_port: Option<u16>) -> String {
    if let Some(port) = https_port {
        if port == 443 {
            format!("https://{}", hostname)
        } else {
            format!("https://{}:{}", hostname, port)
        }
    } else if let Some(port) = http_port {
        if port == 80 {
            format!("http://{}", hostname)
        } else {
            format!("http://{}:{}", hostname, port)
        }
    } else {
        format!("http://{}", hostname)
    }
}

/// Build action buttons for a frontend card.
/// Includes Open, Edit FE, Edit BE, and optionally Start/Stop for process backends.
fn frontend_actions<'a>(
    hostname: &str,
    backend_name: &str,
    backend_state: &ProcState,
    is_process: bool,
    is_light: bool,
    use_kde_buttons: bool,
    http_port: Option<u16>,
    https_port: Option<u16>,
) -> Vec<Element<'a, Message>> {
    let mut actions: Vec<Element<'a, Message>> = Vec::new();

    // Open in browser
    let open_bg = if is_light {
        Color::from_rgb(0.15, 0.50, 0.50)
    } else {
        Color::from_rgb(0.20, 0.58, 0.58)
    };
    let open_hover = if is_light {
        Color::from_rgb(0.12, 0.44, 0.44)
    } else {
        Color::from_rgb(0.28, 0.66, 0.66)
    };
    let url = build_url(hostname, http_port, https_port);
    actions.push(action_btn(
        "Open",
        Some(Message::OpenInBrowser(url)),
        open_bg,
        open_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Neutral,
    ));

    // Edit Backend
    let edit_bg = if is_light {
        Color::from_rgb(0.22, 0.42, 0.72)
    } else {
        Color::from_rgb(0.28, 0.48, 0.82)
    };
    let edit_hover = if is_light {
        Color::from_rgb(0.18, 0.36, 0.65)
    } else {
        Color::from_rgb(0.35, 0.55, 0.90)
    };
    actions.push(action_btn(
        "Edit BE",
        Some(Message::OpenEditBackend(backend_name.to_string())),
        edit_bg,
        edit_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Primary,
    ));

    // Edit Frontend
    let fe_bg = if is_light {
        Color::from_rgb(0.40, 0.40, 0.50)
    } else {
        Color::from_rgb(0.45, 0.45, 0.55)
    };
    let fe_hover = if is_light {
        Color::from_rgb(0.35, 0.35, 0.45)
    } else {
        Color::from_rgb(0.52, 0.52, 0.62)
    };
    actions.push(action_btn(
        "Edit FE",
        Some(Message::OpenEditFrontend(hostname.to_string())),
        fe_bg,
        fe_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Neutral,
    ));

    // Start / Stop for process backends
    if is_process {
        let can_start = matches!(backend_state, ProcState::Stopped | ProcState::Faulty);
        let can_stop = matches!(backend_state, ProcState::Running | ProcState::Faulty);
        let is_transitioning = matches!(backend_state, ProcState::Starting | ProcState::Stopping);

        if can_stop && !is_transitioning {
            let stop_bg = if is_light {
                Color::from_rgb(0.72, 0.20, 0.20)
            } else {
                Color::from_rgb(0.80, 0.28, 0.28)
            };
            let stop_hover = if is_light {
                Color::from_rgb(0.65, 0.15, 0.15)
            } else {
                Color::from_rgb(0.88, 0.33, 0.33)
            };
            actions.push(action_btn(
                "Stop",
                Some(Message::ProcessStop(backend_name.to_string())),
                stop_bg,
                stop_hover,
                Color::WHITE,
                use_kde_buttons,
                super::super::KdeButtonRole::Danger,
            ));
        } else if can_start && !is_transitioning {
            let start_bg = if is_light {
                Color::from_rgb(0.18, 0.52, 0.22)
            } else {
                Color::from_rgb(0.25, 0.62, 0.30)
            };
            let start_hover = if is_light {
                Color::from_rgb(0.14, 0.45, 0.18)
            } else {
                Color::from_rgb(0.32, 0.72, 0.38)
            };
            actions.push(action_btn(
                "Start",
                Some(Message::ProcessStart(backend_name.to_string())),
                start_bg,
                start_hover,
                Color::WHITE,
                use_kde_buttons,
                super::super::KdeButtonRole::Success,
            ));
        }
    }

    actions
}

/// Build action buttons for an unbound process-type backend
fn unbound_process_actions<'a>(
    backend_name: &str,
    state: &ProcState,
    is_light: bool,
    use_kde_buttons: bool,
) -> Vec<Element<'a, Message>> {
    let name = backend_name.to_string();
    let mut actions: Vec<Element<'a, Message>> = Vec::new();

    // Bind button — create a new frontend for this backend
    let bind_bg = if is_light {
        Color::from_rgb(0.15, 0.50, 0.50)
    } else {
        Color::from_rgb(0.20, 0.58, 0.58)
    };
    let bind_hover = if is_light {
        Color::from_rgb(0.12, 0.44, 0.44)
    } else {
        Color::from_rgb(0.28, 0.66, 0.66)
    };
    actions.push(action_btn(
        "Bind",
        Some(Message::OpenNewFrontendForBackend(name.clone())),
        bind_bg,
        bind_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Neutral,
    ));

    // Edit button
    let edit_bg = if is_light {
        Color::from_rgb(0.22, 0.42, 0.72)
    } else {
        Color::from_rgb(0.28, 0.48, 0.82)
    };
    let edit_hover = if is_light {
        Color::from_rgb(0.18, 0.36, 0.65)
    } else {
        Color::from_rgb(0.35, 0.55, 0.90)
    };
    actions.push(action_btn(
        "Edit",
        Some(Message::OpenEditBackend(name.clone())),
        edit_bg,
        edit_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Primary,
    ));

    // Manage button (navigate to processes page)
    let manage_bg = if is_light {
        Color::from_rgb(0.40, 0.40, 0.50)
    } else {
        Color::from_rgb(0.45, 0.45, 0.55)
    };
    let manage_hover = if is_light {
        Color::from_rgb(0.35, 0.35, 0.45)
    } else {
        Color::from_rgb(0.52, 0.52, 0.62)
    };
    actions.push(action_btn(
        "Manage",
        Some(Message::ManageProcess(name.clone())),
        manage_bg,
        manage_hover,
        Color::WHITE,
        use_kde_buttons,
        super::super::KdeButtonRole::Neutral,
    ));

    // Start / Stop
    let can_start = matches!(state, ProcState::Stopped | ProcState::Faulty);
    let can_stop = matches!(state, ProcState::Running | ProcState::Faulty);
    let is_transitioning = matches!(state, ProcState::Starting | ProcState::Stopping);

    if can_stop && !is_transitioning {
        let stop_bg = if is_light {
            Color::from_rgb(0.72, 0.20, 0.20)
        } else {
            Color::from_rgb(0.80, 0.28, 0.28)
        };
        let stop_hover = if is_light {
            Color::from_rgb(0.65, 0.15, 0.15)
        } else {
            Color::from_rgb(0.88, 0.33, 0.33)
        };
        actions.push(action_btn(
            "Stop",
            Some(Message::ProcessStop(name)),
            stop_bg,
            stop_hover,
            Color::WHITE,
            use_kde_buttons,
            super::super::KdeButtonRole::Danger,
        ));
    } else if can_start && !is_transitioning {
        let start_bg = if is_light {
            Color::from_rgb(0.18, 0.52, 0.22)
        } else {
            Color::from_rgb(0.25, 0.62, 0.30)
        };
        let start_hover = if is_light {
            Color::from_rgb(0.14, 0.45, 0.18)
        } else {
            Color::from_rgb(0.32, 0.72, 0.38)
        };
        actions.push(action_btn(
            "Start",
            Some(Message::ProcessStart(name)),
            start_bg,
            start_hover,
            Color::WHITE,
            use_kde_buttons,
            super::super::KdeButtonRole::Success,
        ));
    } else {
        // Transitioning – show disabled
        let disabled_bg = Color::from_rgb(0.4, 0.4, 0.4);
        let label = if matches!(state, ProcState::Starting) {
            "Starting…"
        } else {
            "Stopping…"
        };
        actions.push(action_btn(
            label,
            None,
            disabled_bg,
            disabled_bg,
            Color::WHITE,
            use_kde_buttons,
            super::super::KdeButtonRole::Neutral,
        ));
    }

    actions
}

/// Build action buttons for an unbound remote or static backend (no start/stop)
fn unbound_simple_actions<'a>(
    backend_name: &str,
    is_light: bool,
    use_kde_buttons: bool,
) -> Vec<Element<'a, Message>> {
    let name = backend_name.to_string();

    // Bind button — create a new frontend for this backend
    let bind_bg = if is_light {
        Color::from_rgb(0.15, 0.50, 0.50)
    } else {
        Color::from_rgb(0.20, 0.58, 0.58)
    };
    let bind_hover = if is_light {
        Color::from_rgb(0.12, 0.44, 0.44)
    } else {
        Color::from_rgb(0.28, 0.66, 0.66)
    };

    let edit_bg = if is_light {
        Color::from_rgb(0.22, 0.42, 0.72)
    } else {
        Color::from_rgb(0.28, 0.48, 0.82)
    };
    let edit_hover = if is_light {
        Color::from_rgb(0.18, 0.36, 0.65)
    } else {
        Color::from_rgb(0.35, 0.55, 0.90)
    };
    vec![
        action_btn(
            "Bind",
            Some(Message::OpenNewFrontendForBackend(name.clone())),
            bind_bg,
            bind_hover,
            Color::WHITE,
            use_kde_buttons,
            super::super::KdeButtonRole::Neutral,
        ),
        action_btn(
            "Edit",
            Some(Message::OpenEditBackend(name)),
            edit_bg,
            edit_hover,
            Color::WHITE,
            use_kde_buttons,
            super::super::KdeButtonRole::Primary,
        ),
    ]
}

impl OddBoxGui {
    pub(in crate::gui) fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| {
                let total_secs = d.as_secs();
                let days = total_secs / 86400;
                let hours = (total_secs % 86400) / 3600;
                let mins = (total_secs % 3600) / 60;
                let secs = total_secs % 60;
                if days > 0 {
                    format!("{}d {}h {}m {}s", days, hours, mins, secs)
                } else if hours > 0 {
                    format!("{}h {}m {}s", hours, mins, secs)
                } else if mins > 0 {
                    format!("{}m {}s", mins, secs)
                } else {
                    format!("{}s", secs)
                }
            })
            .unwrap_or_else(|_| "Unknown".to_string());
        let theme_snapshot = self.theme();
        let dashboard_surface_bg = self.surface_panel_bg(&theme_snapshot);
        let dashboard_surface_border = self.surface_border_color(&theme_snapshot);

        let dashboard_content: Element<'_, Message> = responsive(move |size| {
            let (cols, card_width) = card_layout(size);
            let use_kde_buttons = self.use_kde_system_styles();

            // Detect light theme for theme-aware colors
            let is_light = match self.theme_mode {
                super::super::ThemeMode::Light => true,
                super::super::ThemeMode::Dark => false,
                super::super::ThemeMode::System => matches!(self.system_theme, Some(theme::Mode::Light)),
            };

            let status_green = theme_aware_color(
                is_light,
                Color::from_rgb(0.4, 0.8, 0.4),
                Color::from_rgb(0.15, 0.55, 0.15),
            );

            // --- Uptime bar ---
            let http_port = self.cached_config.http_port;
            let https_port = self.cached_config.https_port;

            let mut uptime_row = Row::new()
                .spacing(6)
                .align_y(Alignment::Center)
                .push(text("●").color(status_green).size(super::super::text_size(14)))
                .push(
                    text("Status: Running")
                        .size(super::super::text_size(15))
                        .color(status_green),
                )
                .push(
                    text(format!("  Uptime: {}", &uptime))
                        .size(super::super::text_size(15))
                        .style(muted_text),
                )
                .push(Space::new().width(Length::Fill));

            if let Some(port) = http_port {
                uptime_row = uptime_row.push(
                    text(format!("HTTP :{}", port))
                        .size(super::super::text_size(13))
                        .style(muted_text),
                );
            }
            if http_port.is_some() && https_port.is_some() {
                uptime_row = uptime_row.push(
                    text("·")
                        .size(super::super::text_size(13))
                        .style(muted_text),
                );
            }
            if let Some(port) = https_port {
                uptime_row = uptime_row.push(
                    text(format!("HTTPS :{}", port))
                        .size(super::super::text_size(13))
                        .style(muted_text),
                );
            }

            let uptime_bar = container(uptime_row)
            .padding(14)
            .width(Length::Fill)
            .style(move |_theme: &Theme| {
                container::Style {
                    background: Some(dashboard_surface_bg.into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: dashboard_surface_border,
                    },
                    ..Default::default()
                }
            });

            // Route helpers
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

            // Summary counts
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

            let sites_color = theme_aware_color(
                is_light,
                Color::WHITE,
                Color::from_rgb(0.15, 0.15, 0.25),
            );
            let running_color = theme_aware_color(
                is_light,
                Color::from_rgb(0.4, 0.8, 0.4),
                Color::from_rgb(0.15, 0.55, 0.15),
            );
            let stopped_color = theme_aware_color(
                is_light,
                Color::from_rgb(0.6, 0.6, 0.6),
                Color::from_rgb(0.35, 0.35, 0.35),
            );
            let faulty_color = theme_aware_color(
                is_light,
                Color::from_rgb(0.9, 0.3, 0.3),
                Color::from_rgb(0.75, 0.15, 0.15),
            );

            let summary_row = Row::with_children(vec![
                stat_box(
                    "Sites",
                    total,
                    sites_color,
                    dashboard_surface_bg,
                    dashboard_surface_border,
                ),
                stat_box(
                    "Running",
                    running,
                    running_color,
                    dashboard_surface_bg,
                    dashboard_surface_border,
                ),
                stat_box(
                    "Stopped",
                    stopped,
                    stopped_color,
                    dashboard_surface_bg,
                    dashboard_surface_border,
                ),
                stat_box(
                    "Faulty",
                    faulty,
                    faulty_color,
                    dashboard_surface_bg,
                    dashboard_surface_border,
                ),
            ])
            .spacing(12);

            let selected = self.dashboard_process_menu.as_deref();
            let routes = &self.cached_config.routes;

            let mut sections: Vec<Element<'_, Message>> = Vec::new();

            // ─── Frontends section ───
            // Each card represents a frontend route, showing hostname + backend info.
            {
                // Only include routes whose backends actually exist
                let valid_routes: Vec<_> = routes.iter()
                    .filter(|r| backend_exists(&r.backend))
                    .collect();

                if !valid_routes.is_empty() {
                    let cruma_global = self.cached_config.cruma_globally_enabled;
                    let cruma_domain = self.cached_config.cruma_assigned_domain.as_deref();
                    let cards: Vec<Element<'_, Message>> = valid_routes.iter().map(|route| {
                        let accent = accent_for_backend(&route.backend, self);
                        let state = state_for_backend(&route.backend, self);
                        let mut subtitle = subtitle_for_backend(&route.backend, self);

                        // Append the resolved cruma FQDN to the subtitle when
                        // cruma is globally enabled and this route opts in.
                        if cruma_global && route.enable_cruma {
                            if let Some(domain) = cruma_domain {
                                let host = route.hostname.trim();
                                let resolved = if host == "@" || host.is_empty() {
                                    domain.to_string()
                                } else if host == "*" {
                                    "*".to_string()
                                } else if host.contains('@') {
                                    host.replace('@', domain)
                                } else if host.contains('.') {
                                    host.to_string()
                                } else {
                                    format!("{}.{}", host, domain)
                                };
                                // Only append the resolved domain when it differs
                                // from the hostname to avoid redundant display.
                                if resolved != host {
                                    subtitle = format!("{} · 👻 {}", subtitle, resolved);
                                }
                            }
                        }
                        // Use the route's hostname as the selection key (unique per route)
                        let selection_key = route.hostname.clone();
                        let is_sel = selected == Some(selection_key.as_str());
                        let actions = if is_sel {
                            let is_proc = is_process_backend(&route.backend, self);
                            frontend_actions(
                                &route.hostname,
                                &route.backend,
                                &state,
                                is_proc,
                                is_light,
                                use_kde_buttons,
                                http_port,
                                https_port,
                            )
                        } else {
                            vec![]
                        };
                        // Mark cruma-enabled routes with a ghost emoji when cruma
                        // is also enabled in the global configuration.
                        let display_name = if cruma_global && route.enable_cruma {
                            format!("👻 {}", route.hostname)
                        } else {
                            route.hostname.clone()
                        };
                        site_card(
                            &display_name,
                            &subtitle,
                            accent,
                            dashboard_surface_bg,
                            dashboard_surface_border,
                            &state,
                            card_width,
                            is_sel,
                            actions,
                            Some(Message::DashboardToggleProcessMenu(selection_key)),
                        )
                    }).collect();

                    let frontend_accent = theme_aware_color(
                        is_light,
                        Color::from_rgb(0.5, 0.7, 1.0),
                        Color::from_rgb(0.2, 0.4, 0.7),
                    );

                    sections.push(
                        column![
                            section_header("Frontends", valid_routes.len(), frontend_accent, is_light),
                            rows_from_cards(cards, cols),
                        ]
                        .spacing(10)
                        .into(),
                    );
                }
            }

            // ─── Orphan routes (pointing to missing backends) ───
            let orphan_cards: Vec<Element<'_, Message>> = self
                .cached_config
                .routes
                .iter()
                .filter(|r| !backend_exists(&r.backend))
                .map(|route| {
                    site_card(
                        &route.hostname,
                        &format!("Missing backend: {}", route.backend),
                        Color::from_rgb(0.9, 0.3, 0.3),
                        dashboard_surface_bg,
                        dashboard_surface_border,
                        &ProcState::Faulty,
                        card_width,
                        false,
                        vec![],
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
                            Color::from_rgb(0.9, 0.3, 0.3),
                            is_light,
                        ),
                        rows_from_cards(orphan_cards, cols),
                    ]
                    .spacing(10)
                    .into(),
                );
            }

            // ─── Unbound backends section ───
            // Collect unbound backends across all types
            let mut unbound_cards: Vec<Element<'_, Message>> = Vec::new();

            // Unbound process backends
            for proc in &self.cached_config.processes {
                if !is_bound(&proc.name, routes) {
                    let is_sel = selected == Some(proc.name.as_str());
                    let actions = if is_sel {
                        unbound_process_actions(&proc.name, &proc.state, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let subtitle = format!("{} · :{}", basename(&proc.bin), proc.port);
                    unbound_cards.push(site_card(
                        &proc.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        dashboard_surface_bg,
                        dashboard_surface_border,
                        &proc.state,
                        card_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(proc.name.clone())),
                    ));
                }
            }

            // Unbound remote backends
            for remote in &self.cached_config.remote_backends {
                if !is_bound(&remote.name, routes) {
                    let is_sel = selected == Some(remote.name.as_str());
                    let actions = if is_sel {
                        unbound_simple_actions(&remote.name, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let proto = if remote.https { "https" } else { "http" };
                    let subtitle = format!("{}://{}", proto, remote.endpoints);
                    unbound_cards.push(site_card(
                        &remote.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        dashboard_surface_bg,
                        dashboard_surface_border,
                        &remote.state,
                        card_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(remote.name.clone())),
                    ));
                }
            }

            // Unbound static backends
            for sb in &self.cached_config.static_backends {
                if !is_bound(&sb.name, routes) {
                    let is_sel = selected == Some(sb.name.as_str());
                    let actions = if is_sel {
                        unbound_simple_actions(&sb.name, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let subtitle = format!("dir: {}", sb.dir);
                    unbound_cards.push(site_card(
                        &sb.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        dashboard_surface_bg,
                        dashboard_surface_border,
                        &sb.state,
                        card_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(sb.name.clone())),
                    ));
                }
            }

            if !unbound_cards.is_empty() {
                let unbound_count = unbound_cards.len();

                // Info text explaining what unbound means
                let info_text = container(
                    text("These backends have no frontend route mapped to them. They may need a hostname binding, or can be removed if unused.")
                        .size(super::super::text_size(12))
                        .wrapping(Wrapping::Word)
                        .style(muted_text),
                )
                .width(Length::Fill)
                .padding(Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 4.0,
                    left: 0.0,
                });

                sections.push(
                    column![
                        section_header(
                            "Unbound Backends",
                            unbound_count,
                            COLOR_UNBOUND,
                            is_light,
                        ),
                        info_text,
                        rows_from_cards(unbound_cards, cols),
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

            let area = mouse_area(dashboard_column)
                .on_move(|p| Message::DashboardCursorMoved(p.x, p.y));

            // When a card menu is open, clicking outside any card dismisses it.
            // mouse_area only fires on_press when no child widget captured the event,
            // so clicking a button inside a card won't trigger this.
            if self.dashboard_process_menu.is_some() {
                area.on_press(Message::DashboardDismissMenu).into()
            } else {
                area.into()
            }
        })
        .into();
        dashboard_content
    }
}
