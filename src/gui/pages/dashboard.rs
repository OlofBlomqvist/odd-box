use iced::theme;
use iced::widget::text::Wrapping;
use iced::widget::{
    Column, Row, Scrollable, Space, button, column, container, mouse_area, responsive, row, text,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme, mouse};

use cruma_tunnels_lib::hostname::{
    HostnamePatternKind, classify_hostname_pattern, resolve_display_url,
};

use crate::global_state::ProcState;

use super::super::{KdeButtonRole, Message, OddBoxGui};

/// Category accent colors
const COLOR_PROCESS: Color = Color::from_rgb(0.4, 0.6, 1.0);
const COLOR_REMOTE: Color = Color::from_rgb(0.7, 0.5, 1.0);
const COLOR_DIR_SERVER: Color = Color::from_rgb(0.3, 0.8, 0.7);
const COLOR_UNBOUND: Color = Color::from_rgb(0.75, 0.55, 0.25);
const DASHBOARD_SPLITTER_HIT_WIDTH: f32 = 10.0;
const DASHBOARD_MIN_LIST_WIDTH: f32 = 240.0;
const DASHBOARD_MIN_LOGS_WIDTH: f32 = 620.0;
const DASHBOARD_TWO_COLUMN_MIN_WIDTH: f32 = 930.0;

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

fn truncate_to_fit(text: &str, available_px: f32, font_size_px: f32) -> String {
    let char_w = (font_size_px * 0.58).max(1.0);
    let max_chars = (available_px / char_w).floor() as usize;
    if max_chars == 0 || text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }

    let mut truncated = String::with_capacity(max_chars);
    for ch in text.chars().take(max_chars - 1) {
        truncated.push(ch);
    }
    truncated.push('…');
    truncated
}

/// Selected list row container style
fn selected_list_item_style(_theme: &Theme, accent: Color, surface_bg: Color) -> container::Style {
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

/// Build a compact action button for row actions.
/// Estimate the pixel width of an action button from its label.
fn estimate_btn_width(label: &str) -> f32 {
    let scale = super::super::gui_scale();
    let char_w = 0.65 * 11.0 * scale; // approximate char width for bold size-11 font
    let h_pad = 20.0; // 10px padding on each side
    char_w * label.len() as f32 + h_pad
}

fn action_btn<'a>(
    label: &'a str,
    msg: Option<Message>,
    bg_normal: Color,
    bg_hover: Color,
    fg: Color,
    use_kde_buttons: bool,
    kde_role: KdeButtonRole,
) -> (f32, Element<'a, Message>) {
    let estimated_width = estimate_btn_width(label);

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

    (estimated_width, btn.into())
}

