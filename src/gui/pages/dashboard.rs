use iced::widget::{column, container, text};
use iced::{Border, Color, Element, Length, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| format!("{:.0?}", d))
            .unwrap_or_else(|_| "Unknown".to_string());

        let status_items = column![
            text("Status: Running").color(Color::from_rgb(0.4, 0.8, 0.4)),
            text(format!("Uptime: {}", uptime)).color(Color::from_rgb(0.8, 0.8, 0.8)),
        ]
        .spacing(8);

        container(status_items)
            .padding(20)
            .width(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(Color::from_rgb(0.14, 0.14, 0.16).into()),
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: Color::from_rgb(0.2, 0.2, 0.22),
                },
                ..Default::default()
            })
            .into()
    }
}
