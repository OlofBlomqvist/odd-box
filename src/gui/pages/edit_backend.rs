use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input,
};
use iced::{Border, Color, Element, Length, Theme};

use super::super::{
    BackendKind, EditBackendField, Message, OddBoxGui,
};

const PROTOCOL_OPTIONS: [crate::configuration::v4::Protocol; 4] = [
    crate::configuration::v4::Protocol::H1,
    crate::configuration::v4::Protocol::H2,
    crate::configuration::v4::Protocol::H2C,
    crate::configuration::v4::Protocol::H2CPK,
];

fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.extended_palette().background.weak.text),
        ..Default::default()
    }
}

impl OddBoxGui {
    pub(in crate::gui) fn view_edit_backend(&self) -> Element<'_, Message> {
        let mut errors: Vec<Element<'_, Message>> = Vec::new();

        if self.edit_backend_form.id.trim().is_empty() {
            errors.push(
                text("Backend id is required.")
                    .size(12)
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Remote)
            && self.edit_backend_form.endpoints.trim().is_empty()
        {
            errors.push(
                text("At least one endpoint is required.")
                    .size(12)
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Static)
            && self.edit_backend_form.dir.trim().is_empty()
        {
            errors.push(
                text("Directory is required.")
                    .size(12)
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Static)
            && !self.edit_backend_form.cache_max_age.trim().is_empty()
            && self.edit_backend_form.cache_max_age.trim().parse::<u64>().is_err()
        {
            errors.push(
                text("Cache max-age must be a number.")
                    .size(12)
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        let notice = self
            .edit_backend_notice
            .as_ref()
            .map(|msg| text(msg).size(13).style(muted_text));

        let actions = row![
            button(text("Save").size(14)).on_press(Message::EditBackendSave),
            button(text("Back").size(14))
                .on_press(Message::NavigateTo(super::super::Page::Backends)),
        ]
        .spacing(10);

        let layout = responsive(|size| {
            let card_style = |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weaker.color.into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: palette.background.strong.color,
                    },
                    ..Default::default()
                }
            };

            let kind_label = match self.edit_backend_form.kind {
                BackendKind::Remote => "Remote Backend",
                BackendKind::Static => "Static Backend",
                BackendKind::Process => "Process Backend",
                BackendKind::Unknown => "Unknown Backend",
            };

            let header = column![
                text(kind_label).size(14).style(muted_text),
                text(format!("ID: {}", self.edit_backend_form.id))
                    .size(12)
                    .style(muted_text),
            ]
            .spacing(4);

            let (fields_card, info_card) = match self.edit_backend_form.kind {
                BackendKind::Remote => {
                    let endpoints_label = text("Endpoints").size(13).style(muted_text);
                    let endpoints_help = text("Comma-separated host:port list")
                        .size(12)
                        .style(muted_text);
                    let endpoints_input = text_input(
                        "example.com:80, 10.0.0.5:8080",
                        &self.edit_backend_form.endpoints,
                    )
                    .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Endpoints(v)))
                    .padding(8)
                    .width(Length::Fill);

                    let protocol_label = text("Protocol").size(13).style(muted_text);
                    let protocol_help = text("Upstream protocol (h1, h2, h2c, h2cpk)")
                        .size(12)
                        .style(muted_text);
                    let protocol_picker = pick_list(
                        PROTOCOL_OPTIONS.as_slice(),
                        Some(self.edit_backend_form.protocol.clone()),
                        |v| Message::EditBackendFieldChanged(EditBackendField::Protocol(v)),
                    )
                    .padding(8)
                    .width(Length::Fill);

                    let https_toggle = checkbox(self.edit_backend_form.https)
                        .label("Upstream uses HTTPS")
                        .on_toggle(|v| Message::EditBackendFieldChanged(EditBackendField::Https(v)));
                    let https_help = text("Enable if endpoints are HTTPS")
                        .size(12)
                        .style(muted_text);

                    let keep_host_toggle = checkbox(self.edit_backend_form.keep_original_host_header)
                        .label("Keep original Host header")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::KeepOriginalHostHeader(v))
                        });
                    let keep_host_help = text("Forward the incoming Host header to upstream")
                        .size(12)
                        .style(muted_text);

                    let fields = column![
                        header,
                        endpoints_label,
                        endpoints_input,
                        endpoints_help,
                        protocol_label,
                        protocol_picker,
                        protocol_help,
                        https_toggle,
                        https_help,
                        keep_host_toggle,
                        keep_host_help,
                    ]
                    .spacing(6);

                    let info = column![
                        text("Remote Backend").size(14).style(muted_text),
                        text("Routes to one or more upstream servers.")
                            .size(12)
                            .style(muted_text),
                        text("Use commas to list multiple endpoints.")
                            .size(12)
                            .style(muted_text),
                    ]
                    .spacing(6);

                    (
                        container(fields)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info)
                                .padding(12)
                                .style(card_style)
                                .width(Length::Fill),
                        ),
                    )
                }
                BackendKind::Static => {
                    let dir_label = text("Directory").size(13).style(muted_text);
                    let dir_help = text("Folder path to serve files from")
                        .size(12)
                        .style(muted_text);
                    let dir_input = text_input("/var/www/site", &self.edit_backend_form.dir)
                        .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Dir(v)))
                        .padding(8)
                        .width(Length::Fill);
                    let dir_browse = button(text("Browse").size(12))
                        .padding(8)
                        .on_press(Message::EditBackendPickDir);
                    let resolved_dir = self.edit_backend_resolved_dir.as_ref().map(|path| {
                        text(format!("Resolved: {}", path))
                            .size(12)
                            .style(muted_text)
                    });
                    let resolve_error = self.edit_backend_resolve_error.as_ref().map(|err| {
                        text(err)
                            .size(12)
                            .color(Color::from_rgb(0.9, 0.3, 0.3))
                    });
                    let has_vars = self.edit_backend_form.dir.contains("$root_dir")
                        || self.edit_backend_form.dir.contains("$cfg_dir")
                        || self.edit_backend_form.dir.contains('~');
                    let vars_help = column![
                        text("Available variables:").size(12).style(muted_text),
                        text("$root_dir  Project root directory").size(12).style(muted_text),
                        text("$cfg_dir   Directory of the config file").size(12).style(muted_text),
                        text("~          Home directory").size(12).style(muted_text),
                    ]
                    .spacing(2);

                    let list_toggle = checkbox(self.edit_backend_form.list_dir)
                        .label("Enable directory listing")
                        .on_toggle(|v| Message::EditBackendFieldChanged(EditBackendField::ListDir(v)));

                    let render_toggle = checkbox(self.edit_backend_form.render_markdown)
                        .label("Render markdown")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::RenderMarkdown(v))
                        });

                    let cache_label = text("Cache max-age (seconds)")
                        .size(13)
                        .style(muted_text);
                    let cache_input = text_input("3600", &self.edit_backend_form.cache_max_age)
                        .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::CacheMaxAge(v)))
                        .padding(8)
                        .width(Length::Fill);
                    let cache_help = text("Sets Cache-Control: max-age=<seconds> on responses")
                        .size(12)
                        .style(muted_text);

                    let mut path_box = column![
                        header,
                        dir_label,
                        row![dir_input, dir_browse].spacing(8),
                        dir_help,
                    ]
                    .spacing(6);
                    if let Some(err) = resolve_error {
                        path_box = path_box.push(err);
                    } else if let Some(resolved) = resolved_dir {
                        path_box = path_box.push(resolved);
                    } else if !has_vars {
                        path_box = path_box.push(vars_help);
                    }

                    let options_box = column![
                        text("Options").size(14).style(muted_text),
                        list_toggle,
                        render_toggle,
                        cache_label,
                        cache_input,
                        cache_help,
                    ]
                    .spacing(6);

                    let fields = column![
                        container(path_box)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                        container(options_box)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                    ]
                    .spacing(10);

                    (
                        container(fields)
                            .width(Length::Fill)
                            .style(|_| container::Style::default()),
                        None,
                    )
                }
                BackendKind::Process => {
                    let note = text("Process backends are edited on the Managed Processes page.")
                        .size(12)
                        .style(muted_text);
                    let fields = column![header, note].spacing(6);
                    let info = column![
                        text("Process Backend").size(14).style(muted_text),
                        text("This editor does not modify process configuration.")
                            .size(12)
                            .style(muted_text),
                    ]
                    .spacing(6);
                    (
                        container(fields)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info)
                                .padding(12)
                                .style(card_style)
                                .width(Length::Fill),
                        ),
                    )
                }
                BackendKind::Unknown => {
                    let note = text("Backend not found.").size(12).style(muted_text);
                    let fields = column![header, note].spacing(6);
                    let info = column![
                        text("Unknown Backend").size(14).style(muted_text),
                        text("Select a backend from the Backends list.")
                            .size(12)
                            .style(muted_text),
                    ]
                    .spacing(6);
                    (
                        container(fields)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info)
                                .padding(12)
                                .style(card_style)
                                .width(Length::Fill),
                        ),
                    )
                }
            };

            if size.width < 760.0 {
                if let Some(info_card) = info_card {
                    column![fields_card, info_card]
                        .spacing(12)
                        .width(Length::Fill)
                        .into()
                } else {
                    column![fields_card]
                        .spacing(12)
                        .width(Length::Fill)
                        .into()
                }
            } else {
                if let Some(info_card) = info_card {
                    row![
                        fields_card.width(Length::FillPortion(3)),
                        info_card.width(Length::FillPortion(2))
                    ]
                    .spacing(12)
                    .width(Length::Fill)
                    .into()
                } else {
                    column![fields_card]
                        .spacing(12)
                        .width(Length::Fill)
                        .into()
                }
            }
        });

        let mut content = column![
            text("Edit Backend").size(20),
            text("Backend settings").size(13).style(muted_text),
            layout,
            actions,
        ]
        .spacing(12);

        for err in errors {
            content = content.push(err);
        }

        if let Some(n) = notice {
            content = content.push(n);
        }

        container(content).width(Length::Fill).into()
    }
}