/// Build one list row for a site/backend entry.
fn site_list_row<'a>(
    display_name: &str,
    subtitle: &str,
    accent_color: Color,
    surface_bg: Color,
    surface_border: Color,
    state: &ProcState,
    action_wrap_width: f32,
    is_selected: bool,
    actions: Vec<(f32, Element<'a, Message>)>,
    on_press: Option<Message>,
    hover_key: Option<String>,
    is_hovered: bool,
    quick_action: Option<(f32, Element<'a, Message>)>,
    quick_action_slot_width: Option<f32>,
) -> Element<'a, Message> {
    let dot_color = status_color(state);
    let name_font_size = super::super::text_size(14).min(16.0);
    let subtitle_font_size = super::super::text_size(11).min(13.0);
    let quick_action_reserved = quick_action_slot_width
        .or_else(|| quick_action.as_ref().map(|(width, _)| *width))
        .map(|width| width + 12.0)
        .unwrap_or(0.0);
    let name_text_width = (action_wrap_width - 60.0 - quick_action_reserved).max(90.0);
    let display_name = truncate_to_fit(display_name, name_text_width, name_font_size);

    let name_row = row![
        text("●").color(dot_color).size(name_font_size),
        container(
            text(display_name)
                .size(name_font_size)
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

    let subtitle_widget = text(subtitle.to_string())
        .size(subtitle_font_size)
        .wrapping(Wrapping::WordOrGlyph)
        .style(muted_text);

    if is_selected {
        let content_width = (action_wrap_width - 32.0).max(120.0);
        let btn_spacing = 4.0_f32;

        let mut rows_vec: Vec<Vec<Element<'a, Message>>> = vec![vec![]];
        let mut current_row_width = 0.0_f32;

        for (est_btn_w, action) in actions {
            let needed = if current_row_width > 0.0 {
                btn_spacing + est_btn_w
            } else {
                est_btn_w
            };

            if current_row_width + needed > content_width && current_row_width > 0.0 {
                rows_vec.push(vec![action]);
                current_row_width = est_btn_w;
            } else {
                current_row_width += needed;
                rows_vec
                    .last_mut()
                    .expect("rows_vec is never empty")
                    .push(action);
            }
        }

        let mut actions_col = Column::new().spacing(4);
        for row_items in rows_vec {
            actions_col = actions_col.push(
                Row::with_children(row_items)
                    .spacing(4)
                    .align_y(Alignment::Center),
            );
        }

        let row_content = column![name_row, subtitle_widget, actions_col]
            .spacing(8)
            .width(Length::Fill);

        container(row_content)
            .padding(Padding {
                top: 10.0,
                right: 14.0,
                bottom: 10.0,
                left: 14.0,
            })
            .width(Length::Fill)
            .style(move |theme: &Theme| selected_list_item_style(theme, accent_color, surface_bg))
            .into()
    } else {
        let mut heading_row = Row::new()
            .push(name_row)
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Fill);
        if let Some((_, action)) = quick_action {
            heading_row = heading_row.push(action);
        } else if let Some(slot_width) = quick_action_slot_width {
            heading_row = heading_row.push(
                Space::new()
                    .width(Length::Fixed(slot_width))
                    .height(Length::Fixed(super::super::scaled(22.0))),
            );
        }

        let row_content = column![heading_row, subtitle_widget]
            .spacing(5)
            .width(Length::Fill);

        let border_color = if is_hovered {
            accent_color
        } else {
            surface_border
        };
        let border_width = if is_hovered { 2.0 } else { 1.0 };

        let row_surface = container(row_content)
            .padding(Padding {
                top: 10.0,
                right: 14.0,
                bottom: 10.0,
                left: 14.0,
            })
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(surface_bg.into()),
                border: Border {
                    radius: 8.0.into(),
                    width: border_width,
                    color: border_color,
                },
                ..Default::default()
            });

        let mut row_btn = mouse_area(row_surface).on_exit(Message::DashboardSetHoveredRow(None));
        if let Some(row_key) = hover_key {
            row_btn = row_btn.on_move(move |_p| Message::DashboardSetHoveredRow(Some(row_key.clone())));
        }
        if let Some(msg) = on_press {
            row_btn = row_btn.on_press(msg);
        }

        row_btn.into()
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
fn section_header<'a>(title: &'a str, count: usize, accent: Color) -> Element<'a, Message> {
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

