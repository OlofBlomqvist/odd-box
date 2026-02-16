use iced::widget::text::Wrapping;
use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input,
};
use iced::{Border, Color, Element, Length, Theme};

use super::super::{BackendOption, KdeButtonRole, Message, OddBoxGui, scaled, text_size};

/// Returns true if the hostname uses cruma-specific patterns (@ or bare *).
fn uses_cruma_pattern(hostname: &str) -> bool {
    hostname.contains('@') || hostname == "*"
}

fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.extended_palette().background.weak.text),
        ..Default::default()
    }
}

impl OddBoxGui {
    pub(in crate::gui) fn view_edit_frontend(&self) -> Element<'_, Message> {
        let mut errors: Vec<Element<'_, Message>> = Vec::new();
        let mut warnings: Vec<Element<'_, Message>> = Vec::new();
        let hostname = self.edit_frontend_form.hostname.trim();
        let cruma_global = self.cached_config.cruma_globally_enabled;
        let cruma_on_route = self.edit_frontend_form.enable_cruma;

        if hostname.is_empty() {
            errors.push(
                text("Hostname is required.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if hostname.contains("://") {
            errors.push(
                text("Hostname must not include a protocol (e.g. remove http:// or https://).")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if hostname.contains(':') {
            errors.push(
                text("Hostname must not include a port. Use the Frontends page to set ports.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if hostname.contains('/') || hostname.contains('?') || hostname.contains('#') {
            errors.push(
                text("Hostname must not contain path, query, or fragment characters (/, ?, #).")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if hostname.contains(' ') {
            errors.push(
                text("Hostname must not contain spaces.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if !hostname.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '*' || c == '@'
        }) {
            errors.push(
                text("Hostname contains invalid characters. Only letters, digits, hyphens, underscores, dots, * and @ are allowed.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if uses_cruma_pattern(hostname) && !cruma_global {
            // Cruma-specific pattern used but cruma is globally disabled
            warnings.push(
                text("⚠ This hostname uses a cruma pattern (@ or *) but cruma is disabled in the global config. The pattern will not work until cruma is enabled.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.7, 0.2))
                    .into(),
            );
        }

        // Warn if "Enable Cruma" is checked on this route but cruma is globally disabled
        if cruma_on_route && !cruma_global {
            warnings.push(
                text("⚠ \"Enable Cruma\" is checked but cruma is disabled in the global config. Enable cruma on the Cruma Ingress page for this to take effect.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.7, 0.2))
                    .into(),
            );
        }
        if self.edit_frontend_form.backend.trim().is_empty() {
            errors.push(
                text("Backend is required.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if !self
            .backend_names
            .contains(&self.edit_frontend_form.backend)
        {
            errors.push(
                text("Selected backend does not exist.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if self.edit_frontend_form.capture_subdomains && self.edit_frontend_form.lets_encrypt {
            errors.push(
                text("LetsEncrypt cannot be used with capture subdomains.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        let notice = self
            .edit_frontend_notice
            .as_ref()
            .map(|msg| text(msg).size(text_size(13)).style(muted_text));

        let use_kde_buttons = self.use_kde_system_styles();

        let save_btn = button(text("Save").size(text_size(14)))
            .on_press(Message::EditFrontendSave)
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Primary,
                    use_kde_buttons,
                )
            });
        let back_btn = button(text("Back").size(text_size(14)))
            .on_press(Message::NavigateTo(super::super::Page::Frontends))
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Neutral,
                    use_kde_buttons,
                )
            });

        let mut actions_children: Vec<Element<'_, Message>> =
            vec![save_btn.into(), back_btn.into()];
        if !self.edit_frontend_is_new {
            let delete_btn = button(text("Delete").size(text_size(14)))
                .on_press(Message::EditFrontendDelete)
                .style(move |theme, status| {
                    super::super::themed_button_style(
                        theme,
                        status,
                        KdeButtonRole::Danger,
                        use_kde_buttons,
                    )
                });
            actions_children.push(delete_btn.into());
        }
        let actions = row::Row::with_children(actions_children).spacing(scaled(10.0));

        let layout = responsive(|size| {
            let hostname_label = text("Hostname").size(text_size(13)).style(muted_text);
            let hostname_help = text("The public host this route matches (e.g. example.local)")
                .size(text_size(12))
                .style(muted_text);
            let hostname_input = text_input("example.local", &self.edit_frontend_form.hostname)
                .on_input(Message::EditFrontendHostChanged)
                .padding(scaled(8.0))
                .width(Length::Fill);

            // Build help text for hostname patterns based on cruma state
            let pattern_help_lines: Vec<Element<'_, Message>> =
                if self.cached_config.cruma_globally_enabled {
                    vec![
                        text("Hostname patterns (cruma enabled):")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  @  →  matches the assigned cruma domain")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  myapp  →  matches myapp and myapp.<cruma-domain>")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  host.example.com  →  exact FQDN (dot = full domain)")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  *  →  matches any domain")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  *.@  →  any subdomain of the cruma domain")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  example.@  →  example.<cruma-domain>")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                    ]
                } else {
                    vec![
                        text("Hostname patterns:")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  example.local  →  exact hostname match")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                        text("  (Enable cruma globally for @/* pattern support)")
                            .size(text_size(11))
                            .style(muted_text)
                            .into(),
                    ]
                };
            let pattern_help_col =
                iced::widget::Column::with_children(pattern_help_lines).spacing(scaled(2.0));

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

            let backend_label = text("Backend").size(text_size(13)).style(muted_text);
            let backend_help = text("Select which backend handles requests for this host")
                .size(text_size(12))
                .style(muted_text);
            let backend_picker = pick_list(
                backend_options,
                backend_selected,
                Message::EditFrontendBackendChanged,
            )
            .placeholder("Select backend")
            .padding(scaled(8.0))
            .width(Length::Fill);

            let capture_toggle = checkbox(self.edit_frontend_form.capture_subdomains)
                .label("Capture subdomains")
                .on_toggle(Message::EditFrontendCaptureSubdomainsToggled);
            let capture_help = text("Match *.example.com as well as the root host")
                .size(text_size(12))
                .wrapping(Wrapping::Word)
                .width(Length::Fill)
                .style(muted_text);

            let forward_toggle = checkbox(self.edit_frontend_form.forward_subdomains)
                .label("Forward subdomains")
                .on_toggle(Message::EditFrontendForwardSubdomainsToggled);
            let forward_help = text("Preserve the subdomain in the upstream host header")
                .size(text_size(12))
                .wrapping(Wrapping::Word)
                .width(Length::Fill)
                .style(muted_text);

            let redirect_toggle = checkbox(self.edit_frontend_form.redirect_to_https)
                .label("Redirect HTTP to HTTPS")
                .on_toggle(Message::EditFrontendRedirectHttpsToggled);
            let redirect_help = text("Send HTTP requests to HTTPS for this host")
                .size(text_size(12))
                .wrapping(Wrapping::Word)
                .width(Length::Fill)
                .style(muted_text);

            let lets_encrypt_toggle = checkbox(self.edit_frontend_form.lets_encrypt)
                .label("Use Let's Encrypt")
                .on_toggle(Message::EditFrontendLetsEncryptToggled);
            let lets_encrypt_help = text("Enable ACME certificates for this host")
                .size(text_size(12))
                .wrapping(Wrapping::Word)
                .width(Length::Fill)
                .style(muted_text);

            let enable_cruma_toggle = checkbox(self.edit_frontend_form.enable_cruma)
                .label("Enable Cruma")
                .on_toggle(Message::EditFrontendEnableCrumaToggled);
            let enable_cruma_help = text("Expose this route through the cruma tunnel")
                .size(text_size(12))
                .wrapping(Wrapping::Word)
                .width(Length::Fill)
                .style(muted_text);

            // Show the resolved cruma FQDN(s) when cruma is enabled on this
            // route and the tunnel has an assigned domain.
            let cruma_fqdn_info: Option<Element<'_, Message>> =
                if self.edit_frontend_form.enable_cruma {
                    if let Some(ref domain) = self.cached_config.cruma_assigned_domain {
                        let host = self.edit_frontend_form.hostname.trim();
                        let resolved = if host == "@" || host.is_empty() {
                            // @ resolves to the bare cruma domain
                            format!("👻 {}", domain)
                        } else if host == "*" {
                            format!("👻 *  (any domain via cruma)")
                        } else if host.contains('@') {
                            // e.g. "*.@" or "example.@"
                            let expanded = host.replace('@', domain);
                            format!("👻 {}", expanded)
                        } else if host.contains('.') {
                            // FQDN — used as-is
                            format!("👻 {} (FQDN)", host)
                        } else {
                            // single label — matches both bare and under cruma domain
                            format!("👻 {} , {}.{}", host, host, domain)
                        };
                        Some(
                            text(resolved)
                                .size(text_size(12))
                                .wrapping(Wrapping::WordOrGlyph)
                                .width(Length::Fill)
                                .color(Color::from_rgb(0.4, 0.75, 0.95))
                                .into(),
                        )
                    } else if self.cached_config.cruma_globally_enabled {
                        Some(
                            text("👻 Waiting for cruma domain assignment…")
                                .size(text_size(12))
                                .wrapping(Wrapping::WordOrGlyph)
                                .width(Length::Fill)
                                .style(muted_text)
                                .into(),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

            let fields = column![
                hostname_label,
                hostname_input,
                hostname_help,
                pattern_help_col,
                backend_label,
                backend_picker,
                backend_help,
            ]
            .spacing(scaled(6.0));

            let mut options = column![
                text("Options").size(text_size(14)).style(muted_text),
                capture_toggle,
                capture_help,
                forward_toggle,
                forward_help,
                redirect_toggle,
                redirect_help,
                lets_encrypt_toggle,
                lets_encrypt_help,
                enable_cruma_toggle,
                enable_cruma_help,
            ]
            .spacing(scaled(6.0));

            if let Some(fqdn_el) = cruma_fqdn_info {
                options = options.push(fqdn_el);
            }

            let card_style = |theme: &Theme| container::Style {
                background: Some(self.surface_panel_bg(theme).into()),
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
                },
                ..Default::default()
            };

            let fields_card = container(fields).padding(scaled(12.0)).style(card_style);

            let options_card = container(options).padding(scaled(12.0)).style(card_style);

            if size.width < 760.0 {
                column![
                    fields_card.width(Length::Fill),
                    options_card.width(Length::Fill)
                ]
                .spacing(scaled(12.0))
                .width(Length::Fill)
                .into()
            } else {
                row![
                    fields_card.width(Length::FillPortion(3)),
                    options_card.width(Length::FillPortion(2))
                ]
                .spacing(scaled(12.0))
                .width(Length::Fill)
                .into()
            }
        });

        let mut content = column![
            text("Route settings (HTTP frontend)")
                .size(text_size(13))
                .style(muted_text),
            layout,
            actions,
        ]
        .spacing(scaled(12.0));

        for err in errors {
            content = content.push(err);
        }

        for warn in warnings {
            content = content.push(warn);
        }

        if let Some(n) = notice {
            content = content.push(n);
        }

        container(content).width(Length::Fill).into()
    }
}
