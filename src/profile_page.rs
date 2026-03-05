//! GUI page for managing configuration profiles.
//!
//! Implements the [`CustomPage`] trait so it integrates into the cruma
//! sidebar as a first-class page.

use cruma::gui::{CustomPage, CustomPageContext, CustomPayload, Message};
use cruma::iced::widget::{
    Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use cruma::iced::{Alignment, Color, Element, Length, Padding, Theme};

use crate::profiles::{ProfileEntry, ProfilesConfig, save_profiles};

// ── Action IDs ────────────────────────────────────────────────────────────────
/// Switch to the profile given by name (`Text(name)`).
const ACTION_SWITCH: u32 = 0;
/// Set a profile as the default (`Text(name)`).
const ACTION_SET_DEFAULT: u32 = 1;
/// Delete a profile (`Text(name)`).
const ACTION_DELETE: u32 = 2;
/// Submit the "add profile" form (`Pair(name, path_str)`).
const ACTION_ADD: u32 = 3;
/// Toggle ask_on_startup (`Bool(new_value)`).
const ACTION_TOGGLE_ASK: u32 = 4;
/// New-profile name field changed (`Text(value)`).
const ACTION_NAME_INPUT: u32 = 5;
/// New-profile path field changed (`Text(value)`).
const ACTION_PATH_INPUT: u32 = 6;
/// Begin renaming a profile — opens inline editor (`Text(name)`).
const ACTION_RENAME_BEGIN: u32 = 7;
/// Rename input value changed (`Text(value)`).
const ACTION_RENAME_INPUT: u32 = 8;
/// Commit the rename (`Pair(old_name, new_name)`).
const ACTION_RENAME_SUBMIT: u32 = 9;
/// Cancel an in-progress rename (`None`).
const ACTION_RENAME_CANCEL: u32 = 10;
/// Open native file-picker to choose a config file path (`None`).
const ACTION_BROWSE: u32 = 11;

// ── Page struct ───────────────────────────────────────────────────────────────

pub struct ProfilePage {
    profiles: ProfilesConfig,
    /// Status / error message shown at the bottom of the page.
    status: String,
    /// Add-profile form: name field.
    add_name: String,
    /// Add-profile form: path field.
    add_path: String,
    /// Which profile (by name) is currently being renamed, if any.
    rename_target: Option<String>,
    /// The new name being typed in the inline rename editor.
    rename_value: String,
}

impl ProfilePage {
    pub fn new(profiles: ProfilesConfig) -> Self {
        Self {
            profiles,
            status: String::new(),
            add_name: String::new(),
            add_path: String::new(),
            rename_target: None,
            rename_value: String::new(),
        }
    }

    /// Reload profiles from disk (called at the start of each `view`).
    fn refresh(&mut self) {
        self.profiles = crate::profiles::load_profiles();
    }

    /// Return the active config path by reading the runtime state.
    fn active_config_path(ctx: &CustomPageContext) -> Option<std::path::PathBuf> {
        ctx.app_runtime.proxy_config.load().config_path.clone()
    }

    fn display_path(path: &std::path::Path) -> String {
        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| path.to_path_buf())
            }
        });
        let s = absolute.display().to_string();
        if let Some(home) = dirs::home_dir() {
            let home_str = home.display().to_string();
            if let Some(rest) = s.strip_prefix(&home_str) {
                return format!("~{rest}");
            }
        }
        s
    }

    fn has_supported_config_extension(path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "yaml" | "yml" | "toml"))
            .unwrap_or(false)
    }
}

impl CustomPage for ProfilePage {
    fn id(&self) -> &str {
        "profiles"
    }

    fn title(&self) -> &str {
        "Profiles"
    }

    fn icon_char(&self) -> char {
        // Use a folder/bookmark-like character.
        '\u{25A3}' // ▣
    }

    fn view(&self, ctx: CustomPageContext) -> Element<'_, Message> {
        let is_dark = matches!(ctx.theme, Theme::Dark);

        // Active config path for highlighting the active profile.
        let active_path = Self::active_config_path(&ctx);
        let active_canonical = active_path.as_deref().and_then(|p| {
            std::fs::canonicalize(p).ok()
        });

        let default_name = self.profiles.default_profile.as_deref();

