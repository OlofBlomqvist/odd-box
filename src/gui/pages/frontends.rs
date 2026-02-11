use iced::Color;
use iced::Element;
use iced::widget::{button, column, row, text, text_input};

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, text_cell, wrap_text_cell},
};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_frontends(&self) -> Element<'_, Message> {
        let actions =
            row![button(text("Add Route").size(super::super::text_size(14))).on_press(Message::OpenNewFrontend),].spacing(8);

        let http_placeholder = self
            .cached_config
            .http_port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "80".to_string());
        let https_placeholder = self
            .cached_config
            .https_port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "443".to_string());

        let http_input = text_input(&http_placeholder, &self.frontend_http_port_input)
            .on_input(Message::FrontendHttpPortChanged)
            .size(super::super::text_size(14));
        let https_input = text_input(&https_placeholder, &self.frontend_https_port_input)
            .on_input(Message::FrontendHttpsPortChanged)
            .size(super::super::text_size(14));

        let ports_row = row![
            text("HTTP Port").size(super::super::text_size(14)),
            http_input,
            text("HTTPS Port").size(super::super::text_size(14)),
            https_input,
            button(text("Apply Ports").size(super::super::text_size(14))).on_press(Message::FrontendPortsSave),
        ]
        .spacing(8);

        let notice = self.frontend_port_notice.as_ref().map(|msg| {
            text(msg)
                .size(super::super::text_size(13))
                .color(self.theme().extended_palette().background.strong.text)
        });

        if self.cached_config.routes.is_empty() {
            let mut col = column![actions, ports_row];
            if let Some(msg) = notice {
                col = col.push(msg);
            }
            return col
                .push(
                    text("No routes configured")
                        .color(self.theme().extended_palette().background.strong.text),
                )
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
        let mut known_backends: Vec<String> = self
            .cached_config
            .processes
            .iter()
            .map(|p| p.name.clone())
            .chain(
                self.cached_config
                    .remote_backends
                    .iter()
                    .map(|b| b.name.clone()),
            )
            .chain(
                self.cached_config
                    .static_backends
                    .iter()
                    .map(|b| b.name.clone()),
            )
            .collect();
        known_backends.sort();
        known_backends.dedup();

        for route in &self.cached_config.routes {
            let missing_backend = !known_backends.contains(&route.backend);
            let backend_cell = if missing_backend {
                let label = format!("{} (missing)", route.backend);
                crate::gui::components::table::colored_text_cell(
                    label,
                    Color::from_rgb(0.9, 0.3, 0.3),
                )
            } else {
                text_cell(&route.backend)
            };

            table = table.push_row_with_message(
                vec![
                    wrap_text_cell(&route.hostname),
                    backend_cell,
                    bool_cell(route.https_redirect),
                    bool_cell(route.capture_subdomains),
                ],
                Message::OpenEditFrontend(route.hostname.clone()),
            );
        }

        let mut col = column![actions, ports_row];
        if let Some(msg) = notice {
            col = col.push(msg);
        }
        col.push(table.build()).spacing(12).into()
    }
}