/// Build a subtitle describing the backend for a frontend row.
fn subtitle_for_backend(backend_name: &str, gui: &OddBoxGui) -> String {
    if let Some(p) = gui
        .cached_config
        .processes
        .iter()
        .find(|p| p.name == backend_name)
    {
        format!("proc {} · :{}", basename(&p.bin), p.port)
    } else if let Some(r) = gui
        .cached_config
        .remote_backends
        .iter()
        .find(|r| r.name == backend_name)
    {
        let proto = if r.https { "https" } else { "http" };
        format!("remote {}://{}", proto, r.endpoints)
    } else if let Some(s) = gui
        .cached_config
        .static_backends
        .iter()
        .find(|s| s.name == backend_name)
    {
        format!("dir: {}", s.dir)
    } else {
        format!("missing: {}", backend_name)
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

/// Build the frontend "Open" URL.
///
/// When a route is materialized through cruma, prefer the SDK-resolved URL so
/// shorthand hostnames (for example a single label) open at the assigned
/// tunnel domain.
fn build_frontend_open_url(
    hostname: &str,
    http_port: Option<u16>,
    https_port: Option<u16>,
    route_uses_cruma: bool,
    cruma_assigned_domain: Option<&str>,
) -> String {
    if route_uses_cruma {
        let assigned = cruma_assigned_domain.map(|d| d.to_string());
        if let Some(url) = resolve_display_url(hostname, &assigned) {
            return url;
        }
    }

    build_url(hostname, http_port, https_port)
}

fn frontend_process_control_action<'a>(
    backend_name: &str,
    backend_state: &ProcState,
    is_light: bool,
    use_kde_buttons: bool,
) -> Option<(f32, Element<'a, Message>)> {
    let can_start = matches!(backend_state, ProcState::Stopped | ProcState::Faulty);
    let can_stop = matches!(backend_state, ProcState::Running | ProcState::Faulty);
    let is_transitioning = matches!(backend_state, ProcState::Starting | ProcState::Stopping);

    if is_transitioning {
        let disabled_bg = Color::from_rgb(0.4, 0.4, 0.4);
        let label = if matches!(backend_state, ProcState::Starting) {
            "Starting..."
        } else {
            "Stopping..."
        };
        return Some(action_btn(
            label,
            None,
            disabled_bg,
            disabled_bg,
            Color::WHITE,
            use_kde_buttons,
            KdeButtonRole::Neutral,
        ));
    }

    if can_stop {
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
        Some(action_btn(
            "Stop",
            Some(Message::ProcessStop(backend_name.to_string())),
            stop_bg,
            stop_hover,
            Color::WHITE,
            use_kde_buttons,
            KdeButtonRole::Danger,
        ))
    } else if can_start {
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
        Some(action_btn(
            "Start",
            Some(Message::ProcessStart(backend_name.to_string())),
            start_bg,
            start_hover,
            Color::WHITE,
            use_kde_buttons,
            KdeButtonRole::Success,
        ))
    } else {
        None
    }
}

/// Build action buttons for a frontend row.
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
    route_uses_cruma: bool,
    cruma_assigned_domain: Option<&str>,
) -> Vec<(f32, Element<'a, Message>)> {
    let mut actions: Vec<(f32, Element<'a, Message>)> = Vec::new();

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
    let url = build_frontend_open_url(
        hostname,
        http_port,
        https_port,
        route_uses_cruma,
        cruma_assigned_domain,
    );
    actions.push(action_btn(
        "Open",
        Some(Message::OpenInBrowser(url)),
        open_bg,
        open_hover,
        Color::WHITE,
        use_kde_buttons,
        KdeButtonRole::Neutral,
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
        KdeButtonRole::Primary,
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
        KdeButtonRole::Neutral,
    ));

    // Start / Stop for process backends
    if is_process
        && let Some(start_stop_btn) =
            frontend_process_control_action(backend_name, backend_state, is_light, use_kde_buttons)
    {
        actions.push(start_stop_btn);
    }

    actions
}

/// Build action buttons for an unbound process-type backend
fn unbound_process_actions<'a>(
    backend_name: &str,
    state: &ProcState,
    is_light: bool,
    use_kde_buttons: bool,
) -> Vec<(f32, Element<'a, Message>)> {
    let name = backend_name.to_string();
    let mut actions: Vec<(f32, Element<'a, Message>)> = Vec::new();

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
        KdeButtonRole::Neutral,
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
        KdeButtonRole::Primary,
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
        KdeButtonRole::Neutral,
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
            KdeButtonRole::Danger,
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
            KdeButtonRole::Success,
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
            KdeButtonRole::Neutral,
        ));
    }

    actions
}

/// Build action buttons for an unbound remote or static backend (no start/stop)
fn unbound_simple_actions<'a>(
    backend_name: &str,
    is_light: bool,
    use_kde_buttons: bool,
) -> Vec<(f32, Element<'a, Message>)> {
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
            KdeButtonRole::Neutral,
        ),
        action_btn(
            "Edit",
            Some(Message::OpenEditBackend(name)),
            edit_bg,
            edit_hover,
            Color::WHITE,
            use_kde_buttons,
            KdeButtonRole::Primary,
        ),
    ]
}

impl OddBoxGui {
    pub(in crate::gui) fn view_dashboard(&self) -> Element<'_, Message> {
        let theme_snapshot = self.theme();
        let list_panel_bg = self.surface_panel_bg(&theme_snapshot);
        let list_item_bg = self.surface_panel_alt_bg(&theme_snapshot);
        let logs_panel_bg = self.surface_panel_bg(&theme_snapshot);
        let logs_entries_bg = {
            let base_bg = theme_snapshot.extended_palette().background.base.color;
            if theme_snapshot.extended_palette().is_dark {
                theme::palette::mix(base_bg, Color::BLACK, 0.33)
            } else {
                base_bg
            }
        };
        let panel_border = self.surface_border_color(&theme_snapshot);

