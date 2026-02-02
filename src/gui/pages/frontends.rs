use iced::Element;
use iced::widget::text;

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, text_cell},
};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_frontends(&self) -> Element<'_, Message> {
        if self.cached_config.routes.is_empty() {
            return text("No routes configured")
                .color(self.theme().extended_palette().background.strong.text)
                .into();
        }

        let columns = vec![
            TableColumn::portion("Hostname", 3),
            TableColumn::portion("Backend", 2),
            TableColumn::fixed("HTTPS Redirect", 120.0),
            TableColumn::fixed("Subdomains", 100.0),
        ];

        let mut table = Table::new(columns).hover(Message::NoOp);

        for route in &self.cached_config.routes {
            table = table.push_row(vec![
                text_cell(&route.hostname),
                text_cell(&route.backend),
                bool_cell(route.https_redirect),
                bool_cell(route.capture_subdomains),
            ]);
        }

        table.build()
    }
}
