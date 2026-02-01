use iced::widget::{button, row, text};
use iced::{Border, Color, Element, Font, Padding, Theme};

use crate::gui::components::{
    table::{bool_cell, colored_text_cell, text_cell},
    Column as TableColumn, Table,
};
use crate::types::app_state::ProcState;

use super::super::{Message, OddBoxGui};

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
            TableColumn::fixed("Actions", 140.0),
        ];

        let mut table = Table::new(columns);

        for proc in &self.cached_config.processes {
            let status_color = match proc.state {
                ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                _ => Color::from_rgb(0.6, 0.6, 0.6),
            };

            let is_running = matches!(proc.state, ProcState::Running | ProcState::Starting);
            let is_transitioning = matches!(proc.state, ProcState::Starting | ProcState::Stopping);

            let actions = self.build_process_actions(&proc.name, is_running, is_transitioning);

            table = table.push_row(vec![
                text_cell(&proc.name),
                text_cell(&proc.bin),
                text_cell(&proc.port),
                text_cell(&proc.protocol),
                colored_text_cell(format!("{:?}", proc.state), status_color),
                bool_cell(proc.auto_start),
                actions,
            ]);
        }

        table.build()
    }

    fn build_process_actions(
        &self,
        proc_name: &str,
        is_running: bool,
        is_transitioning: bool,
    ) -> Element<'_, Message> {
        let proc_name_start = proc_name.to_string();
        let proc_name_stop = proc_name.to_string();

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

        let start_btn = if !is_running && !is_transitioning {
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

        let stop_btn = if is_running && !is_transitioning {
            stop_btn.on_press(Message::ProcessStop(proc_name_stop))
        } else {
            stop_btn
        };

        row![start_btn, stop_btn].spacing(6).into()
    }
}