        let dashboard_content: Element<'_, Message> = responsive(move |size| {
            let use_kde_buttons = self.use_kde_system_styles();
            let total_width = size.width.max(1.0);
            let show_observations = total_width >= DASHBOARD_TWO_COLUMN_MIN_WIDTH;
            let (list_width, logs_width) = if show_observations {
                let available_width = (total_width - DASHBOARD_SPLITTER_HIT_WIDTH).max(1.0);
                let min_list_width = DASHBOARD_MIN_LIST_WIDTH.min(available_width * 0.7);
                let min_logs_width = DASHBOARD_MIN_LOGS_WIDTH.min(available_width * 0.7);
                let max_list_width = (available_width - min_logs_width).max(available_width * 0.3);
                let min_list_width = min_list_width.min(max_list_width);
                let list_width = (available_width * self.dashboard_split_ratio)
                    .clamp(min_list_width, max_list_width);
                let logs_width = (available_width - list_width).max(1.0);
                (list_width, logs_width)
            } else {
                (total_width, 0.0)
            };
            let list_action_wrap_width = (list_width - 32.0).max(220.0);

            // Detect light theme for theme-aware colors
            let is_light = match self.theme_mode {
                super::super::ThemeMode::Light => true,
                super::super::ThemeMode::Dark => false,
                super::super::ThemeMode::System => {
                    matches!(self.system_theme, Some(theme::Mode::Light))
                }
            };

            // Route helpers
            let backend_exists = |name: &str| -> bool {
                self.cached_config.processes.iter().any(|p| p.name == name)
                    || self.cached_config.remote_backends.iter().any(|r| r.name == name)
                    || self.cached_config.static_backends.iter().any(|s| s.name == name)
            };

            let selected = self.dashboard_process_menu.as_deref();
            let routes = &self.cached_config.routes;

            let mut list_sections: Vec<Element<'_, Message>> = Vec::new();

            // Frontends section
            {
                let valid_routes: Vec<_> = routes.iter().filter(|r| backend_exists(&r.backend)).collect();

                if !valid_routes.is_empty() {
                    let cruma_global = self.cached_config.cruma_globally_enabled;
                    let cruma_domain = self.cached_config.cruma_assigned_domain.as_deref();
                    let list_rows: Vec<Element<'_, Message>> = valid_routes
                        .iter()
                        .map(|route| {
                            let accent = accent_for_backend(&route.backend, self);
                            let state = state_for_backend(&route.backend, self);
                            let mut subtitle = subtitle_for_backend(&route.backend, self);

                            // Append resolved cruma FQDN to subtitle when applicable.
                            if cruma_global && route.enable_cruma {
                                let assigned = cruma_domain.map(|d| d.to_string());
                                let info = classify_hostname_pattern(&route.hostname, &assigned);
                                match info.kind {
                                    HostnamePatternKind::Invalid => {}
                                    HostnamePatternKind::Pending => {
                                        subtitle = format!("{} · cruma {}", subtitle, info.display_label);
                                    }
                                    _ => {
                                        let host = route.hostname.trim();
                                        if info.display_label != host {
                                            subtitle =
                                                format!("{} · cruma {}", subtitle, info.display_label);
                                        }
                                    }
                                }
                            }

                            let selection_key = format!("route:{}", route.hostname);
                            let is_sel = selected == Some(selection_key.as_str());
                            let is_proc = is_process_backend(&route.backend, self);
                            let is_hovered =
                                self.dashboard_hovered_row.as_deref() == Some(selection_key.as_str());
                            let actions = if is_sel {
                                let route_uses_cruma = cruma_global && route.enable_cruma;
                                frontend_actions(
                                    &route.hostname,
                                    &route.backend,
                                    &state,
                                    is_proc,
                                    is_light,
                                    use_kde_buttons,
                                    self.cached_config.http_port,
                                    self.cached_config.https_port,
                                    route_uses_cruma,
                                    cruma_domain,
                                )
                            } else {
                                vec![]
                            };
                            let mut quick_action_candidate = if !is_sel && is_proc {
                                frontend_process_control_action(
                                    &route.backend,
                                    &state,
                                    is_light,
                                    use_kde_buttons,
                                )
                            } else {
                                None
                            };
                            let quick_action_slot_width =
                                quick_action_candidate.as_ref().map(|(width, _)| *width);
                            let quick_action = if is_hovered {
                                quick_action_candidate.take()
                            } else {
                                None
                            };

                            let display_name = if cruma_global && route.enable_cruma {
                                format!("[cruma] {}", route.hostname)
                            } else {
                                route.hostname.clone()
                            };

                            site_list_row(
                                &display_name,
                                &subtitle,
                                accent,
                                list_item_bg,
                                panel_border,
                                &state,
                                list_action_wrap_width,
                                is_sel,
                                actions,
                                Some(Message::DashboardToggleProcessMenu(selection_key.clone())),
                                Some(selection_key),
                                is_hovered,
                                quick_action,
                                quick_action_slot_width,
                            )
                        })
                        .collect();

                    let frontend_accent = theme_aware_color(
                        is_light,
                        Color::from_rgb(0.5, 0.7, 1.0),
                        Color::from_rgb(0.2, 0.4, 0.7),
                    );

                    list_sections.push(
                        column![
                            section_header("Frontends", valid_routes.len(), frontend_accent),
                            Column::with_children(list_rows).spacing(6),
                        ]
                        .spacing(8)
                        .width(Length::Fill)
                        .into(),
                    );
                }
            }

