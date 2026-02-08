use iced::widget::{column, container, text};
use iced::{Element, Length, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_edit_backend(&self) -> Element<'_, Message> {
        let target = self
            .edit_target
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("(no selection)");

        let content = column![
            text(format!("Editing backend: {}", target)).size(18),
            text("TODO:")
                .size(14)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
            text("- UI for backend endpoints and protocol")
                .size(13)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
            text("- Validation and save actions")
                .size(13)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
        ]
        .spacing(6);

        container(content)
            .width(Length::Fill)
            .into()
    }
}
