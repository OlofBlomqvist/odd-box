use iced::widget::text::Wrapping;
use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme};

use cruma_tunnels_lib::hostname::{HostnamePatternKind, classify_hostname_pattern};

use super::super::{
    BackendOption, FormAuthUserDraft, KdeButtonRole, Message, OddBoxGui, scaled, text_size,
};

fn muted_text(theme: &Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme.extended_palette().background.weak.text),
        ..Default::default()
    }
}

fn view_form_auth_user_row(user: &FormAuthUserDraft) -> Element<'_, Message> {
    let user_id = user.id;
    row![
        text_input("Username", &user.username)
            .on_input(move |v| Message::EditFrontendFormAuthUserNameChanged(user_id, v))
            .padding(Padding::from(6.0))
            .width(Length::FillPortion(1)),
        text_input("Password", &user.password)
            .on_input(move |v| Message::EditFrontendFormAuthUserPasswordChanged(user_id, v))
            .padding(Padding::from(6.0))
            .secure(true)
            .width(Length::FillPortion(1)),
        button(text("✖").size(text_size(12)))
            .on_press(Message::EditFrontendRemoveFormAuthUser(user_id))
            .padding(Padding::from(6.0))
            .style(|theme: &Theme, status| {
                let p = theme.extended_palette();
                let base = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => p.danger.strong.color,
                    _ => p.danger.base.color,
                };
                iced::widget::button::Style {
                    background: Some(base.into()),
                    text_color: p.danger.strong.text,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
    ]
    .spacing(scaled(6.0))
    .align_y(Alignment::Center)
    .into()
}

fn view_form_auth_section<'a>(
    users: &'a [FormAuthUserDraft],
    use_kde_buttons: bool,
) -> Element<'a, Message> {
    let header = text("Form Auth Users")
        .size(text_size(13))
        .style(muted_text);
    let help = text(
        "Requests to this route will require login via an HTML form. \
         Leave empty to disable form auth.",
    )
    .size(text_size(12))
    .wrapping(Wrapping::Word)
    .width(Length::Fill)
    .style(muted_text);

    let mut col = column![header, help].spacing(scaled(6.0));

    for user in users {
        col = col.push(view_form_auth_user_row(user));
    }

    let add_btn = button(text("+ Add user").size(text_size(12)))
        .on_press(Message::EditFrontendAddFormAuthUser)
        .padding(Padding {
            top: scaled(5.0),
            right: scaled(10.0),
            bottom: scaled(5.0),
            left: scaled(10.0),
        })
        .style(move |theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Neutral,
                use_kde_buttons,
            )
        });

    col = col.push(add_btn);
    col.into()
}

impl OddBoxGui {
    pub(in crate::gui) fn view_edit_frontend(&self) -> Element<'_, Message> {
        let mut errors: Vec<Element<'_, Message>> = Vec::new();
        let mut warnings: Vec<Element<'_, Message>> = Vec::new();
        let hostname = self.edit_frontend_form.hostname.trim();
        let cruma_global = self.cached_config.cruma_globally_enabled;
        let cruma_on_route = self.edit_frontend_form.enable_cruma;

