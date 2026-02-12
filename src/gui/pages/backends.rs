use iced::widget::{Column, button, column, row, text};
use iced::{Color, Element};

use crate::gui::components::{
    Column as TableColumn, Table,
    table::{bool_cell, colored_text_cell, text_cell, wrap_text_cell},
};

use super::super::{Message, OddBoxGui};
use crate::global_state::ProcState;

impl OddBoxGui {
    pub(in crate::gui) fn view_backends(&self) -> Element<'_, Message> {
        let mut sections: Vec<Element<'_, Message>> = Vec::new();
        let use_kde_buttons = self.use_kde_system_styles();
        let table_theme = self.theme();
        let table_header_bg = self.surface_panel_alt_bg(&table_theme);
        let table_row_even_bg = self.surface_panel_bg(&table_theme);
        let table_row_odd_bg = self.surface_panel_alt_bg(&table_theme);
        let table_border = self.surface_border_color(&table_theme);

        let mut add_remote_btn = button(text("Add Remote").size(super::super::text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Remote));
        let mut add_static_btn = button(text("Add Static").size(super::super::text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Static));
        let mut add_process_btn = button(text("Add Process").size(super::super::text_size(14)))
            .on_press(Message::OpenNewBackend(super::super::BackendKind::Process));
        let mut start_all_btn =
            button(text("Start All").size(super::super::text_size(14))).on_press(Message::ProcessStartAll);
        let mut stop_all_btn =
            button(text("Stop All").size(super::super::text_size(14))).on_press(Message::ProcessStopAll);

        if use_kde_buttons {
            add_remote_btn = add_remote_btn.style(super::super::kde_neutral_button_style);
            add_static_btn = add_static_btn.style(super::super::kde_neutral_button_style);
            add_process_btn = add_process_btn.style(super::super::kde_neutral_button_style);
            start_all_btn = start_all_btn.style(super::super::kde_success_button_style);
            stop_all_btn = stop_all_btn.style(super::super::kde_danger_button_style);
        }

        let actions = row![
            add_remote_btn,
            add_static_btn,
            add_process_btn,
            start_all_btn,
            stop_all_btn,
        ]
        .spacing(8);

        // Process backends section
        if !self.cached_config.processes.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Binary", 2),
                TableColumn::fixed("Port", 80.0),
                TableColumn::fixed("Protocol", 80.0),
                TableColumn::fixed("State", 140.0),
                TableColumn::fixed("Auto", 60.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for proc in &self.cached_config.processes {
                let state_color = match proc.state {
                    ProcState::Running => Color::from_rgb(0.4, 0.85, 0.4),
                    ProcState::Starting | ProcState::Stopping => Color::from_rgb(1.0, 0.8, 0.3),
                    ProcState::Stopped => Color::from_rgb(0.5, 0.5, 0.5),
                    ProcState::Faulty => Color::from_rgb(1.0, 0.4, 0.4),
                    _ => Color::from_rgb(0.6, 0.6, 0.6),
                };
                let state_cell = colored_text_cell(format!("{:?}", proc.state), state_color);
                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&proc.name),
                        text_cell(&proc.bin),
                        text_cell(&proc.port),
                        text_cell(&proc.protocol),
                        state_cell,
                        bool_cell(proc.auto_start),
                    ],
                    Message::OpenEditBackend(proc.name.clone()),
                );
            }

            sections.push(
                column![text("Process Backends"), table.build()]
                    .spacing(10)
                    .into(),
            );
        }

        // Remote backends section
        if !self.cached_config.remote_backends.is_empty() {
            let columns = vec![
                TableColumn::portion("Name", 2),
                TableColumn::portion("Endpoints", 3),
                TableColumn::fixed("Protocol", 80.0),
                TableColumn::fixed("HTTPS", 60.0),
            ];

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for backend in &self.cached_config.remote_backends {
                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&backend.name),
                        text_cell(&backend.endpoints),
                        text_cell(&backend.protocol),
                        bool_cell(backend.https),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![text("Remote Backends"), table.build()]
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

            let mut table = Table::new(columns).surface_colors(
                table_header_bg,
                table_row_even_bg,
                table_row_odd_bg,
                table_border,
            );

            for backend in &self.cached_config.static_backends {
                table = table.push_row_with_message(
                    vec![
                        wrap_text_cell(&backend.name),
                        text_cell(&backend.dir),
                        bool_cell(backend.list_dir),
                    ],
                    Message::OpenEditBackend(backend.name.clone()),
                );
            }

            sections.push(
                column![text("Static Backends"), table.build()]
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
