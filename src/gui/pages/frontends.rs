use iced::Element;
use iced::widget::{button, column, row, text};

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, text_cell},
};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_frontends(&self) -> Element<'_, Message> {
        let actions = row![
            button(text("Add Route").size(14)).on_press(Message::OpenNewFrontend),
        ]
        .spacing(8);

        if self.cached_config.routes.is_empty() {
            return column![
                actions,
                text("No routes configured")
                    .color(self.theme().extended_palette().background.strong.text)
            ]
            .spacing(12)
            .into();
        }

        let columns = vec![
            TableColumn::portion("Hostname", 3),
            TableColumn::portion("Backend", 2),
            TableColumn::fixed("HTTPS Redirect", 120.0),
            TableColumn::fixed("Subdomains", 100.0),
        ];

        let mut table = Table::new(columns);

        for route in &self.cached_config.routes {
            table = table.push_row_with_message(
                vec![
                    text_cell(&route.hostname),
                    text_cell(&route.backend),
                    bool_cell(route.https_redirect),
                    bool_cell(route.capture_subdomains),
                ],
                Message::OpenEditFrontend(route.hostname.clone()),
            );
        }

        column![actions, table.build()].spacing(12).into()
    }
}
