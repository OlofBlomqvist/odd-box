use iced::widget::{column, container, text};
use iced::{Border, Element, Length, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_dashboard(&self) -> Element<'_, Message> {
        let uptime = self
            .state
            .uptime()
            .map(|d| format!("{:.0?}", d))
            .unwrap_or_else(|_| "Unknown".to_string());

        let status_items =
            column![text("Status: Running"), text(format!("Uptime: {}", uptime)),].spacing(8);

        container(status_items)
            .padding(20)
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