        // Use the SDK to classify and validate the hostname pattern.
        let assigned_fqdn: Option<String> = self.cached_config.cruma_assigned_domain.clone();
        let pattern_info = classify_hostname_pattern(hostname, &assigned_fqdn);

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
        } else if !pattern_info.is_valid {
            errors.push(
                text(format!("⚠ {}", pattern_info.explanation))
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        } else if !hostname.is_empty() {
            // Check if the pattern requires cruma but cruma is globally disabled
            let needs_cruma = matches!(
                pattern_info.kind,
                HostnamePatternKind::DefaultRoute
                    | HostnamePatternKind::SubLabel
                    | HostnamePatternKind::WildcardAll
                    | HostnamePatternKind::WildcardSubLabel
                    | HostnamePatternKind::Pending
            );
            if needs_cruma && !cruma_global {
                warnings.push(
                    text("⚠ This hostname pattern relies on a cruma tunnel domain but cruma is disabled in the global config. Enable cruma for it to work.")
                        .size(text_size(12))
                        .color(Color::from_rgb(0.9, 0.7, 0.2))
                        .into(),
                );
            }
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

        let layout = responsive(move |size| {
            let hostname_label = text("Hostname").size(text_size(13)).style(muted_text);
            let hostname_input = text_input("example.local", &self.edit_frontend_form.hostname)
                .on_input(Message::EditFrontendHostChanged)
                .padding(scaled(8.0))
                .width(Length::Fill);

            // Classify the hostname pattern using the SDK inside the closure
            // so all values are owned and satisfy the 'static bound.
            let hostname = self.edit_frontend_form.hostname.trim();
            let assigned_fqdn: Option<String> = self.cached_config.cruma_assigned_domain.clone();
            let pattern_info = classify_hostname_pattern(hostname, &assigned_fqdn);

            // Show the SDK classification for the current hostname when non-empty.
            let pattern_explainer: Option<Element<'_, Message>> =
                if !hostname.is_empty() && pattern_info.is_valid {
                    Some(
                        text(format!("Pattern: {}", pattern_info.explanation))
                            .size(text_size(11))
                            .color(Color::from_rgb(0.4, 0.75, 0.95))
                            .into(),
                    )
                } else {
                    None
                };

            // Show the resolved cruma FQDN directly under hostname so we do not
            // duplicate the same information again in the "Options" card.
            let cruma_fqdn_info: Option<Element<'_, Message>> =
                if self.edit_frontend_form.enable_cruma && !hostname.is_empty() {
                    match pattern_info.kind {
                        HostnamePatternKind::Pending => Some(
                            text(format!(
                                "👻 {} - waiting for domain assignment...",
                                pattern_info.display_label
                            ))
                            .size(text_size(12))
                            .wrapping(Wrapping::WordOrGlyph)
                            .width(Length::Fill)
                            .style(muted_text)
                            .into(),
                        ),
                        HostnamePatternKind::Invalid => None,
                        _ if assigned_fqdn.is_some() => Some(
                            text(format!("👻 {}", pattern_info.display_label))
                                .size(text_size(12))
                                .wrapping(Wrapping::WordOrGlyph)
                                .width(Length::Fill)
                                .color(Color::from_rgb(0.4, 0.75, 0.95))
                                .into(),
                        ),
                        _ if self.cached_config.cruma_globally_enabled => Some(
                            text("👻 Waiting for cruma domain assignment...")
                                .size(text_size(12))
                                .wrapping(Wrapping::WordOrGlyph)
                                .width(Length::Fill)
                                .style(muted_text)
                                .into(),
                        ),
                        _ => None,
                    }
                } else {
                    None
                };

            // Collapsible hostname tips, matching the tunnel-agent editor style.
            let tips_toggle_label = if self.edit_frontend_hostname_tips_expanded {
                "i Hostname pattern tips v"
            } else {
                "i Hostname pattern tips >"
            };
            let tips_toggle_btn = button(
                text(tips_toggle_label)
                    .size(text_size(11))
                    .style(muted_text),
            )
            .on_press(Message::EditFrontendToggleHostnameTips)
            .padding([4.0, 0.0])
            .style(|theme: &Theme, _status| iced::widget::button::Style {
                background: None,
                border: Border::default(),
                text_color: theme.extended_palette().background.weak.text,
                ..Default::default()
            });

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

            let mut hostname_details_children: Vec<Element<'_, Message>> = vec![
                text("The public host this route matches (e.g. example.local)")
                    .size(text_size(12))
                    .style(muted_text)
                    .into(),
            ];
            if let Some(pattern) = pattern_explainer {
                hostname_details_children.push(pattern);
            }
            if let Some(fqdn) = cruma_fqdn_info {
                hostname_details_children.push(fqdn);
            }
            hostname_details_children.push(tips_toggle_btn.into());

            let mut hostname_details =
                iced::widget::Column::with_children(hostname_details_children).spacing(scaled(4.0));

            if self.edit_frontend_hostname_tips_expanded {
                let mut pattern_tips = column![].spacing(scaled(2.0));
                if self.edit_frontend_form.enable_cruma {
                    pattern_tips = pattern_tips
                        .push(
                            text("  @ or empty - default route using assigned cruma domain")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  * - match any hostname (catch-all)")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  app - single label, becomes app.<cruma-domain>")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  *.app - wildcard sub-label under assigned domain")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  example.com - exact FQDN match")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  *.example.com - wildcard subdomain match")
                                .size(text_size(11))
                                .style(muted_text),
                        );
                } else {
                    pattern_tips = pattern_tips
                        .push(
                            text("  example.local - exact hostname match")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  *.example.com - wildcard subdomain match")
                                .size(text_size(11))
                                .style(muted_text),
                        )
                        .push(
                            text("  app-*, *-staging - glob / wildcard label patterns")
                                .size(text_size(11))
                                .style(muted_text),
                        );
                }
                pattern_tips = pattern_tips.push(
                    text("Do not include http:// or https:// - only the hostname.")
                        .size(text_size(11))
                        .style(muted_text),
                );

                let tips_panel =
                    container(pattern_tips)
                        .padding([8.0, 12.0])
                        .style(|theme: &Theme| {
                            let weak = theme.extended_palette().background.weak.color;
                            container::Style {
                                background: Some(
                                    Color::from_rgba(weak.r, weak.g, weak.b, 0.4).into(),
                                ),
                                border: Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });
                hostname_details = hostname_details.push(tips_panel);
            }

            let fields = column![
                hostname_label,
                hostname_input,
                hostname_details,
                backend_label,
                backend_picker,
                backend_help,
            ]
            .spacing(scaled(6.0));

            let form_auth_section = view_form_auth_section(
                &self.edit_frontend_form.form_auth_users,
                use_kde_buttons,
            );

            let options = column![
                text("Options").size(text_size(14)).style(muted_text),
                forward_toggle,
                forward_help,
                redirect_toggle,
                redirect_help,
                lets_encrypt_toggle,
                lets_encrypt_help,
                enable_cruma_toggle,
                enable_cruma_help,
                form_auth_section,
            ]
            .spacing(scaled(6.0));

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
