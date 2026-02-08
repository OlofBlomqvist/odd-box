use clap::builder::styling::RgbColor;
use iced::theme::palette;
use iced::widget::{Column, button, column, row, text};
use iced::{Color, Element};

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, text_cell},
};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();
        let actions = row![
            button(text("Add Remote").size(14))
                .on_press(Message::OpenNewBackend(super::super::BackendKind::Remote)),
            button(text("Add Static").size(14))
                .on_press(Message::OpenNewBackend(super::super::BackendKind::Static)),
            button(text("Add Process").size(14))
                .on_press(Message::OpenNewBackend(super::super::BackendKind::Process)),
        ]
        .spacing(8);

        // Remote backends section
        if !self.cached_config.remote_backends.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Endpoints", 3),
                TableColumn::fixed("Protocol", 80.0),
                TableColumn::fixed("HTTPS", 60.0),
            ];

            let mut table = Table::new(columns);

            for backend in &self.cached_config.remote_backends {
                table = table.push_row_with_message(
                    vec![
                        text_cell(&backend.name),
                        text_cell(&backend.endpoints),
                        text_cell(&backend.protocol),
                        bool_cell(backend.https),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![
                    text("Remote Backends"),
                    table.build()
                ]
                .spacing(10)
                .into(),
            );
        }

        // Static backends section
        if !self.cached_config.static_backends.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Directory", 4),
                TableColumn::fixed("List Dir", 80.0),
            ];

            let mut table = Table::new(columns);

            for backend in &self.cached_config.static_backends {
                table = table.push_row_with_message(
                    vec![
                        text_cell(&backend.name),
                        text_cell(&backend.dir),
                        bool_cell(backend.list_dir),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![text("Static File Backends"), table.build()]
                    .spacing(10)
                    .into(),
            );
        }

        if sections.is_empty() {
            return column![
                actions,
                text("No backends configured")
                    .color(self.theme().extended_palette().background.strong.text)
            ]
            .spacing(12)
            .into();
        }

        let mut content = vec![actions.into()];
        content.extend(sections);
        Column::with_children(content).spacing(20).into()
    }
}