        // ── Header ───────────────────────────────────────────────────
        let active_label = active_path
            .as_deref()
            .map(Self::display_path)
            .unwrap_or_else(|| "(none – select a profile below)".into());

        let header = column![
            text("Config Profiles")
                .size(18.0)
                .color(if is_dark { Color::WHITE } else { Color::BLACK }),
            text(format!("Active config: {active_label}"))
                .size(12.0)
                .color(if is_dark {
                    Color::from_rgb(0.6, 0.6, 0.6)
                } else {
                    Color::from_rgb(0.4, 0.4, 0.4)
                }),
        ]
        .spacing(4.0)
        .width(Length::Fill);

        // ── Profile list ─────────────────────────────────────────────
        let mut profile_rows: Vec<Element<Message>> = Vec::new();

        if self.profiles.profiles.is_empty() {
            profile_rows.push(
                text("No profiles yet. Add one below.")
                    .size(13.0)
                    .color(if is_dark {
                        Color::from_rgb(0.5, 0.5, 0.5)
                    } else {
                        Color::from_rgb(0.5, 0.5, 0.5)
                    })
                    .into(),
            );
        }

        for entry in &self.profiles.profiles {
            let entry_canonical = std::fs::canonicalize(&entry.path).ok();
            let is_active = match (&active_canonical, &entry_canonical) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            };
            let is_default = default_name == Some(entry.name.as_str());

            // Abbreviated path string.
            let path_str = Self::display_path(&entry.path);

            let status_indicator = if is_active {
                text("● ").color(Color::from_rgb(0.2, 0.8, 0.4))
            } else if is_default {
                text("★ ").color(Color::from_rgb(1.0, 0.8, 0.0))
            } else {
                text("  ")
            };

            let name_cloned = entry.name.clone();
            let is_renaming = self.rename_target.as_deref() == Some(&entry.name);

            // ── Name cell: either inline rename editor or static label ──
            let name_cell: Element<Message> = if is_renaming {
                let old_name = name_cloned.clone();
                let new_name = self.rename_value.clone();
                let can_submit = !new_name.trim().is_empty() && new_name.trim() != old_name;

                let input = text_input("New name", &self.rename_value)
                    .on_input(|v| Message::CustomPageAction {
                        page_id: "profiles".into(),
                        action_id: ACTION_RENAME_INPUT,
                        payload: CustomPayload::Text(v),
                    })
                    .padding(Padding::from([4.0, 6.0]))
                    .size(12.0)
                    .width(Length::Fixed(130.0));

                let ok_btn = {
                    let on = name_cloned.clone();
                    let nv = new_name.clone();
                    let btn = button(text("✓").size(12.0)).padding(Padding::from([4.0, 8.0]));
                    if can_submit {
                        btn.on_press(Message::CustomPageAction {
                            page_id: "profiles".into(),
                            action_id: ACTION_RENAME_SUBMIT,
                            payload: CustomPayload::Pair(on, nv),
                        })
                    } else {
                        btn
                    }
                };

                let cancel_btn = button(text("✕").size(12.0))
                    .padding(Padding::from([4.0, 8.0]))
                    .on_press(Message::CustomPageAction {
                        page_id: "profiles".into(),
                        action_id: ACTION_RENAME_CANCEL,
                        payload: CustomPayload::None,
                    });

                row![input, Space::new().width(4.0), ok_btn, Space::new().width(2.0), cancel_btn]
                    .align_y(Alignment::Center)
                    .into()
            } else {
                let name_label = text(entry.name.clone())
                    .size(13.0)
                    .color(if is_dark { Color::WHITE } else { Color::BLACK })
                    .width(Length::Fixed(130.0));

                let rename_btn = button(text("✎").size(12.0))
                    .padding(Padding::from([2.0, 6.0]))
                    .on_press(Message::CustomPageAction {
                        page_id: "profiles".into(),
                        action_id: ACTION_RENAME_BEGIN,
                        payload: CustomPayload::Text(name_cloned.clone()),
                    })
                    .style(|theme: &Theme, status| {
                        use cruma::iced::widget::button;
                        let palette = theme.extended_palette();
                        button::Style {
                            background: Some(cruma::iced::Background::Color(
                                if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                                    palette.background.strong.color
                                } else {
                                    Color::TRANSPARENT
                                },
                            )),
                            text_color: palette.background.base.text,
                            border: cruma::iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    });

                row![name_label, rename_btn]
                    .align_y(Alignment::Center)
                    .spacing(2.0)
                    .width(Length::Fixed(160.0))
                    .into()
            };

