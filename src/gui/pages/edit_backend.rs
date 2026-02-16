use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input,
};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use super::super::{
    BackendKind, EditBackendField, KdeButtonRole, Message, OddBoxGui, ProcessLogLevelChoice,
    scaled, text_size,
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
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Remote)
            && self.edit_backend_form.endpoints.trim().is_empty()
        {
            errors.push(
                text("At least one endpoint is required.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Static)
            && self.edit_backend_form.dir.trim().is_empty()
        {
            errors.push(
                text("Directory is required.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Static)
            && !self.edit_backend_form.cache_max_age.trim().is_empty()
            && self
                .edit_backend_form
                .cache_max_age
                .trim()
                .parse::<u64>()
                .is_err()
        {
            errors.push(
                text("Cache max-age must be a number.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if matches!(self.edit_backend_form.kind, BackendKind::Process)
            && self.edit_backend_form.proc_bin.trim().is_empty()
        {
            errors.push(
                text("Binary is required.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if matches!(self.edit_backend_form.kind, BackendKind::Process)
            && !self.edit_backend_form.proc_port.trim().is_empty()
            && self
                .edit_backend_form
                .proc_port
                .trim()
                .parse::<u16>()
                .is_err()
        {
            errors.push(
                text("Port must be a number.")
                    .size(text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        let notice = self
            .edit_backend_notice
            .as_ref()
            .map(|msg| text(msg).size(text_size(13)).style(muted_text));

        let use_kde_buttons = self.use_kde_system_styles();

        let save_btn = button(text("Save").size(text_size(14)))
            .on_press(Message::EditBackendSave)
            .style(move |theme, status| {
                super::super::themed_button_style(
                    theme,
                    status,
                    KdeButtonRole::Primary,
                    use_kde_buttons,
                )
            });
        let back_btn = button(text("Back").size(text_size(14)))
            .on_press(Message::NavigateTo(super::super::Page::Backends))
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
        if !self.edit_backend_is_new {
            let delete_btn = button(text("Delete").size(text_size(14)))
                .on_press(Message::EditBackendDelete)
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
            let use_kde_buttons = self.use_kde_system_styles();
            let card_style = |theme: &Theme| container::Style {
                background: Some(self.surface_panel_bg(theme).into()),
                border: Border {
                    radius: scaled(6.0).into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
                },
                ..Default::default()
            };

            let type_label = text("Type").size(text_size(13)).style(muted_text);
            let type_picker = pick_list(
                BackendKind::ALL.as_slice(),
                Some(self.edit_backend_form.kind),
                |v| Message::EditBackendFieldChanged(EditBackendField::Kind(v)),
            )
            .padding(scaled(8.0))
            .width(Length::Fill);

            let id_label = text("Backend ID").size(text_size(13)).style(muted_text);
            let id_input = text_input("backend-id", &self.edit_backend_form.id)
                .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Id(v)))
                .padding(scaled(8.0))
                .width(Length::Fill);
            let header = column![type_label, type_picker, id_label, id_input,].spacing(scaled(6.0));

            let (fields_card, info_card) = match self.edit_backend_form.kind {
                BackendKind::Remote => {
                    let endpoints_label = text("Endpoints").size(text_size(13)).style(muted_text);
                    let endpoints_help = text("Comma-separated host:port list")
                        .size(text_size(12))
                        .style(muted_text);
                    let endpoints_input = text_input(
                        "example.com:80, 10.0.0.5:8080",
                        &self.edit_backend_form.endpoints,
                    )
                    .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Endpoints(v)))
                    .padding(scaled(8.0))
                    .width(Length::Fill);

                    let protocol_label = text("Protocol").size(text_size(13)).style(muted_text);
                    let protocol_help = text("Upstream protocol (h1, h2, h2c, h2cpk)")
                        .size(text_size(12))
                        .style(muted_text);
                    let protocol_picker = pick_list(
                        PROTOCOL_OPTIONS.as_slice(),
                        Some(self.edit_backend_form.protocol.clone()),
                        |v| Message::EditBackendFieldChanged(EditBackendField::Protocol(v)),
                    )
                    .padding(scaled(8.0))
                    .width(Length::Fill);

                    let https_toggle = checkbox(self.edit_backend_form.https)
                        .label("Upstream uses HTTPS")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::Https(v))
                        });
                    let https_help = text("Enable if endpoints are HTTPS")
                        .size(text_size(12))
                        .style(muted_text);

                    let keep_host_toggle =
                        checkbox(self.edit_backend_form.keep_original_host_header)
                            .label("Keep original Host header")
                            .on_toggle(|v| {
                                Message::EditBackendFieldChanged(
                                    EditBackendField::KeepOriginalHostHeader(v),
                                )
                            });
                    let keep_host_help = text("Forward the incoming Host header to upstream")
                        .size(text_size(12))
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
                    .spacing(scaled(6.0));

                    let info = column![
                        text("Remote Backend").size(text_size(14)).style(muted_text),
                        text("Routes to one or more upstream servers.")
                            .size(text_size(12))
                            .style(muted_text),
                        text("Use commas to list multiple endpoints.")
                            .size(text_size(12))
                            .style(muted_text),
                    ]
                    .spacing(scaled(6.0));

                    (
                        container(fields)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info)
                                .padding(scaled(12.0))
                                .style(card_style)
                                .width(Length::Fill),
                        ),
                    )
                }
                BackendKind::Static => {
                    let dir_label = text("Directory").size(text_size(13)).style(muted_text);
                    let dir_help = text("Folder path to serve files from")
                        .size(text_size(12))
                        .style(muted_text);
                    let dir_input = text_input("/var/www/site", &self.edit_backend_form.dir)
                        .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Dir(v)))
                        .padding(scaled(8.0))
                        .width(Length::Fill);
                    let dir_browse = button(text("Browse").size(text_size(12)))
                        .padding(scaled(8.0))
                        .on_press(Message::EditBackendPickDir)
                        .style(move |theme, status| {
                            super::super::themed_button_style(
                                theme,
                                status,
                                KdeButtonRole::Neutral,
                                use_kde_buttons,
                            )
                        });
                    let resolved_dir = self.edit_backend_resolved_dir.as_ref().map(|path| {
                        text(format!("Resolved: {}", path))
                            .size(text_size(12))
                            .style(muted_text)
                    });
                    let resolve_error = self.edit_backend_resolve_error.as_ref().map(|err| {
                        text(err)
                            .size(text_size(12))
                            .color(Color::from_rgb(0.9, 0.3, 0.3))
                    });
                    let has_vars = self.edit_backend_form.dir.contains("$root_dir")
                        || self.edit_backend_form.dir.contains("$cfg_dir")
                        || self.edit_backend_form.dir.contains('~');
                    let vars_help = column![
                        text("Available variables:")
                            .size(text_size(12))
                            .style(muted_text),
                        text("$root_dir  Project root directory")
                            .size(text_size(12))
                            .style(muted_text),
                        text("$cfg_dir   Directory of the config file")
                            .size(text_size(12))
                            .style(muted_text),
                        text("~          Home directory")
                            .size(text_size(12))
                            .style(muted_text),
                    ]
                    .spacing(scaled(2.0));

                    let list_toggle = checkbox(self.edit_backend_form.list_dir)
                        .label("Enable directory listing")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ListDir(v))
                        });

                    let render_toggle = checkbox(self.edit_backend_form.render_markdown)
                        .label("Render markdown")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::RenderMarkdown(v))
                        });

                    let spa_toggle = checkbox(self.edit_backend_form.spa_fallback)
                        .label("SPA fallback (serve index.html for missing paths)")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::SpaFallback(v))
                        });

                    let cache_label = text("Cache max-age (seconds)")
                        .size(text_size(13))
                        .style(muted_text);
                    let cache_input = text_input("3600", &self.edit_backend_form.cache_max_age)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::CacheMaxAge(v))
                        })
                        .padding(scaled(8.0))
                        .width(Length::Fill);
                    let cache_help = text("Sets Cache-Control: max-age=<seconds> on responses")
                        .size(text_size(12))
                        .style(muted_text);

                    let mut path_box = column![
                        header,
                        dir_label,
                        row![dir_input, dir_browse].spacing(scaled(8.0)),
                        dir_help,
                    ]
                    .spacing(scaled(6.0));
                    if let Some(err) = resolve_error {
                        path_box = path_box.push(err);
                    } else if let Some(resolved) = resolved_dir {
                        path_box = path_box.push(resolved);
                    } else if !has_vars {
                        path_box = path_box.push(vars_help);
                    }

                    let options_box = column![
                        text("Options").size(text_size(14)).style(muted_text),
                        list_toggle,
                        render_toggle,
                        spa_toggle,
                        cache_label,
                        cache_input,
                        cache_help,
                    ]
                    .spacing(scaled(6.0));

                    let fields = column![
                        container(path_box)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                        container(options_box)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                    ]
                    .spacing(scaled(10.0));

                    (
                        container(fields)
                            .width(Length::Fill)
                            .style(|_| container::Style::default()),
                        None,
                    )
                }
                BackendKind::Process => {
                    let bin_label = text("Binary").size(text_size(13)).style(muted_text);
                    let bin_input = text_input("my-app", &self.edit_backend_form.proc_bin)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcBin(v))
                        })
                        .padding(scaled(8.0))
                        .width(Length::Fill);
                    let bin_browse = button(text("Browse").size(text_size(12)))
                        .padding(scaled(8.0))
                        .on_press(Message::EditBackendPickBin)
                        .style(move |theme, status| {
                            super::super::themed_button_style(
                                theme,
                                status,
                                KdeButtonRole::Neutral,
                                use_kde_buttons,
                            )
                        });
                    let bin_help = text("Path or command to execute")
                        .size(text_size(12))
                        .style(muted_text);
                    let bin_trimmed = self.edit_backend_form.proc_bin.trim();
                    let bin_path = std::path::Path::new(bin_trimmed);
                    let bin_missing = !bin_trimmed.is_empty()
                        && (bin_path.is_absolute()
                            || bin_trimmed.contains(std::path::MAIN_SEPARATOR))
                        && !bin_path.exists();
                    let bin_missing_msg = if bin_missing {
                        Some(
                            text("Binary not found on disk.")
                                .size(text_size(12))
                                .color(Color::from_rgb(0.9, 0.3, 0.3)),
                        )
                    } else {
                        None
                    };

                    let args_label = text("Args").size(text_size(13)).style(muted_text);
                    let args_input = text_input("--flag value", &self.edit_backend_form.proc_args)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcArgs(v))
                        })
                        .padding(scaled(8.0))
                        .width(Length::Fill);
                    let args_help = text("Space-separated arguments")
                        .size(text_size(12))
                        .style(muted_text);

                    let dir_label = text("Working dir").size(text_size(13)).style(muted_text);
                    let dir_input = text_input("/path/to/app", &self.edit_backend_form.proc_dir)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcDir(v))
                        })
                        .padding(scaled(8.0))
                        .width(Length::Fill);
                    let dir_help = text("Optional working directory")
                        .size(text_size(12))
                        .style(muted_text);

                    let protocol_label = text("Protocol").size(text_size(13)).style(muted_text);
                    let protocol_picker = pick_list(
                        PROTOCOL_OPTIONS.as_slice(),
                        Some(self.edit_backend_form.protocol.clone()),
                        |v| Message::EditBackendFieldChanged(EditBackendField::Protocol(v)),
                    )
                    .padding(scaled(8.0))
                    .width(Length::Fill);

                    let https_toggle = checkbox(self.edit_backend_form.https)
                        .label("Upstream uses HTTPS")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::Https(v))
                        });

                    let port_label = text("Port (optional)")
                        .size(text_size(13))
                        .style(muted_text);
                    let port_input = text_input("8080", &self.edit_backend_form.proc_port)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcPort(v))
                        })
                        .padding(scaled(8.0))
                        .width(Length::Fill);

                    let env_label = text("Environment Variables")
                        .size(text_size(13))
                        .style(muted_text);

                    // Build env var rows
                    let mut env_rows: Vec<Element<'_, Message>> = Vec::new();
                    for (idx, (key, value)) in self.edit_backend_form.proc_env.iter().enumerate() {
                        let key_input = text_input("KEY", key)
                            .on_input(move |v| Message::EditBackendEnvKeyChanged(idx, v))
                            .padding(scaled(6.0))
                            .size(text_size(12))
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2));

                        let value_input = text_input("VALUE", value)
                            .on_input(move |v| Message::EditBackendEnvValueChanged(idx, v))
                            .padding(scaled(6.0))
                            .size(text_size(12))
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(3));

                        let remove_btn = button(text("✕").size(text_size(12)))
                            .padding(Padding {
                                top: scaled(4.0),
                                right: scaled(6.0),
                                bottom: scaled(4.0),
                                left: scaled(6.0),
                            })
                            .style(move |theme: &Theme, status| {
                                super::super::themed_button_style(
                                    theme,
                                    status,
                                    KdeButtonRole::Danger,
                                    use_kde_buttons,
                                )
                            })
                            .on_press(Message::EditBackendEnvRemove(idx));

                        env_rows.push(
                            row![key_input, value_input, remove_btn]
                                .spacing(scaled(4.0))
                                .align_y(iced::Alignment::Center)
                                .into(),
                        );
                    }

                    // Add new env var row
                    let new_key_input = text_input("New key...", &self.edit_backend_env_new_key)
                        .on_input(Message::EditBackendEnvNewKeyChanged)
                        .on_submit(Message::EditBackendEnvAdd)
                        .padding(scaled(6.0))
                        .size(text_size(12))
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2));

                    let new_value_input =
                        text_input("New value...", &self.edit_backend_env_new_value)
                            .on_input(Message::EditBackendEnvNewValueChanged)
                            .on_submit(Message::EditBackendEnvAdd)
                            .padding(scaled(6.0))
                            .size(text_size(12))
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(3));

                    let add_btn = button(text("+").size(text_size(12)))
                        .padding(Padding {
                            top: scaled(4.0),
                            right: scaled(8.0),
                            bottom: scaled(4.0),
                            left: scaled(8.0),
                        })
                        .style(move |theme: &Theme, status| {
                            super::super::themed_button_style(
                                theme,
                                status,
                                KdeButtonRole::Primary,
                                use_kde_buttons,
                            )
                        })
                        .on_press(Message::EditBackendEnvAdd);

                    let add_row = row![new_key_input, new_value_input, add_btn]
                        .spacing(scaled(4.0))
                        .align_y(iced::Alignment::Center);

                    let mut env_list = column![].spacing(scaled(4.0));
                    for env_row in env_rows {
                        env_list = env_list.push(env_row);
                    }
                    env_list = env_list.push(add_row);

                    let env_container = container(env_list)
                        .padding(scaled(8.0))
                        .width(Length::Fill)
                        .style(|theme: &Theme| container::Style {
                            background: Some(self.surface_panel_alt_bg(theme).into()),
                            border: Border {
                                radius: scaled(4.0).into(),
                                width: 1.0,
                                color: self.surface_border_color(theme),
                            },
                            ..Default::default()
                        });

                    let env_help = if self.edit_backend_form.proc_env.is_empty() {
                        text("No process-specific env vars. Add with + button.")
                            .size(text_size(12))
                            .style(muted_text)
                    } else {
                        text("Process-level vars override global vars with same key.")
                            .size(text_size(12))
                            .style(muted_text)
                    };

                    let log_level_label = text("Process log level")
                        .size(text_size(13))
                        .style(muted_text);
                    let log_level_picker = pick_list(
                        ProcessLogLevelChoice::ALL.as_slice(),
                        Some(self.edit_backend_form.proc_log_level),
                        |v| Message::EditBackendFieldChanged(EditBackendField::ProcLogLevel(v)),
                    )
                    .padding(scaled(8.0))
                    .width(Length::Fill);
                    let log_level_help = text("Overrides default process log level")
                        .size(text_size(12))
                        .style(muted_text);

                    let auto_start_toggle = checkbox(self.edit_backend_form.proc_auto_start)
                        .label("Auto-start process")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcAutoStart(v))
                        });

                    let exclude_toggle =
                        checkbox(self.edit_backend_form.proc_exclude_from_start_all)
                            .label("Exclude from start-all")
                            .on_toggle(|v| {
                                Message::EditBackendFieldChanged(
                                    EditBackendField::ProcExcludeFromStartAll(v),
                                )
                            });

                    let mut fields = column![
                        header,
                        bin_label,
                        row![bin_input, bin_browse].spacing(scaled(8.0)),
                        bin_help,
                        args_label,
                        args_input,
                        args_help,
                        dir_label,
                        dir_input,
                        dir_help,
                        protocol_label,
                        protocol_picker,
                        https_toggle,
                        port_label,
                        port_input,
                        env_label,
                        env_container,
                        env_help,
                        log_level_label,
                        log_level_picker,
                        log_level_help,
                        auto_start_toggle,
                        exclude_toggle,
                    ]
                    .spacing(scaled(6.0));
                    if let Some(msg) = bin_missing_msg {
                        fields = fields.push(msg);
                    }

                    let info = column![
                        text("Process Backend")
                            .size(text_size(14))
                            .style(muted_text),
                        text("Managed by odd-box. Changes take effect after reload.")
                            .size(text_size(12))
                            .style(muted_text),
                    ]
                    .spacing(scaled(6.0));

                    let global_env_map = self.state.config.load_full().env.clone();
                    let mut global_env_lines: Vec<String> = global_env_map
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect();
                    global_env_lines.sort();
                    if global_env_lines.is_empty() {
                        global_env_lines.push("No global env vars set.".to_string());
                    }
                    let mut global_env_list = column![];
                    for line in global_env_lines {
                        global_env_list =
                            global_env_list.push(text(line).size(text_size(12)).style(muted_text));
                    }
                    let global_env = column![
                        text("Global Env Vars")
                            .size(text_size(14))
                            .style(muted_text),
                        text("Applies to all process backends.")
                            .size(text_size(12))
                            .style(muted_text),
                        global_env_list,
                    ]
                    .spacing(scaled(6.0));

                    let info_stack = column![
                        container(info)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                        container(global_env)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                    ]
                    .spacing(scaled(10.0));
                    (
                        container(fields)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info_stack)
                                .style(|_| container::Style::default())
                                .width(Length::Fill),
                        ),
                    )
                }
                BackendKind::Unknown => {
                    let note = text("Backend not found.")
                        .size(text_size(12))
                        .style(muted_text);
                    let fields = column![header, note].spacing(scaled(6.0));
                    let info = column![
                        text("Unknown Backend")
                            .size(text_size(14))
                            .style(muted_text),
                        text("Select a backend from the Backends list.")
                            .size(text_size(12))
                            .style(muted_text),
                    ]
                    .spacing(scaled(6.0));
                    (
                        container(fields)
                            .padding(scaled(12.0))
                            .style(card_style)
                            .width(Length::Fill),
                        Some(
                            container(info)
                                .padding(scaled(12.0))
                                .style(card_style)
                                .width(Length::Fill),
                        ),
                    )
                }
            };

            if size.width < 760.0 {
                if let Some(info_card) = info_card {
                    column![fields_card, info_card]
                        .spacing(scaled(12.0))
                        .width(Length::Fill)
                        .into()
                } else {
                    column![fields_card]
                        .spacing(scaled(12.0))
                        .width(Length::Fill)
                        .into()
                }
            } else {
                if let Some(info_card) = info_card {
                    row![
                        fields_card.width(Length::FillPortion(3)),
                        info_card.width(Length::FillPortion(2))
                    ]
                    .spacing(scaled(12.0))
                    .width(Length::Fill)
                    .into()
                } else {
                    column![fields_card]
                        .spacing(scaled(12.0))
                        .width(Length::Fill)
                        .into()
                }
            }
        });

        let page_title = if self.edit_backend_is_new {
            format!("New {} Backend", self.edit_backend_form.kind)
        } else {
            format!("Edit {} Backend", self.edit_backend_form.kind)
        };

        let mut content =
            column![text(page_title).size(text_size(18)), layout, actions,].spacing(scaled(12.0));

        for err in errors {
            content = content.push(err);
        }

        if let Some(n) = notice {
            content = content.push(n);
        }

        container(content).width(Length::Fill).into()
    }
}
