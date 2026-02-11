use iced::widget::{button, column, container, row, text, text_input, Row, Space};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use crate::global_state::ProcState;
use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, colored_text_cell, text_cell},
};

use super::super::{Message, OddBoxGui, ProcessesTab};
use iced::widget::text::Wrapping;

/// Style for tab buttons
fn tab_button_style(
    theme: &Theme,
    status: button::Status,
    is_active: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    if is_active {
        button::Style {
            background: Some(palette.primary.strong.color.into()),
            text_color: palette.primary.strong.text,
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    } else {
        let bg = match status {
            button::Status::Hovered => palette.background.weak.color,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(bg.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: 6.0.into(),
                width: 1.0,
                color: palette.background.strong.color,
            },
            ..Default::default()
        }
    }
}

impl OddBoxGui {
    pub(in crate::gui) fn view_processes(&self) -> Element<'_, Message> {
        let current_tab = self.processes_tab;

        // Tab bar
        let processes_tab_btn = button(
            text("Processes")
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .size(14),
        )
        .padding(Padding {
            top: 8.0,
            right: 16.0,
            bottom: 8.0,
            left: 16.0,
        })
        .style(move |theme: &Theme, status| {
            tab_button_style(theme, status, current_tab == ProcessesTab::Processes)
        })
        .on_press(Message::ProcessesTabChanged(ProcessesTab::Processes));

        let global_vars_tab_btn = button(
            text("Global Variables")
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .size(14),
        )
        .padding(Padding {
            top: 8.0,
            right: 16.0,
            bottom: 8.0,
            left: 16.0,
        })
        .style(move |theme: &Theme, status| {
            tab_button_style(theme, status, current_tab == ProcessesTab::GlobalVariables)
        })
        .on_press(Message::ProcessesTabChanged(ProcessesTab::GlobalVariables));

        let tab_bar = container(
            row![processes_tab_btn, global_vars_tab_btn].spacing(2),
        )
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                border: Border {
                    width: 0.0,
                    color: palette.background.strong.color,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        });

        // Separator line under tabs
        let separator = container(Space::new().width(Length::Fill).height(Length::Fixed(0.0)))
            .height(1)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.strong.color.into()),
                    ..Default::default()
                }
            });

        let tab_content: Element<'_, Message> = match self.processes_tab {
            ProcessesTab::Processes => self.view_processes_tab(),
            ProcessesTab::GlobalVariables => self.view_global_variables_tab(),
        };

        column![tab_bar, separator, tab_content]
            .spacing(12)
            .width(Length::Fill)
            .into()
    }

    fn view_processes_tab(&self) -> Element<'_, Message> {
        if self.cached_config.processes.is_empty() {
            return text("No managed processes configured").into();
        }

        let columns = vec![
            TableColumn::portion("Name", 2),
            TableColumn::portion("Binary", 2),
            TableColumn::fixed("Port", 80.0),
            TableColumn::fixed("Protocol", 80.0),
            TableColumn::fixed("Status", 100.0),
            TableColumn::fixed("Auto", 60.0),
        ];

        let mut table = Table::new(columns);

        for proc in &self.cached_config.processes {
            let is_expanded = self.expanded_process.as_deref() == Some(&proc.name);
            let status_color = match proc.state {
                ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                _ => Color::from_rgb(0.6, 0.6, 0.6),
            };

            table = table.push_row_with_message(
                vec![
                    text_cell(&proc.name),
                    text_cell(&proc.bin),
                    text_cell(&proc.port),
                    text_cell(&proc.protocol),
                    colored_text_cell(format!("{:?}", proc.state), status_color),
                    bool_cell(proc.auto_start),
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
                    .size(super::super::text_size(12))
                    .wrapping(Wrapping::Word);
                let actions = self.build_process_actions(&proc.name, proc.state.clone());
                let detail_content = column![detail_text, actions].spacing(8);
                let detail_row = container(detail_content)
                    .padding(Padding {
                        top: 6.0,
                        right: 12.0,
                        bottom: 6.0,
                        left: 24.0,
                    })
                    .width(Length::Fill);
                table = table.push_full_row(detail_row.into());
            }
        }

        table.build()
    }

    fn view_global_variables_tab(&self) -> Element<'_, Message> {
        let description = text(
            "Global environment variables are available to all process backends. \
             Process-level variables override global ones with the same key.",
        )
        .size(13)
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.weak.text),
            ..Default::default()
        });

        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        // Header row
        let header = container(
            row![
                container(
                    text("Key")
                        .size(13)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        })
                )
                .width(Length::FillPortion(3)),
                container(
                    text("Value")
                        .size(13)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        })
                )
                .width(Length::FillPortion(5)),
                container(Space::new().width(Length::Fixed(36.0)).height(Length::Fixed(0.0))).width(Length::Fixed(36.0)),
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
            container::Style {
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });
        rows.push(header.into());

        // Existing variable rows
        for (idx, (key, value)) in self.global_env_vars.iter().enumerate() {
            let key_input = text_input("KEY", key)
                .on_input(move |v| Message::GlobalEnvKeyChanged(idx, v))
                .padding(6)
                .size(13)
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(3));

            let value_input = text_input("VALUE", value)
                .on_input(move |v| Message::GlobalEnvValueChanged(idx, v))
                .padding(6)
                .size(13)
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(5));

            let remove_btn = button(
                text("✕").size(14),
            )
            .padding(Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let (bg, fg) = match status {
                    button::Status::Hovered => {
                        (palette.danger.strong.color, palette.danger.strong.text)
                    }
                    _ => (Color::TRANSPARENT, palette.danger.base.color),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: fg,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::GlobalEnvRemove(idx));

            let env_row = container(
                row![key_input, value_input, remove_btn]
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
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                let bg = if idx % 2 == 0 {
                    Color::TRANSPARENT
                } else {
                    let c = palette.background.weak.color;
                    Color::from_rgba(c.r, c.g, c.b, 0.3)
                };
                container::Style {
                    background: Some(bg.into()),
                    border: Border {
                        width: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

            rows.push(env_row.into());
        }

        // Empty state message
        if self.global_env_vars.is_empty() {
            let empty_msg = container(
                text("No global environment variables configured")
                    .size(13)
                    .style(|theme: &Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().background.weak.text),
                        ..Default::default()
                    }),
            )
            .padding(20)
            .width(Length::Fill)
            .align_x(iced::Alignment::Center);
            rows.push(empty_msg.into());
        }

        // Table container
        let var_table = container(
            iced::widget::Column::with_children(rows).width(Length::Fill),
        )
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });

        // Add new variable row
        let new_key_input = text_input("New key...", &self.global_env_new_key)
            .on_input(Message::GlobalEnvNewKeyChanged)
            .on_submit(Message::GlobalEnvAdd)
            .padding(8)
            .size(13)
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(3));

        let new_value_input = text_input("New value...", &self.global_env_new_value)
            .on_input(Message::GlobalEnvNewValueChanged)
            .on_submit(Message::GlobalEnvAdd)
            .padding(8)
            .size(13)
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(5));

        let add_btn = button(
            text("+ Add").size(13).font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        )
        .padding(Padding {
            top: 6.0,
            right: 12.0,
            bottom: 6.0,
            left: 12.0,
        })
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let (bg, fg) = match status {
                button::Status::Hovered => {
                    (palette.primary.strong.color, palette.primary.strong.text)
                }
                button::Status::Disabled => (
                    palette.background.weak.color,
                    palette.background.strong.text,
                ),
                _ => (palette.primary.weak.color, palette.primary.weak.text),
            };
            button::Style {
                background: Some(bg.into()),
                text_color: fg,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(Message::GlobalEnvAdd);

        let add_row = row![new_key_input, new_value_input, add_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        // Save button + notice
        let save_btn = button(
            text("Save").size(14).font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
        )
        .padding(Padding {
            top: 8.0,
            right: 20.0,
            bottom: 8.0,
            left: 20.0,
        })
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let (bg, fg) = match status {
                button::Status::Hovered => {
                    (palette.success.strong.color, palette.success.strong.text)
                }
                button::Status::Disabled => (
                    palette.background.weak.color,
                    palette.background.strong.text,
                ),
                _ => (palette.success.weak.color, palette.success.weak.text),
            };
            button::Style {
                background: Some(bg.into()),
                text_color: fg,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(Message::GlobalEnvSave);

        let mut bottom_row_children: Vec<Element<'_, Message>> = vec![save_btn.into()];

        if self.global_env_dirty {
            bottom_row_children.push(
                text("Unsaved changes")
                    .size(12)
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
            bottom_row_children.push(text(notice.clone()).size(13).color(color).into());
        }

        let bottom_row = Row::with_children(bottom_row_children)
            .spacing(12)
            .align_y(iced::Alignment::Center);

        column![description, var_table, add_row, bottom_row]
            .spacing(14)
            .width(Length::Fill)
            .into()
    }

    fn build_process_actions(&self, proc_name: &str, state: ProcState) -> Element<'_, Message> {
        let proc_name_start = proc_name.to_string();
        let proc_name_stop = proc_name.to_string();
        let proc_name_edit = proc_name.to_string();
        let can_start = matches!(state, ProcState::Stopped | ProcState::Faulty);
        let can_stop = matches!(state, ProcState::Running | ProcState::Faulty);
        let is_transitioning = matches!(state, ProcState::Starting | ProcState::Stopping);

        // Start button
        let start_btn = button(text("Start").font(Font::MONOSPACE))
            .padding(Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let (bg, fg) = match status {
                    button::Status::Hovered => {
                        (palette.success.strong.color, palette.success.strong.text)
                    }
                    button::Status::Disabled => (
                        palette.background.weak.color,
                        palette.background.strong.text,
                    ),
                    _ => (palette.success.weak.color, palette.success.weak.text),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: fg,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

        let start_btn = if can_start && !is_transitioning {
            start_btn.on_press(Message::ProcessStart(proc_name_start))
        } else {
            start_btn
        };

        // Stop button
        let stop_btn = button(text("Stop").font(Font::MONOSPACE))
            .padding(Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let (bg, fg) = match status {
                    button::Status::Hovered => {
                        (palette.danger.strong.color, palette.danger.strong.text)
                    }
                    button::Status::Disabled => (
                        palette.background.weak.color,
                        palette.background.strong.text,
                    ),
                    _ => (palette.danger.weak.color, palette.danger.weak.text),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: fg,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

        let stop_btn = if can_stop && !is_transitioning {
            stop_btn.on_press(Message::ProcessStop(proc_name_stop))
        } else {
            stop_btn
        };

        // Edit button
        let edit_btn = button(text("Edit").font(Font::MONOSPACE))
            .padding(Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                let (bg, fg) = match status {
                    button::Status::Hovered => {
                        (palette.primary.strong.color, palette.primary.strong.text)
                    }
                    _ => (palette.primary.weak.color, palette.primary.weak.text),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: fg,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::OpenEditBackend(proc_name_edit));

        row![start_btn, stop_btn, edit_btn].spacing(6).into()
    }
}