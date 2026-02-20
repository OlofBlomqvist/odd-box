use super::super::readable_on;
use iced::widget::{Row, Space, button, column, container, row, text, text_input};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use crate::global_state::ProcState;
use crate::gui::components::{
    Column as TableColumn, Table,
    table::{colored_text_cell, text_cell},
};

use super::super::{KdeButtonRole, Message, OddBoxGui, ProcessesTab, scaled, text_size};
use iced::widget::text::Wrapping;

/// Style for tab buttons
fn tab_button_style(
    theme: &Theme,
    status: button::Status,
    is_active: bool,
    use_kde_buttons: bool,
) -> button::Style {
    if use_kde_buttons {
        if is_active {
            let active_status = match status {
                button::Status::Hovered | button::Status::Pressed => status,
                _ => button::Status::Pressed,
            };
            return super::super::kde_primary_button_style(theme, active_status);
        }
        return super::super::kde_neutral_button_style(theme, status);
    }
    let palette = theme.extended_palette();
    let is_dark = palette.is_dark;
    let r = scaled(6.0);
    if is_active {
        let bg = if is_dark {
            Color::from_rgb(0.25, 0.48, 0.85)
        } else {
            palette.primary.strong.color
        };
        let fg = readable_on(bg, Color::WHITE, Color::BLACK);
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => {
                if is_dark {
                    Color::from_rgb(0.35, 0.56, 0.92)
                } else {
                    palette.primary.base.color
                }
            }
            _ => bg,
        };
        button::Style {
            background: Some(bg.into()),
            text_color: fg,
            border: Border {
                radius: r.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    } else {
        let bg = match status {
            button::Status::Hovered => {
                if is_dark {
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
                radius: r.into(),
                width: 1.0,
                color: if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.2)
                } else {
                    palette.background.strong.color
                },
            },
            ..Default::default()
        }
    }
}

impl OddBoxGui {
    pub(in crate::gui) fn view_processes(&self) -> Element<'_, Message> {
        let current_tab = self.processes_tab;
        let use_kde_buttons = self.use_kde_system_styles();

        // Tab bar
        let processes_tab_btn = button(
            text("Processes")
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .size(text_size(14)),
        )
        .padding(Padding {
            top: scaled(8.0),
            right: scaled(16.0),
            bottom: scaled(8.0),
            left: scaled(16.0),
        })
        .style(move |theme: &Theme, status| {
            tab_button_style(
                theme,
                status,
                current_tab == ProcessesTab::Processes,
                use_kde_buttons,
            )
        })
        .on_press(Message::ProcessesTabChanged(ProcessesTab::Processes));

        let global_vars_tab_btn = button(
            text("Global Variables")
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .size(text_size(14)),
        )
        .padding(Padding {
            top: scaled(8.0),
            right: scaled(16.0),
            bottom: scaled(8.0),
            left: scaled(16.0),
        })
        .style(move |theme: &Theme, status| {
            tab_button_style(
                theme,
                status,
                current_tab == ProcessesTab::GlobalVariables,
                use_kde_buttons,
            )
        })
        .on_press(Message::ProcessesTabChanged(ProcessesTab::GlobalVariables));

        let tab_bar = container(row![processes_tab_btn, global_vars_tab_btn].spacing(scaled(8.0)))
            .style(|theme: &Theme| container::Style {
                border: Border {
                    width: 0.0,
                    color: self.surface_border_color(theme),
                    radius: 0.0.into(),
                },
                ..Default::default()
            });

