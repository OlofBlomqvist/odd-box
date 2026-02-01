use iced::widget::{button, column, container, row, text, Column};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use crate::types::app_state::ProcState;

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_processes(&self) -> Element<'_, Message> {
        if self.cached_config.processes.is_empty() {
            return text("No managed processes configured").into();
        }

        // Table header
        let header = row![
            text("Name")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("Binary")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("Port")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(80.0)),
            text("Protocol")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(80.0)),
            text("Status")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(100.0)),
            text("Auto")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(60.0)),
            text("Actions")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(140.0)),
        ]
        .spacing(10)
        .padding(Padding {
            top: 8.0,
            right: 10.0,
            bottom: 8.0,
            left: 10.0,
        });

        let header_container = container(header)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.strong.color.into()),
                    ..Default::default()
                }
            });

        // Table rows with alternating colors
        let rows: Vec<Element<'_, Message>> = self
            .cached_config
            .processes
            .iter()
            .enumerate()
            .map(|(idx, proc)| {
                let status_color = match proc.state {
                    ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                    ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                    ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                    ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                    _ => Color::from_rgb(0.6, 0.6, 0.6),
                };

                let is_running = matches!(proc.state, ProcState::Running | ProcState::Starting);
                let is_transitioning =
                    matches!(proc.state, ProcState::Starting | ProcState::Stopping);

                let proc_name = proc.name.clone();
                let proc_name_stop = proc.name.clone();

                // Start button - enabled when stopped or faulty
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
                    start_btn.on_press(Message::ProcessStart(proc_name))
                } else {
                    start_btn
                };

                // Stop button - enabled when running
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

                let actions = row![start_btn, stop_btn]
                    .spacing(6)
                    .width(Length::Fixed(140.0));

                let row_content = row![
                    text(&proc.name)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(&proc.bin)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(&proc.port)
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(80.0)),
                    text(&proc.protocol)
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(80.0)),
                    text(format!("{:?}", proc.state))
                        .font(Font::MONOSPACE)
                        .color(status_color)
                        .width(Length::Fixed(100.0)),
                    text(if proc.auto_start { "Yes" } else { "No" })
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(60.0)),
                    actions,
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center);

                let is_even = idx % 2 == 0;

                container(row_content)
                    .width(Length::Fill)
                    .padding(Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 6.0,
                        left: 10.0,
                    })
                    .style(move |theme: &Theme| {
                        let palette = theme.extended_palette();
                        let bg = if is_even {
                            palette.background.weak.color
                        } else {
                            palette.background.base.color
                        };
                        container::Style {
                            background: Some(bg.into()),
                            ..Default::default()
                        }
                    })
                    .into()
            })
            .collect();

        let table = column![header_container]
            .push(Column::with_children(rows))
            .width(Length::Fill);

        container(table)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into()
    }
}