            // Faulty routes (pointing to missing backends)
            let faulty_rows: Vec<Element<'_, Message>> = self
                .cached_config
                .routes
                .iter()
                .filter(|r| !backend_exists(&r.backend))
                .map(|route| {
                    site_list_row(
                        &route.hostname,
                        &format!("Missing backend: {}", route.backend),
                        Color::from_rgb(0.9, 0.3, 0.3),
                        list_item_bg,
                        panel_border,
                        &ProcState::Faulty,
                        list_action_wrap_width,
                        false,
                        vec![],
                        Some(Message::OpenEditFrontend(route.hostname.clone())),
                        None,
                        false,
                        None,
                        None,
                    )
                })
                .collect();
            if !faulty_rows.is_empty() {
                let count = faulty_rows.len();
                list_sections.push(
                    column![
                        section_header("Faulty Routes", count, Color::from_rgb(0.9, 0.3, 0.3)),
                        Column::with_children(faulty_rows).spacing(6),
                    ]
                    .spacing(8)
                    .width(Length::Fill)
                    .into(),
                );
            }

            // Unbound backends section
            let mut unbound_rows: Vec<Element<'_, Message>> = Vec::new();

            // Unbound process backends
            for proc in &self.cached_config.processes {
                if !is_bound(&proc.name, routes) {
                    let selection_key = format!("unbound:proc:{}", proc.name);
                    let is_sel = selected == Some(selection_key.as_str());
                    let actions = if is_sel {
                        unbound_process_actions(&proc.name, &proc.state, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let subtitle = format!("{} · :{}", basename(&proc.bin), proc.port);
                    unbound_rows.push(site_list_row(
                        &proc.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        list_item_bg,
                        panel_border,
                        &proc.state,
                        list_action_wrap_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(selection_key)),
                        None,
                        false,
                        None,
                        None,
                    ));
                }
            }

            // Unbound remote backends
            for remote in &self.cached_config.remote_backends {
                if !is_bound(&remote.name, routes) {
                    let selection_key = format!("unbound:remote:{}", remote.name);
                    let is_sel = selected == Some(selection_key.as_str());
                    let actions = if is_sel {
                        unbound_simple_actions(&remote.name, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let proto = if remote.https { "https" } else { "http" };
                    let subtitle = format!("{}://{}", proto, remote.endpoints);
                    unbound_rows.push(site_list_row(
                        &remote.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        list_item_bg,
                        panel_border,
                        &remote.state,
                        list_action_wrap_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(selection_key)),
                        None,
                        false,
                        None,
                        None,
                    ));
                }
            }

            // Unbound static backends
            for sb in &self.cached_config.static_backends {
                if !is_bound(&sb.name, routes) {
                    let selection_key = format!("unbound:static:{}", sb.name);
                    let is_sel = selected == Some(selection_key.as_str());
                    let actions = if is_sel {
                        unbound_simple_actions(&sb.name, is_light, use_kde_buttons)
                    } else {
                        vec![]
                    };
                    let subtitle = format!("dir: {}", sb.dir);
                    unbound_rows.push(site_list_row(
                        &sb.name,
                        &subtitle,
                        COLOR_UNBOUND,
                        list_item_bg,
                        panel_border,
                        &sb.state,
                        list_action_wrap_width,
                        is_sel,
                        actions,
                        Some(Message::DashboardToggleProcessMenu(selection_key)),
                        None,
                        false,
                        None,
                        None,
                    ));
                }
            }

            if !unbound_rows.is_empty() {
                let unbound_count = unbound_rows.len();
                let info_text = text(
                    "These backends have no frontend route mapped to them. They may need a hostname binding, or can be removed if unused.",
                )
                .size(super::super::text_size(12))
                .wrapping(Wrapping::Word)
                .style(muted_text);

                list_sections.push(
                    column![
                        section_header("Unbound Backends", unbound_count, COLOR_UNBOUND),
                        info_text,
                        Column::with_children(unbound_rows).spacing(6),
                    ]
                    .spacing(8)
                    .width(Length::Fill)
                    .into(),
                );
            }

            let list_content: Element<'_, Message> = if list_sections.is_empty() {
                column![
                    text("No frontends or backends configured yet.")
                        .size(super::super::text_size(13))
                        .style(muted_text)
                ]
                .width(Length::Fill)
                .into()
            } else {
                Column::with_children(list_sections)
                    .spacing(14)
                    .width(Length::Fill)
                    .into()
            };

            let list_scroller = Scrollable::new(
                container(list_content)
                    .padding(Padding {
                        top: 14.0,
                        right: 20.0,
                        bottom: 14.0,
                        left: 14.0,
                    })
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill);

            let list_pane = container(list_scroller)
                .width(if show_observations {
                    Length::Fixed(list_width)
                } else {
                    Length::Fill
                })
                .height(Length::Fill)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(list_panel_bg.into()),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: panel_border,
                    },
                    ..Default::default()
                });

            let dashboard_page: Element<'_, Message> = if show_observations {
                let logs_title = row![
                    text("Observations").size(super::super::text_size(20)),
                    Space::new().width(Length::Fill),
                    button(text("Clear Logs"))
                        .padding(Padding {
                            top: super::super::scaled(6.0),
                            right: super::super::scaled(12.0),
                            bottom: super::super::scaled(6.0),
                            left: super::super::scaled(12.0),
                        })
                        .style(move |theme: &Theme, status| {
                            super::super::themed_button_style(
                                theme,
                                status,
                                KdeButtonRole::Danger,
                                use_kde_buttons,
                            )
                        })
                        .on_press(Message::LogsClear),
                ]
                .align_y(Alignment::Center);

                let logs_entries = container(self.view_log_entries())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_theme: &Theme| container::Style {
                        background: Some(logs_entries_bg.into()),
                        border: Border {
                            radius: 6.0.into(),
                            width: 1.0,
                            color: panel_border,
                        },
                        ..Default::default()
                    });

                let logs_pane = container(
                    column![logs_title, self.view_log_filter_bar(false), logs_entries]
                        .spacing(12)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .padding(Padding {
                    top: 14.0,
                    right: 14.0,
                    bottom: 14.0,
                    left: 14.0,
                })
                .width(Length::Fixed(logs_width))
                .height(Length::Fill)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(logs_panel_bg.into()),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: panel_border,
                    },
                    ..Default::default()
                });

                let splitter_active = self.dashboard_is_resizing_split;
                let splitter_bg = if splitter_active {
                    let strong = self.theme().extended_palette().primary.base.color;
                    Color::from_rgba(strong.r, strong.g, strong.b, 0.40)
                } else {
                    Color::from_rgba(panel_border.r, panel_border.g, panel_border.b, 0.25)
                };

                let splitter = mouse_area(
                    container(Space::new().width(Length::Fill).height(Length::Fill))
                        .width(Length::Fixed(DASHBOARD_SPLITTER_HIT_WIDTH))
                        .height(Length::Fill)
                        .style(move |_theme: &Theme| container::Style {
                            background: Some(splitter_bg.into()),
                            border: Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                )
                .interaction(mouse::Interaction::ResizingHorizontally)
                .on_press(Message::DashboardStartSplitResize)
                .on_release(Message::DashboardEndSplitResize);

                row![list_pane, splitter, logs_pane]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                list_pane.into()
            };

            let area = mouse_area(dashboard_page)
                .on_move(|p| Message::DashboardCursorMoved(p.x, p.y))
                .on_release(Message::DashboardEndSplitResize)
                .on_exit(Message::DashboardEndSplitResize);

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
