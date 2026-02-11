use iced::widget::{button, column, container, row, text};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use crate::global_state::ProcState;
use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, colored_text_cell, text_cell},
};

use super::super::{Message, OddBoxGui};
use iced::widget::text::Wrapping;

impl OddBoxGui {
    pub(in crate::gui) fn view_processes(&self) -> Element<'_, Message> {
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
                    .size(12)
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

    fn build_process_actions(&self, proc_name: &str, state: ProcState) -> Element<'_, Message> {
        let proc_name_start = proc_name.to_string();
        let proc_name_stop = proc_name.to_string();
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

        row![start_btn, stop_btn].spacing(6).into()
    }
}
