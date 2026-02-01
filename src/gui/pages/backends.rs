use iced::widget::{column, container, row, text, Column};
use iced::{Border, Element, Font, Length, Padding, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();

        // Remote backends section
        if !self.cached_config.remote_backends.is_empty() {
            let header = row![
                text("Name")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(2)),
                text("Endpoints")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(3)),
                text("Protocol")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(80.0)),
                text("HTTPS")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(60.0)),
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
                        background: Some(palette.background.weak.color.into()),
                        ..Default::default()
                    }
                });

            let rows: Vec<Element<'_, Message>> = self
                .cached_config
                .remote_backends
                .iter()
                .map(|backend| {
                    row![
                        text(&backend.name)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2)),
                        text(&backend.endpoints)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(3)),
                        text(&backend.protocol)
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(80.0)),
                        text(if backend.https { "Yes" } else { "No" })
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(60.0)),
                    ]
                    .spacing(10)
                    .padding(Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 6.0,
                        left: 10.0,
                    })
                    .into()
                })
                .collect();

            let table = column![header_container]
                .push(Column::with_children(rows).spacing(2))
                .width(Length::Fill);

            sections.push(
                column![
                    text("Remote Backends"),
                    container(table).width(Length::Fill).style(|theme: &Theme| {
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
                ]
                .spacing(10)
                .into(),
            );
        }

        // Static backends section
        if !self.cached_config.static_backends.is_empty() {
            let header = row![
                text("Name")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(2)),
                text("Directory")
                    .font(Font::MONOSPACE)
                    .width(Length::FillPortion(4)),
                text("List Dir")
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(80.0)),
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
                        background: Some(palette.background.weak.color.into()),
                        ..Default::default()
                    }
                });

            let rows: Vec<Element<'_, Message>> = self
                .cached_config
                .static_backends
                .iter()
                .map(|backend| {
                    row![
                        text(&backend.name)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2)),
                        text(&backend.dir)
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(4)),
                        text(if backend.list_dir { "Yes" } else { "No" })
                            .font(Font::MONOSPACE)
                            .width(Length::Fixed(80.0)),
                    ]
                    .spacing(10)
                    .padding(Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 6.0,
                        left: 10.0,
                    })
                    .into()
                })
                .collect();

            let table = column![header_container]
                .push(Column::with_children(rows).spacing(2))
                .width(Length::Fill);

            sections.push(
                column![
                    text("Static File Backends"),
                    container(table).width(Length::Fill).style(|theme: &Theme| {
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
                ]
                .spacing(10)
                .into(),
            );
        }

        if sections.is_empty() {
            return text("No backends configured")
                .color(self.theme().extended_palette().background.strong.text)
                .into();
        }

        Column::with_children(sections).spacing(20).into()
    }
}
