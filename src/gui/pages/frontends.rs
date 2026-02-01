use iced::widget::{column, container, row, text, Column};
use iced::{Border, Element, Font, Length, Padding, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_frontends(&self) -> Element<'_, Message> {
        if self.cached_config.routes.is_empty() {
            return text("No routes configured")
                .color(self.theme().extended_palette().background.strong.text)
                .into();
        }

        // Table header
        let header = row![
            text("Hostname")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(3)),
            text("Backend")
                .font(Font::MONOSPACE)
                .width(Length::FillPortion(2)),
            text("HTTPS Redirect")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(120.0)),
            text("Subdomains")
                .font(Font::MONOSPACE)
                .width(Length::Fixed(100.0)),
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
            .style(container::rounded_box);

        // Table rows
        let rows: Vec<Element<'_, Message>> = self
            .cached_config
            .routes
            .iter()
            .map(|route| {
                row![
                    text(&route.hostname)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(3)),
                    text(&route.backend)
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2)),
                    text(if route.https_redirect { "Yes" } else { "No" })
                        .font(Font::MONOSPACE)
                        .width(Length::Fixed(120.0)),
                    text(if route.capture_subdomains {
                        "Yes"
                    } else {
                        "No"
                    })
                    .font(Font::MONOSPACE)
                    .width(Length::Fixed(100.0)),
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
