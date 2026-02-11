use iced::widget::{
    button, checkbox, column, container, pick_list, responsive, row, text, text_input, Row, Space,
};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

use super::super::{BackendKind, EditBackendField, Message, OddBoxGui, ProcessLogLevelChoice};

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
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Remote)
            && self.edit_backend_form.endpoints.trim().is_empty()
        {
            errors.push(
                text("At least one endpoint is required.")
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        if matches!(self.edit_backend_form.kind, BackendKind::Static)
            && self.edit_backend_form.dir.trim().is_empty()
        {
            errors.push(
                text("Directory is required.")
                    .size(super::super::text_size(12))
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
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }
        if matches!(self.edit_backend_form.kind, BackendKind::Process)
            && self.edit_backend_form.proc_bin.trim().is_empty()
        {
            errors.push(
                text("Binary is required.")
                    .size(super::super::text_size(12))
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
                    .size(super::super::text_size(12))
                    .color(Color::from_rgb(0.9, 0.3, 0.3))
                    .into(),
            );
        }

        let notice = self
            .edit_backend_notice
            .as_ref()
            .map(|msg| text(msg).size(super::super::text_size(13)).style(muted_text));

        let mut actions_children: Vec<Element<'_, Message>> = vec![
            button(text("Save").size(super::super::text_size(14)))
                .on_press(Message::EditBackendSave)
                .into(),
            button(text("Back").size(super::super::text_size(14)))
                .on_press(Message::NavigateTo(super::super::Page::Backends))
                .into(),
        ];
        if !self.edit_backend_is_new {
            actions_children.push(
                button(text("Delete").size(super::super::text_size(14)))
                    .on_press(Message::EditBackendDelete)
                    .into(),
            );
        }
        let actions = row::Row::with_children(actions_children).spacing(10);

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

            let id_label = text("Backend ID").size(super::super::text_size(13)).style(muted_text);
            let id_input = text_input("backend-id", &self.edit_backend_form.id)
                .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Id(v)))
                .padding(8)
                .width(Length::Fill);
            let header = column![
                text(kind_label).size(super::super::text_size(14)).style(muted_text),
                id_label,
                id_input,
            ]
            .spacing(6);

            let (fields_card, info_card) = match self.edit_backend_form.kind {
                BackendKind::Remote => {
                    let endpoints_label = text("Endpoints").size(super::super::text_size(13)).style(muted_text);
                    let endpoints_help = text("Comma-separated host:port list")
                        .size(super::super::text_size(12))
                        .style(muted_text);
                    let endpoints_input = text_input(
                        "example.com:80, 10.0.0.5:8080",
                        &self.edit_backend_form.endpoints,
                    )
                    .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Endpoints(v)))
                    .padding(8)
                    .width(Length::Fill);

                    let protocol_label = text("Protocol").size(super::super::text_size(13)).style(muted_text);
                    let protocol_help = text("Upstream protocol (h1, h2, h2c, h2cpk)")
                        .size(super::super::text_size(12))
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
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::Https(v))
                        });
                    let https_help = text("Enable if endpoints are HTTPS")
                        .size(super::super::text_size(12))
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
                        .size(super::super::text_size(12))
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
                        text("Remote Backend").size(super::super::text_size(14)).style(muted_text),
                        text("Routes to one or more upstream servers.")
                            .size(super::super::text_size(12))
                            .style(muted_text),
                        text("Use commas to list multiple endpoints.")
                            .size(super::super::text_size(12))
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
                    let dir_label = text("Directory").size(super::super::text_size(13)).style(muted_text);
                    let dir_help = text("Folder path to serve files from")
                        .size(super::super::text_size(12))
                        .style(muted_text);
                    let dir_input = text_input("/var/www/site", &self.edit_backend_form.dir)
                        .on_input(|v| Message::EditBackendFieldChanged(EditBackendField::Dir(v)))
                        .padding(8)
                        .width(Length::Fill);
                    let dir_browse = button(text("Browse").size(super::super::text_size(12)))
                        .padding(8)
                        .on_press(Message::EditBackendPickDir);
                    let resolved_dir = self.edit_backend_resolved_dir.as_ref().map(|path| {
                        text(format!("Resolved: {}", path))
                            .size(super::super::text_size(12))
                            .style(muted_text)
                    });
                    let resolve_error = self
                        .edit_backend_resolve_error
                        .as_ref()
                        .map(|err| text(err).size(super::super::text_size(12)).color(Color::from_rgb(0.9, 0.3, 0.3)));
                    let has_vars = self.edit_backend_form.dir.contains("$root_dir")
                        || self.edit_backend_form.dir.contains("$cfg_dir")
                        || self.edit_backend_form.dir.contains('~');
                    let vars_help = column![
                        text("Available variables:").size(super::super::text_size(12)).style(muted_text),
                        text("$root_dir  Project root directory")
                            .size(super::super::text_size(12))
                            .style(muted_text),
                        text("$cfg_dir   Directory of the config file")
                            .size(super::super::text_size(12))
                            .style(muted_text),
                        text("~          Home directory").size(super::super::text_size(12)).style(muted_text),
                    ]
                    .spacing(2);

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

                    let cache_label = text("Cache max-age (seconds)").size(super::super::text_size(13)).style(muted_text);
                    let cache_input = text_input("3600", &self.edit_backend_form.cache_max_age)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::CacheMaxAge(v))
                        })
                        .padding(8)
                        .width(Length::Fill);
                    let cache_help = text("Sets Cache-Control: max-age=<seconds> on responses")
                        .size(super::super::text_size(12))
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
                        text("Options").size(super::super::text_size(14)).style(muted_text),
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
                    let bin_label = text("Binary").size(super::super::text_size(13)).style(muted_text);
                    let bin_input = text_input("my-app", &self.edit_backend_form.proc_bin)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcBin(v))
                        })
                        .padding(8)
                        .width(Length::Fill);
                    let bin_browse = button(text("Browse").size(super::super::text_size(12)))
                        .padding(8)
                        .on_press(Message::EditBackendPickBin);
                    let bin_help = text("Path or command to execute")
                        .size(super::super::text_size(12))
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
                                .size(super::super::text_size(12))
                                .color(Color::from_rgb(0.9, 0.3, 0.3)),
                        )
                    } else {
                        None
                    };

                    let args_label = text("Args").size(super::super::text_size(13)).style(muted_text);
                    let args_input = text_input("--flag value", &self.edit_backend_form.proc_args)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcArgs(v))
                        })
                        .padding(8)
                        .width(Length::Fill);
                    let args_help = text("Space-separated arguments").size(super::super::text_size(12)).style(muted_text);

                    let dir_label = text("Working dir").size(super::super::text_size(13)).style(muted_text);
                    let dir_input = text_input("/path/to/app", &self.edit_backend_form.proc_dir)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcDir(v))
                        })
                        .padding(8)
                        .width(Length::Fill);
                    let dir_help = text("Optional working directory")
                        .size(super::super::text_size(12))
                        .style(muted_text);

                    let protocol_label = text("Protocol").size(super::super::text_size(13)).style(muted_text);
                    let protocol_picker = pick_list(
                        PROTOCOL_OPTIONS.as_slice(),
                        Some(self.edit_backend_form.protocol.clone()),
                        |v| Message::EditBackendFieldChanged(EditBackendField::Protocol(v)),
                    )
                    .padding(8)
                    .width(Length::Fill);

                    let https_toggle = checkbox(self.edit_backend_form.https)
                        .label("Upstream uses HTTPS")
                        .on_toggle(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::Https(v))
                        });

                    let port_label = text("Port (optional)").size(super::super::text_size(13)).style(muted_text);
                    let port_input = text_input("8080", &self.edit_backend_form.proc_port)
                        .on_input(|v| {
                            Message::EditBackendFieldChanged(EditBackendField::ProcPort(v))
                        })
                        .padding(8)
                        .width(Length::Fill);

                    let env_label = text("Environment Variables").size(super::super::text_size(13)).style(muted_text);
                    
                    // Build env var rows
                    let mut env_rows: Vec<Element<'_, Message>> = Vec::new();
                    for (idx, (key, value)) in self.edit_backend_form.proc_env.iter().enumerate() {
                        let key_input = text_input("KEY", key)
                            .on_input(move |v| Message::EditBackendEnvKeyChanged(idx, v))
                            .padding(6)
                            .size(super::super::text_size(12))
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(2));
                        
                        let value_input = text_input("VALUE", value)
                            .on_input(move |v| Message::EditBackendEnvValueChanged(idx, v))
                            .padding(6)
                            .size(super::super::text_size(12))
                            .font(Font::MONOSPACE)
                            .width(Length::FillPortion(3));
                        
                        let remove_btn = button(text("✕").size(super::super::text_size(12)))
                            .padding(Padding {
                                top: 4.0,
                                right: 6.0,
                                bottom: 4.0,
                                left: 6.0,
                            })
                            .style(|theme: &Theme, status| {
                                let palette = theme.extended_palette();
                                let (bg, fg) = match status {
                                    button::Status::Hovered => {
                                        (palette.danger.strong.color, palette.danger.strong.text)
                                    }
                                    _ => (Color::TRANSPARENT, palette.danger.base.color),
                                };
                                button::Style {
                                    background: Some(bg.into()),
                                    text_color: fg,
                                    border: Border {
                                        radius: 4.0.into(),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }
                            })
                            .on_press(Message::EditBackendEnvRemove(idx));
                        
                        env_rows.push(
                            row![key_input, value_input, remove_btn]
                                .spacing(4)
                                .align_y(iced::Alignment::Center)
                                .into(),
                        );
                    }
                    
                    // Add new env var row
                    let new_key_input = text_input("New key...", &self.edit_backend_env_new_key)
                        .on_input(Message::EditBackendEnvNewKeyChanged)
                        .on_submit(Message::EditBackendEnvAdd)
                        .padding(6)
                        .size(super::super::text_size(12))
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(2));
                    
                    let new_value_input = text_input("New value...", &self.edit_backend_env_new_value)
                        .on_input(Message::EditBackendEnvNewValueChanged)
                        .on_submit(Message::EditBackendEnvAdd)
                        .padding(6)
                        .size(super::super::text_size(12))
                        .font(Font::MONOSPACE)
                        .width(Length::FillPortion(3));
                    
                    let add_btn = button(text("+").size(super::super::text_size(12)))
                        .padding(Padding {
                            top: 4.0,
                            right: 8.0,
                            bottom: 4.0,
                            left: 8.0,
                        })
                        .style(|theme: &Theme, status| {
                            let palette = theme.extended_palette();
                            let (bg, fg) = match status {
                                button::Status::Hovered => {
                                    (palette.primary.strong.color, palette.primary.strong.text)
                                }
                                _ => (palette.primary.weak.color, palette.primary.weak.text),
                            };
                            button::Style {
                                background: Some(bg.into()),
                                text_color: fg,
                                border: Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        })
                        .on_press(Message::EditBackendEnvAdd);
                    
                    let add_row = row![new_key_input, new_value_input, add_btn]
                        .spacing(4)
                        .align_y(iced::Alignment::Center);
                    
                    let mut env_list = column![].spacing(4);
                    for env_row in env_rows {
                        env_list = env_list.push(env_row);
                    }
                    env_list = env_list.push(add_row);
                    
                    let env_container = container(env_list)
                        .padding(8)
                        .width(Length::Fill)
                        .style(|theme: &Theme| {
                            let palette = theme.extended_palette();
                            container::Style {
                                background: Some(palette.background.weak.color.into()),
                                border: Border {
                                    radius: 4.0.into(),
                                    width: 1.0,
                                    color: palette.background.strong.color,
                                },
                                ..Default::default()
                            }
                        });
                    
                    let env_help = if self.edit_backend_form.proc_env.is_empty() {
                        text("No process-specific env vars. Add with + button.")
                            .size(super::super::text_size(12))
                            .style(muted_text)
                    } else {
                        text("Process-level vars override global vars with same key.")
                            .size(super::super::text_size(12))
                            .style(muted_text)
                    };

                    let log_level_label = text("Process log level").size(super::super::text_size(13)).style(muted_text);
                    let log_level_picker = pick_list(
                        ProcessLogLevelChoice::ALL.as_slice(),
                        Some(self.edit_backend_form.proc_log_level),
                        |v| Message::EditBackendFieldChanged(EditBackendField::ProcLogLevel(v)),
                    )
                    .padding(8)
                    .width(Length::Fill);
                    let log_level_help = text("Overrides default process log level")
                        .size(super::super::text_size(12))
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
                        row![bin_input, bin_browse].spacing(8),
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
                    .spacing(6);
                    if let Some(msg) = bin_missing_msg {
                        fields = fields.push(msg);
                    }

                    let info = column![
                        text("Process Backend").size(super::super::text_size(14)).style(muted_text),
                        text("Managed by odd-box. Changes take effect after reload.")
                            .size(super::super::text_size(12))
                            .style(muted_text),
                    ]
                    .spacing(6);

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
                            global_env_list.push(text(line).size(super::super::text_size(12)).style(muted_text));
                    }
                    let global_env = column![
                        text("Global Env Vars").size(super::super::text_size(14)).style(muted_text),
                        text("Applies to all process backends.")
                            .size(super::super::text_size(12))
                            .style(muted_text),
                        global_env_list,
                    ]
                    .spacing(6);

                    let info_stack = column![
                        container(info)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                        container(global_env)
                            .padding(12)
                            .style(card_style)
                            .width(Length::Fill),
                    ]
                    .spacing(10);
                    (
                        container(fields)
                            .padding(12)
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
                    let note = text("Backend not found.").size(super::super::text_size(12)).style(muted_text);
                    let fields = column![header, note].spacing(6);
                    let info = column![
                        text("Unknown Backend").size(super::super::text_size(14)).style(muted_text),
                        text("Select a backend from the Backends list.")
                            .size(super::super::text_size(12))
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
                    column![fields_card].spacing(12).width(Length::Fill).into()
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
                    column![fields_card].spacing(12).width(Length::Fill).into()
                }
            }
        });

        let mut content = column![
            text("Backend settings").size(super::super::text_size(13)).style(muted_text),
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
