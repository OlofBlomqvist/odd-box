use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input,
};
use iced::{Border, Color, Element, Length, Theme};

use super::super::{BackendOption, Message, OddBoxGui};

fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.extended_palette().background.weak.text),
        ..Default::default()
    }
}

impl OddBoxGui {
    pub(in crate::gui) fn view_edit_frontend(&self) -> Element<'_, Message> {
        let mut errors: Vec<Element<'_, Message>> = Vec::new();
        if self.edit_frontend_form.hostname.trim().is_empty() {
            errors.push(
                text("Hostname is required.")
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if self.edit_frontend_form.backend.trim().is_empty() {
            errors.push(
                text("Backend is required.")
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if !self
            .backend_names
            .contains(&self.edit_frontend_form.backend)
        {
            errors.push(
                text("Selected backend does not exist.")
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if self.edit_frontend_form.capture_subdomains && self.edit_frontend_form.lets_encrypt {
            errors.push(
                text("LetsEncrypt cannot be used with capture subdomains.")
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        let notice = self
            .edit_frontend_notice
            .as_ref()
            .map(|msg| text(msg).size(super::super::text_size(13)).style(muted_text));

        let mut actions_children: Vec<Element<'_, Message>> = vec![
            button(text("Save").size(super::super::text_size(14)))
                .on_press(Message::EditFrontendSave)
                .into(),
            button(text("Back").size(super::super::text_size(14)))
                .on_press(Message::NavigateTo(super::super::Page::Frontends))
                .into(),
        ];
        if !self.edit_frontend_is_new {
            actions_children.push(
                button(text("Delete").size(super::super::text_size(14)))
                    .on_press(Message::EditFrontendDelete)
                    .into(),
            );
        }
        let actions = row::Row::with_children(actions_children).spacing(10);

        let layout = responsive(|size| {
            let hostname_label = text("Hostname").size(super::super::text_size(13)).style(muted_text);
            let hostname_help = text("The public host this route matches (e.g. example.local)")
                .size(super::super::text_size(12))
                .style(muted_text);
            let hostname_input = text_input("example.local", &self.edit_frontend_form.hostname)
                .on_input(Message::EditFrontendHostChanged)
                .padding(8)
                .width(Length::Fill);

            let mut backend_options: Vec<BackendOption> = Vec::new();
            backend_options.extend(
                self.cached_config
                    .processes
                    .iter()
                    .map(|p| BackendOption::new(p.name.clone(), "process")),
            );
            backend_options.extend(
                self.cached_config
                    .remote_backends
                    .iter()
                    .map(|b| BackendOption::new(b.name.clone(), "remote")),
            );
            backend_options.extend(
                self.cached_config
                    .static_backends
                    .iter()
                    .map(|b| BackendOption::new(b.name.clone(), "static")),
            );
            backend_options.sort_by(|a, b| a.id.cmp(&b.id));

            let backend_selected = backend_options
                .iter()
                .find(|o| o.id == self.edit_frontend_form.backend)
                .cloned();

            let backend_label = text("Backend").size(super::super::text_size(13)).style(muted_text);
            let backend_help = text("Select which backend handles requests for this host")
                .size(super::super::text_size(12))
                .style(muted_text);
            let backend_picker = pick_list(
                backend_options,
                backend_selected,
                Message::EditFrontendBackendChanged,
            )
            .placeholder("Select backend")
            .padding(8)
            .width(Length::Fill);

            let capture_toggle = checkbox(self.edit_frontend_form.capture_subdomains)
                .label("Capture subdomains")
                .on_toggle(Message::EditFrontendCaptureSubdomainsToggled);
            let capture_help = text("Match *.example.com as well as the root host")
                .size(super::super::text_size(12))
                .style(muted_text);

            let forward_toggle = checkbox(self.edit_frontend_form.forward_subdomains)
                .label("Forward subdomains")
                .on_toggle(Message::EditFrontendForwardSubdomainsToggled);
            let forward_help = text("Preserve the subdomain in the upstream host header")
                .size(super::super::text_size(12))
                .style(muted_text);

            let redirect_toggle = checkbox(self.edit_frontend_form.redirect_to_https)
                .label("Redirect HTTP to HTTPS")
                .on_toggle(Message::EditFrontendRedirectHttpsToggled);
            let redirect_help = text("Send HTTP requests to HTTPS for this host")
                .size(super::super::text_size(12))
                .style(muted_text);

            let lets_encrypt_toggle = checkbox(self.edit_frontend_form.lets_encrypt)
                .label("Use Let's Encrypt")
                .on_toggle(Message::EditFrontendLetsEncryptToggled);
            let lets_encrypt_help = text("Enable ACME certificates for this host")
                .size(super::super::text_size(12))
                .style(muted_text);

            let fields = column![
                hostname_label,
                hostname_input,
                hostname_help,
                backend_label,
                backend_picker,
                backend_help,
            ]
            .spacing(6);

            let options = column![
                text("Options").size(super::super::text_size(14)).style(muted_text),
                capture_toggle,
                capture_help,
                forward_toggle,
                forward_help,
                redirect_toggle,
                redirect_help,
                lets_encrypt_toggle,
                lets_encrypt_help,
            ]
            .spacing(6);

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

            let fields_card = container(fields).padding(12).style(card_style);

            let options_card = container(options).padding(12).style(card_style);

            if size.width < 760.0 {
                column![
                    fields_card.width(Length::Fill),
                    options_card.width(Length::Fill)
                ]
                .spacing(12)
                .width(Length::Fill)
                .into()
            } else {
                row![
                    fields_card.width(Length::FillPortion(3)),
                    options_card.width(Length::FillPortion(2))
                ]
                .spacing(12)
                .width(Length::Fill)
                .into()
            }
        });

        let mut content = column![
            text("Edit Frontend").size(super::super::text_size(20)),
            text("Route settings (HTTP frontend)")
                .size(super::super::text_size(13))
                .style(muted_text),
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
