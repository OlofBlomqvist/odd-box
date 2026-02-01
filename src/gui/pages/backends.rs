use clap::builder::styling::RgbColor;
use iced::theme::palette;
use iced::widget::{column, text, Column};
use iced::{Color, Element};

use crate::gui::components::{
    table::{bool_cell, text_cell},
    Column as TableColumn, Table,
};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();

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
                table = table.push_row(vec![
                    text_cell(&backend.name),
                    text_cell(&backend.endpoints),
                    text_cell(&backend.protocol),
                    bool_cell(backend.https),
                ]);
            }

            sections.push(
                column![text("Remote Backends").color(Color {
                    r: 155.0,
                    g: 155.0,
                    b: 0.0,
                    a: 1.0,
                }), table.build()]
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
                table = table.push_row(vec![
                    text_cell(&backend.name),
                    text_cell(&backend.dir),
                    bool_cell(backend.list_dir),
                ]);
            }

            sections.push(
                column![text("Static File Backends"), table.build()]
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