        // Separator line under tabs
        let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(0.0)))
            .height(1)
            .width(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(self.surface_border_color(theme).into()),
                ..Default::default()
            });

        let tab_content: Element<'_, Message> = match self.processes_tab {
            ProcessesTab::Processes => self.view_processes_tab(),
            ProcessesTab::GlobalVariables => self.view_global_variables_tab(),
        };

        column![tab_bar, separator, tab_content]
            .spacing(scaled(12.0))
            .width(Length::Fill)
            .into()
    }

    fn view_processes_tab(&self) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();

        let add_process_btn = button(text("Add Process").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
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
                KdeButtonRole::Neutral,
                use_kde_buttons,
            )
        })
        .on_press(Message::OpenNewBackend(super::super::BackendKind::Process));

        if self.cached_config.processes.is_empty() {
            return column![add_process_btn, text("No managed processes configured"),]
                .spacing(scaled(12.0))
                .into();
        }

        // Start All / Stop All buttons
        let start_all_btn = button(text("Start All").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
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
                KdeButtonRole::Success,
                use_kde_buttons,
            )
        })
        .on_press(Message::ProcessStartAll);

        let stop_all_btn = button(text("Stop All").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .padding(Padding {
            top: scaled(6.0),
            right: scaled(12.0),
            bottom: scaled(6.0),
            left: scaled(12.0),
        })
        .style(move |theme: &Theme, status| {
            super::super::themed_button_style(theme, status, KdeButtonRole::Danger, use_kde_buttons)
        })
        .on_press(Message::ProcessStopAll);

        let bulk_actions = row![add_process_btn, start_all_btn, stop_all_btn].spacing(scaled(8.0));

        let table_theme = self.theme();
        let table_header_bg = self.surface_panel_alt_bg(&table_theme);
        let table_row_even_bg = self.surface_panel_bg(&table_theme);
        let table_row_odd_bg = self.surface_panel_alt_bg(&table_theme);
        let table_border = self.surface_border_color(&table_theme);

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

            // Clickable status cell for start/stop toggling (same behavior as Backends page)
            let status_label = format!("{:?}", proc.state);
            let proc_name_for_status = proc.name.clone();
            let is_transitioning = matches!(proc.state, ProcState::Starting | ProcState::Stopping);
            let can_start = matches!(proc.state, ProcState::Stopped | ProcState::Faulty);
            let can_stop = matches!(proc.state, ProcState::Running | ProcState::Faulty);
            let status_cell: Element<'_, Message> = if !is_transitioning && (can_start || can_stop)
            {
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
            let mut auto_label = if proc.auto_start {
                "Yes".to_string()
            } else {
                "No".to_string()
            };
            if self.proc_auto_start_in_flight.contains(&proc.name) {
                auto_label.push_str("...");
            }
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
                    text_cell(&proc.name),
                    text_cell(&proc.bin),
                    text_cell(&proc.port),
                    text_cell(&proc.protocol),
                    status_cell,
                    auto_cell,
                ],
                Message::ProcessToggleDetails(proc.name.clone()),
            );

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

        column![bulk_actions, table.build()]
            .spacing(scaled(12.0))
            .width(Length::Fill)
            .into()
    }

    fn view_global_variables_tab(&self) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();
        let description = text(
            "Global environment variables are available to all process backends. \
             Process-level variables override global ones with the same key.",
        )
        .size(text_size(13))
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.weak.text),
            ..Default::default()
        });

        // Build the table of existing variables (only when non-empty)
        let var_table: Option<Element<'_, Message>> = if self.global_env_vars.is_empty() {
            None
        } else {
            let table_theme = self.theme();
            let table_header_bg = self.surface_panel_alt_bg(&table_theme);
            let table_row_even_bg = self.surface_panel_bg(&table_theme);
            let table_row_odd_bg = self.surface_panel_alt_bg(&table_theme);
            let table_border = self.surface_border_color(&table_theme);

            let columns = vec![
                TableColumn::portion("Key", 3),
                TableColumn::portion("Value", 5),
                TableColumn::fixed("", 36.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for (idx, (key, value)) in self.global_env_vars.iter().enumerate() {
                let key_input: Element<'_, Message> = text_input("KEY", key)
                    .on_input(move |v| Message::GlobalEnvKeyChanged(idx, v))
                    .padding(scaled(6.0))
                    .size(text_size(13))
                    .font(Font::MONOSPACE)
                    .width(Length::Fill)
                    .into();

                let value_input: Element<'_, Message> = text_input("VALUE", value)
                    .on_input(move |v| Message::GlobalEnvValueChanged(idx, v))
                    .padding(scaled(6.0))
                    .size(text_size(13))
                    .font(Font::MONOSPACE)
                    .width(Length::Fill)
                    .into();

                let remove_btn: Element<'_, Message> = button(text("✕").size(text_size(14)))
                    .padding(Padding {
                        top: scaled(4.0),
                        right: scaled(8.0),
                        bottom: scaled(4.0),
                        left: scaled(8.0),
                    })
                    .style(move |theme: &Theme, status| {
                        super::super::themed_button_style(
                            theme,
                            status,
                            KdeButtonRole::Danger,
                            use_kde_buttons,
                        )
                    })
                    .on_press(Message::GlobalEnvRemove(idx))
                    .into();

                table = table.push_row(vec![key_input, value_input, remove_btn]);
            }

            Some(table.build())
        };

        // Add new variable row
        let new_key_input = text_input("New key...", &self.global_env_new_key)
            .on_input(Message::GlobalEnvNewKeyChanged)
            .on_submit(Message::GlobalEnvAdd)
            .padding(scaled(8.0))
            .size(text_size(13))
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(3));

        let new_value_input = text_input("New value...", &self.global_env_new_value)
            .on_input(Message::GlobalEnvNewValueChanged)
            .on_submit(Message::GlobalEnvAdd)
            .padding(scaled(8.0))
            .size(text_size(13))
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(5));

        let add_btn = button(text("+ Add").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
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
                KdeButtonRole::Primary,
                use_kde_buttons,
            )
        })
        .on_press(Message::GlobalEnvAdd);

        let add_row = row![new_key_input, new_value_input, add_btn]
            .spacing(scaled(8.0))
            .align_y(iced::Alignment::Center);

        // Save button + notice
        let save_btn = button(text("Save").size(text_size(14)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .padding(Padding {
            top: scaled(8.0),
            right: scaled(20.0),
            bottom: scaled(8.0),
            left: scaled(20.0),
        })
        .style(move |theme: &Theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Success,
                use_kde_buttons,
            )
        })
        .on_press(Message::GlobalEnvSave);

        let mut bottom_row_children: Vec<Element<'_, Message>> = vec![save_btn.into()];

        if self.global_env_dirty {
            bottom_row_children.push(
                text("Unsaved changes")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.7, 0.2))
                    .into(),
            );
        }

        if let Some(notice) = &self.global_env_notice {
            let is_error = notice.starts_with("Error");
            let color = if is_error {
                Color::from_rgb(0.9, 0.3, 0.3)
            } else {
                Color::from_rgb(0.4, 0.8, 0.4)
            };
            bottom_row_children.push(text(notice.clone()).size(text_size(13)).color(color).into());
        }

        let bottom_row = Row::with_children(bottom_row_children)
            .spacing(scaled(12.0))
            .align_y(iced::Alignment::Center);

        let mut content = column![description]
            .spacing(scaled(14.0))
            .width(Length::Fill);

        if let Some(table_el) = var_table {
            content = content.push(table_el);
        }

        content = content.push(add_row).push(bottom_row);

        content.into()
    }

    pub(in crate::gui) fn build_process_actions(
        &self,
        proc_name: &str,
        state: ProcState,
    ) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();
        let proc_name_start = proc_name.to_string();
        let proc_name_stop = proc_name.to_string();
        let proc_name_edit = proc_name.to_string();
        let can_start = matches!(state, ProcState::Stopped | ProcState::Faulty);
        let can_stop = matches!(state, ProcState::Running | ProcState::Faulty);
        let is_transitioning = matches!(state, ProcState::Starting | ProcState::Stopping);

        let btn_padding = Padding {
            top: scaled(6.0),
            right: scaled(12.0),
            bottom: scaled(6.0),
            left: scaled(12.0),
        };

        // Start button
        let start_btn = button(text("Start").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .padding(btn_padding)
        .style(move |theme: &Theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Success,
                use_kde_buttons,
            )
        });

        let start_btn = if can_start && !is_transitioning {
            start_btn.on_press(Message::ProcessStart(proc_name_start))
        } else {
            start_btn
        };

        // Stop button
        let stop_btn = button(text("Stop").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .padding(btn_padding)
        .style(move |theme: &Theme, status| {
            super::super::themed_button_style(theme, status, KdeButtonRole::Danger, use_kde_buttons)
        });

        let stop_btn = if can_stop && !is_transitioning {
            stop_btn.on_press(Message::ProcessStop(proc_name_stop))
        } else {
            stop_btn
        };

        // Edit button
        let edit_btn = button(text("Edit").size(text_size(13)).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }))
        .padding(btn_padding)
        .style(move |theme: &Theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Primary,
                use_kde_buttons,
            )
        })
        .on_press(Message::OpenEditBackend(proc_name_edit));

        row![start_btn, stop_btn, edit_btn]
            .spacing(scaled(8.0))
            .into()
    }
}