            let path_text = text(path_str)
                .size(11.0)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
                .width(Length::Fill);

            let switch_btn = button(text("Switch").size(12.0))
                .padding(Padding::from([4.0, 10.0]))
                .on_press(Message::CustomPageAction {
                    page_id: "profiles".into(),
                    action_id: ACTION_SWITCH,
                    payload: CustomPayload::Text(name_cloned.clone()),
                })
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let bg = if is_active {
                        palette.success.base.color
                    } else {
                        palette.primary.base.color
                    };
                    let bg_hover = if is_active {
                        palette.success.strong.color
                    } else {
                        palette.primary.strong.color
                    };
                    use cruma::iced::widget::button;
                    button::Style {
                        background: Some(cruma::iced::Background::Color(
                            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                                bg_hover
                            } else {
                                bg
                            },
                        )),
                        text_color: Color::WHITE,
                        border: cruma::iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                });

            let name_for_default = name_cloned.clone();
            let default_btn = button(text(if is_default { "★ Default" } else { "Set default" }).size(11.0))
                .padding(Padding::from([4.0, 8.0]))
                .on_press(Message::CustomPageAction {
                    page_id: "profiles".into(),
                    action_id: ACTION_SET_DEFAULT,
                    payload: CustomPayload::Text(name_for_default),
                })
                .style(move |theme: &Theme, status| {
                    use cruma::iced::widget::button;
                    let palette = theme.extended_palette();
                    let bg = if is_default {
                        Color::from_rgb(0.5, 0.4, 0.0)
                    } else {
                        palette.background.strong.color
                    };
                    let bg_hover = if is_default {
                        Color::from_rgb(0.6, 0.5, 0.0)
                    } else {
                        palette.background.weak.color
                    };
                    button::Style {
                        background: Some(cruma::iced::Background::Color(
                            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                                bg_hover
                            } else {
                                bg
                            },
                        )),
                        text_color: if is_dark { Color::WHITE } else { Color::BLACK },
                        border: cruma::iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                });

            let name_for_delete = name_cloned.clone();
            let delete_btn = button(text("✕").size(12.0))
                .padding(Padding::from([4.0, 8.0]))
                .on_press(Message::CustomPageAction {
                    page_id: "profiles".into(),
                    action_id: ACTION_DELETE,
                    payload: CustomPayload::Text(name_for_delete),
                })
                .style(|theme: &Theme, status| {
                    use cruma::iced::widget::button;
                    let palette = theme.extended_palette();
                    button::Style {
                        background: Some(cruma::iced::Background::Color(
                            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                                palette.danger.strong.color
                            } else {
                                palette.danger.base.color
                            },
                        )),
                        text_color: Color::WHITE,
                        border: cruma::iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                });

            let row_bg = if is_active {
                Color::from_rgba(0.2, 0.8, 0.4, 0.08)
            } else {
                Color::TRANSPARENT
            };

            let profile_row = container(
                row![
                    status_indicator,
                    name_cell,
                    path_text,
                    switch_btn,
                    Space::new().width(6.0),
                    default_btn,
                    Space::new().width(4.0),
                    delete_btn,
                ]
                .align_y(Alignment::Center)
                .spacing(4.0)
                .width(Length::Fill),
            )
            .padding(Padding::from([6.0, 8.0]))
            .style(move |_theme: &Theme| container::Style {
                background: Some(cruma::iced::Background::Color(row_bg)),
                border: cruma::iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fill);

            profile_rows.push(profile_row.into());
        }

        let profile_list = scrollable(
            column(profile_rows)
                .spacing(4.0)
                .width(Length::Fill)
                .padding(Padding::from([4.0, 0.0])),
        )
        .height(Length::Fixed(240.0))
        .width(Length::Fill);

        // ── Default picker & ask-on-startup ─────────────────────────
        let ask_checkbox = checkbox(self.profiles.ask_on_startup)
            .on_toggle(|v| Message::CustomPageAction {
                page_id: "profiles".into(),
                action_id: ACTION_TOGGLE_ASK,
                payload: CustomPayload::Bool(v),
            });
        let ask_row = row![
            ask_checkbox,
            Space::new().width(8.0),
            text("Ask which profile to use on startup").size(13.0),
        ]
        .align_y(Alignment::Center);

        let default_info = if let Some(def) = &self.profiles.default_profile {
            text(format!(
                "Default profile: {def}  (used when no --config flag is given)"
            ))
            .size(12.0)
            .color(if is_dark {
                Color::from_rgb(0.5, 0.5, 0.5)
            } else {
                Color::from_rgb(0.4, 0.4, 0.4)
            })
        } else {
            text("No default profile — auto-discovers config on startup.")
                .size(12.0)
                .color(if is_dark {
                    Color::from_rgb(0.5, 0.5, 0.5)
                } else {
                    Color::from_rgb(0.4, 0.4, 0.4)
                })
        };

        // ── Add profile form ─────────────────────────────────────────
        let add_name_input = text_input("Profile name (e.g. work)", &self.add_name)
            .on_input(|v| Message::CustomPageAction {
                page_id: "profiles".into(),
                action_id: ACTION_NAME_INPUT,
                payload: CustomPayload::Text(v),
            })
            .padding(Padding::from([6.0, 8.0]))
            .size(13.0)
            .width(Length::Fixed(180.0));

        let add_path_input = text_input("Path to config file", &self.add_path)
            .on_input(|v| Message::CustomPageAction {
                page_id: "profiles".into(),
                action_id: ACTION_PATH_INPUT,
                payload: CustomPayload::Text(v),
            })
            .padding(Padding::from([6.0, 8.0]))
            .size(13.0)
            .width(Length::Fill);

        let browse_btn = button(text("…").size(13.0))
            .padding(Padding::from([6.0, 10.0]))
            .on_press(Message::CustomPageAction {
                page_id: "profiles".into(),
                action_id: ACTION_BROWSE,
                payload: CustomPayload::None,
            })
            .style(|theme: &Theme, status| {
                use cruma::iced::widget::button;
                let palette = theme.extended_palette();
                button::Style {
                    background: Some(cruma::iced::Background::Color(
                        if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                            palette.background.strong.color
                        } else {
                            palette.background.weak.color
                        },
                    )),
                    text_color: palette.background.base.text,
                    border: cruma::iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });

        let can_add = !self.add_name.trim().is_empty() && !self.add_path.trim().is_empty();

        let add_name_val = self.add_name.clone();
        let add_path_val = self.add_path.clone();
        let add_btn = {
            let btn = button(text("+ Add").size(13.0))
                .padding(Padding::from([6.0, 14.0]))
                .style(|theme: &Theme, status| {
                    use cruma::iced::widget::button;
                    let palette = theme.extended_palette();
                    button::Style {
                        background: Some(cruma::iced::Background::Color(
                            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                                palette.primary.strong.color
                            } else {
                                palette.primary.base.color
                            },
                        )),
                        text_color: Color::WHITE,
                        border: cruma::iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                });
            if can_add {
                btn.on_press(Message::CustomPageAction {
                    page_id: "profiles".into(),
                    action_id: ACTION_ADD,
                    payload: CustomPayload::Pair(add_name_val, add_path_val),
                })
            } else {
                btn
            }
        };

        let add_form = column![
            text("Add profile").size(13.0).color(if is_dark {
                Color::from_rgb(0.7, 0.7, 0.7)
            } else {
                Color::from_rgb(0.3, 0.3, 0.3)
            }),
            row![add_name_input, Space::new().width(8.0), add_path_input, Space::new().width(4.0), browse_btn, Space::new().width(8.0), add_btn]
                .align_y(Alignment::Center)
                .width(Length::Fill),
        ]
        .spacing(6.0)
        .width(Length::Fill);

        // ── Status line ──────────────────────────────────────────────
        let status_color = if self.status.starts_with("Failed") || self.status.starts_with("Error")
        {
            Color::from_rgb(0.9, 0.3, 0.3)
        } else if self.status.is_empty() {
            Color::TRANSPARENT
        } else {
            Color::from_rgb(0.2, 0.8, 0.4)
        };

        let status_line = text(if self.status.is_empty() {
            " ".to_string()
        } else {
            self.status.clone()
        })
        .size(12.0)
        .color(status_color);

        // ── Assemble ─────────────────────────────────────────────────
        let divider_color = if is_dark {
            Color::from_rgb(0.25, 0.25, 0.25)
        } else {
            Color::from_rgb(0.85, 0.85, 0.85)
        };

        let content = column![
            header,
            Space::new().height(12.0),
            profile_list,
            Space::new().height(8.0),
            container(Space::new().height(1.0))
                .width(Length::Fill)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(cruma::iced::Background::Color(divider_color)),
                    ..Default::default()
                }),
            Space::new().height(8.0),
            default_info,
            ask_row,
            Space::new().height(16.0),
            add_form,
            Space::new().height(8.0),
            status_line,
        ]
        .spacing(0.0)
        .padding(Padding::from([16.0, 20.0]))
        .width(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn update(
        &mut self,
        action_id: u32,
        payload: CustomPayload,
        ctx: CustomPageContext,
    ) -> cruma::iced::Task<Message> {
        // Refresh from disk before every action.
        self.refresh();

        match action_id {
            ACTION_SWITCH => {
                let CustomPayload::Text(name) = payload else {
                    return cruma::iced::Task::none();
                };
                let Some(entry) = self
                    .profiles
                    .profiles
                    .iter()
                    .find(|e| e.name == name)
                    .cloned()
                else {
                    self.status = format!("Profile '{name}' not found");
                    return cruma::iced::Task::none();
                };

                match cruma::config::load_config_from_path(&entry.path) {
                    Ok(mut config) => {
                        config.config_path = Some(entry.path.clone());
                        let _ = ctx.config_update_tx.send(
                            cruma::utils::ConfigUpdateRequest::ReloadFromDisk { config },
                        );
                        self.status = format!("Switched to profile '{name}'");
                    }
                    Err(e) => {
                        self.status = format!("Failed to load '{}': {e}", entry.path.display());
                    }
                }
            }

            ACTION_SET_DEFAULT => {
                let CustomPayload::Text(name) = payload else {
                    return cruma::iced::Task::none();
                };
                // Toggle off if already default.
                if self.profiles.default_profile.as_deref() == Some(&name) {
                    self.profiles.default_profile = None;
                } else {
                    self.profiles.default_profile = Some(name.clone());
                }
                match save_profiles(&self.profiles) {
                    Ok(()) => {
                        self.status = if self.profiles.default_profile.is_some() {
                            format!("Default set to '{name}'")
                        } else {
                            format!("Default cleared")
                        };
                    }
                    Err(e) => self.status = format!("Failed to save profiles: {e}"),
                }
            }

            ACTION_DELETE => {
                let CustomPayload::Text(name) = payload else {
                    return cruma::iced::Task::none();
                };
                self.profiles.profiles.retain(|e| e.name != name);
                // Clear default if we just removed it.
                if self.profiles.default_profile.as_deref() == Some(&name) {
                    self.profiles.default_profile = None;
                }
                match save_profiles(&self.profiles) {
                    Ok(()) => self.status = format!("Deleted profile '{name}'"),
                    Err(e) => self.status = format!("Failed to save profiles: {e}"),
                }
            }

            ACTION_ADD => {
                let CustomPayload::Pair(name, path_str) = payload else {
                    return cruma::iced::Task::none();
                };
                let name = name.trim().to_string();
                let mut path = std::path::PathBuf::from(path_str.trim());
                let mut migrated_from_legacy = false;

                if name.is_empty() {
                    self.status = "Profile name cannot be empty".into();
                    return cruma::iced::Task::none();
                }
                if !path.exists() {
                    self.status = format!("File not found: {}", path.display());
                    return cruma::iced::Task::none();
                }
                if !path.is_file() {
                    self.status = format!("Not a file: {}", path.display());
                    return cruma::iced::Task::none();
                }
                if !Self::has_supported_config_extension(&path) {
                    self.status = "Config file must use .yaml, .yml, or .toml".into();
                    return cruma::iced::Task::none();
                }

                match cruma::config::load_config_from_path(&path) {
                    Ok(_) => {}
                    Err(_load_err)
                        if crate::migrate::looks_like_legacy_toml(&path)
                            && path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.eq_ignore_ascii_case("toml"))
                                .unwrap_or(false) =>
                    {
                        match crate::migrate::auto_migrate(&path.to_string_lossy()) {
                            Ok((_cfg, new_path)) => {
                                path = new_path;
                                migrated_from_legacy = true;
                            }
                            Err(e) => {
                                self.status =
                                    format!("Failed to auto-migrate '{}': {e}", path.display());
                                return cruma::iced::Task::none();
                            }
                        }
                    }
                    Err(load_err) => {
                        self.status = format!(
                            "Invalid config '{}': {load_err}",
                            path.display()
                        );
                        return cruma::iced::Task::none();
                    }
                }

                if self.profiles.profiles.iter().any(|e| e.name == name) {
                    self.status = format!("Profile '{name}' already exists");
                    return cruma::iced::Task::none();
                }
                let stored_path = path.clone();
                self.profiles.profiles.push(ProfileEntry {
                    name: name.clone(),
                    path: stored_path,
                });
                match save_profiles(&self.profiles) {
                    Ok(()) => {
                        self.status = if migrated_from_legacy {
                            format!(
                                "Added profile '{name}' (auto-migrated to '{}')",
                                path.display()
                            )
                        } else {
                            format!("Added profile '{name}'")
                        };
                        self.add_name.clear();
                        self.add_path.clear();
                    }
                    Err(e) => {
                        self.profiles.profiles.pop(); // roll back
                        self.status = format!("Failed to save profiles: {e}");
                    }
                }
            }

            ACTION_TOGGLE_ASK => {
                let CustomPayload::Bool(v) = payload else {
                    return cruma::iced::Task::none();
                };
                self.profiles.ask_on_startup = v;
                match save_profiles(&self.profiles) {
                    Ok(()) => {
                        self.status = if v {
                            "Ask on startup enabled (takes effect next launch)".into()
                        } else {
                            "Ask on startup disabled".into()
                        };
                    }
                    Err(e) => self.status = format!("Failed to save profiles: {e}"),
                }
            }

            ACTION_NAME_INPUT => {
                if let CustomPayload::Text(v) = payload {
                    self.add_name = v;
                }
            }

            ACTION_PATH_INPUT => {
                if let CustomPayload::Text(v) = payload {
                    self.add_path = v;
                }
            }

            ACTION_RENAME_BEGIN => {
                if let CustomPayload::Text(name) = payload {
                    self.rename_value = name.clone();
                    self.rename_target = Some(name);
                    self.status.clear();
                }
            }

            ACTION_RENAME_INPUT => {
                if let CustomPayload::Text(v) = payload {
                    self.rename_value = v;
                }
            }

            ACTION_RENAME_SUBMIT => {
                let CustomPayload::Pair(old_name, new_name) = payload else {
                    return cruma::iced::Task::none();
                };
                let new_name = new_name.trim().to_string();

                if new_name.is_empty() {
                    self.status = "Name cannot be empty".into();
                    return cruma::iced::Task::none();
                }
                if new_name == old_name {
                    self.rename_target = None;
                    return cruma::iced::Task::none();
                }
                if self.profiles.profiles.iter().any(|e| e.name == new_name) {
                    self.status = format!("A profile named '{new_name}' already exists");
                    return cruma::iced::Task::none();
                }

                for entry in &mut self.profiles.profiles {
                    if entry.name == old_name {
                        entry.name = new_name.clone();
                        break;
                    }
                }
                // Keep default pointer consistent.
                if self.profiles.default_profile.as_deref() == Some(&old_name) {
                    self.profiles.default_profile = Some(new_name.clone());
                }

                match save_profiles(&self.profiles) {
                    Ok(()) => {
                        self.status = format!("Renamed '{old_name}' → '{new_name}'");
                        self.rename_target = None;
                        self.rename_value.clear();
                    }
                    Err(e) => self.status = format!("Failed to save profiles: {e}"),
                }
            }

            ACTION_RENAME_CANCEL => {
                self.rename_target = None;
                self.rename_value.clear();
            }

            ACTION_BROWSE => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select odd-box config file")
                    .add_filter("Supported config files", &["yaml", "yml", "toml"])
                    .pick_file()
                {
                    self.add_path = path.display().to_string();
                    // Auto-fill name from path if still empty.
                    if self.add_name.trim().is_empty() {
                        self.add_name = crate::profiles::derive_profile_name(
                            &path,
                            &self.profiles.profiles,
                        );
                    }
                }
            }

            _ => {}
        }

        cruma::iced::Task::none()
    }
}
