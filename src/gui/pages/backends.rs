use iced::widget::{Column, button, column, container, row, text};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, colored_text_cell, text_cell, wrap_text_cell},
};

use super::super::{KdeButtonRole, Message, OddBoxGui, scaled, text_size};
use crate::global_state::ProcState;
use iced::widget::text::Wrapping;

impl OddBoxGui {
    pub(in crate::gui) fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();
        let use_kde_buttons = self.use_kde_system_styles();
        let table_theme = self.theme();
        let table_header_bg = self.surface_panel_alt_bg(&table_theme);
        let table_row_even_bg = self.surface_panel_bg(&table_theme);
        let table_row_odd_bg = self.surface_panel_alt_bg(&table_theme);
        let table_border = self.surface_border_color(&table_theme);

        let add_remote_btn = button(text("Add Remote").size(text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Remote))
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Neutral,
                    use_kde_buttons,
                )
            });
        let add_static_btn = button(text("Add Static").size(text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Static))
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Neutral,
                    use_kde_buttons,
                )
            });
        let add_process_btn = button(text("Add Process").size(text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Process))
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Neutral,
                    use_kde_buttons,
                )
            });
        let start_all_btn = button(text("Start All").size(text_size(14)))
            .on_press(Message::ProcessStartAll)
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Success,
                    use_kde_buttons,
                )
            });
        let stop_all_btn = button(text("Stop All").size(text_size(14)))
            .on_press(Message::ProcessStopAll)
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Danger,
                    use_kde_buttons,
                )
            });

        let actions = row![
            add_remote_btn,
            add_static_btn,
            add_process_btn,
            start_all_btn,
            stop_all_btn,
        ]
        .spacing(scaled(8.0));

        // Process backends section
        if !self.cached_config.processes.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Binary", 2),
                TableColumn::fixed("Port", 80.0),
                TableColumn::fixed("Protocol", 80.0),
                TableColumn::fixed("Status", 100.0),
                TableColumn::fixed("Auto", 80.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for proc in &self.cached_config.processes {
                let is_expanded = self.expanded_process.as_deref() == Some(&proc.name);
                let status_color = match proc.state {
                    ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                    ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                    ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                    ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                    _ => Color::from_rgb(0.6, 0.6, 0.6),
                };

                // Clickable status cell for start/stop toggling
                let status_label = format!("{:?}", proc.state);
                let proc_name_for_status = proc.name.clone();
                let is_transitioning =
                    matches!(proc.state, ProcState::Starting | ProcState::Stopping);
                let can_start = matches!(proc.state, ProcState::Stopped | ProcState::Faulty);
                let can_stop = matches!(proc.state, ProcState::Running | ProcState::Faulty);

                let status_cell: Element<'_, Message> =
                    if !is_transitioning && (can_start || can_stop) {
                        let msg = if can_start {
                            Message::ProcessStart(proc_name_for_status)
                        } else {
                            Message::ProcessStop(proc_name_for_status)
                        };
                        let sc = status_color;
                        button(text(status_label).size(text_size(12)).color(sc))
                            .padding(Padding {
                                top: 2.0,
                                right: 6.0,
                                bottom: 2.0,
                                left: 6.0,
                            })
                            .style(move |theme: &Theme, status| {
                                let palette = theme.extended_palette();
                                let bg = match status {
                                    button::Status::Hovered => {
                                        if palette.is_dark {
                                            Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                                        } else {
                                            palette.background.weak.color
                                        }
                                    }
                                    _ => Color::TRANSPARENT,
                                };
                                button::Style {
                                    background: Some(bg.into()),
                                    text_color: palette.background.base.text,
                                    border: Border {
                                        radius: 4.0.into(),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            })
                            .on_press(msg)
                            .into()
                    } else {
                        colored_text_cell(status_label, status_color)
                    };

                // Clickable auto-start toggle cell
                let auto_label = if proc.auto_start { "Yes" } else { "No" };
                let auto_color = if proc.auto_start {
                    Color::from_rgb(0.4, 0.85, 0.4)
                } else {
                    Color::from_rgb(0.5, 0.5, 0.5)
                };
                let proc_name_for_auto = proc.name.clone();
                let auto_cell: Element<'_, Message> =
                    button(text(auto_label).size(text_size(12)).color(auto_color))
                        .padding(Padding {
                            top: 2.0,
                            right: 6.0,
                            bottom: 2.0,
                            left: 6.0,
                        })
                        .style(move |theme: &Theme, status| {
                            let palette = theme.extended_palette();
                            let bg = match status {
                                button::Status::Hovered => {
                                    if palette.is_dark {
                                        Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                                    } else {
                                        palette.background.weak.color
                                    }
                                }
                                _ => Color::TRANSPARENT,
                            };
                            button::Style {
                                background: Some(bg.into()),
                                text_color: palette.background.base.text,
                                border: Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        })
                        .on_press(Message::ProcessToggleAutoStart(proc_name_for_auto))
                        .into();

                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&proc.name),
                        text_cell(&proc.bin),
                        text_cell(&proc.port),
                        text_cell(&proc.protocol),
                        status_cell,
                        auto_cell,
                    ],
                    Message::ProcessToggleDetails(proc.name.clone()),
                );

                // Expandable detail row with environment info and action buttons
                if is_expanded {
                    let configured_port = proc
                        .configured_port
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let env_text = if proc.env.is_empty() {
                        "none".to_string()
                    } else {
                        proc.env
                            .iter()
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    let details = format!(
                        "Assigned port: {}\nConfigured port: {}\nBinary: {}\nWorking dir: {}\nArgs: {}\nEnv:\n{}",
                        proc.port,
                        configured_port,
                        if proc.bin.is_empty() { "-" } else { &proc.bin },
                        if proc.dir.is_empty() { "-" } else { &proc.dir },
                        if proc.args.is_empty() {
                            "-"
                        } else {
                            &proc.args
                        },
                        env_text
                    );
                    let detail_text = text(details)
                        .font(Font::MONOSPACE)
                        .size(text_size(12))
                        .wrapping(Wrapping::Word);
                    let actions = self.build_process_actions(&proc.name, proc.state.clone());
                    let detail_content = column![detail_text, actions].spacing(scaled(8.0));
                    let detail_row = container(detail_content)
                        .padding(Padding {
                            top: scaled(6.0),
                            right: scaled(12.0),
                            bottom: scaled(6.0),
                            left: scaled(24.0),
                        })
                        .width(Length::Fill);
                    table = table.push_full_row(detail_row.into());
                }
            }

            sections.push(
                column![text("Process Backends"), table.build()]
                    .spacing(scaled(10.0))
                    .into(),
            );
        }

        // Remote backends section
        if !self.cached_config.remote_backends.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Endpoints", 3),
                TableColumn::fixed("Protocol", 80.0),
                TableColumn::fixed("HTTPS", 60.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for backend in &self.cached_config.remote_backends {
                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&backend.name),
                        text_cell(&backend.endpoints),
                        text_cell(&backend.protocol),
                        bool_cell(backend.https),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![text("Remote Backends"), table.build()]
                    .spacing(scaled(10.0))
                    .into(),
            );
        }

        // Static backends section
        if !self.cached_config.static_backends.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Directory", 4),
                TableColumn::fixed("List Dir", 80.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for backend in &self.cached_config.static_backends {
                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&backend.name),
                        text_cell(&backend.dir),
                        bool_cell(backend.list_dir),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![text("Static Backends"), table.build()]
                    .spacing(scaled(10.0))
                    .into(),
            );
        }

        if sections.is_empty() {
            return column![
                actions,
                text("No backends configured")
                    .color(self.theme().extended_palette().background.strong.text)
            ]
            .spacing(scaled(12.0))
            .into();
        }

        let mut content = vec![actions.into()];
        content.extend(sections);
        Column::with_children(content).spacing(scaled(20.0)).into()
    }
}
