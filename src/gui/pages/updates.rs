use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, text};
use iced::{Border, Color, Element, Length, Padding, Theme};

use super::super::{KdeButtonRole, Message, OddBoxGui, scaled, text_size};

impl OddBoxGui {
    pub(in crate::gui) fn view_updates(&self) -> Element<'_, Message> {
        let use_kde_buttons = self.use_kde_system_styles();
        let current_tag = format!("v{}", self.update_current_version);

        let (status_text, status_color) = if self.update_action_in_progress {
            (
                "Running self-update...".to_string(),
                self.theme().extended_palette().primary.strong.color,
            )
        } else if self.update_check_in_progress {
            (
                "Checking latest release...".to_string(),
                self.theme().extended_palette().primary.strong.color,
            )
        } else if let Some(err) = &self.update_check_error {
            (
                format!("Update check failed: {err}"),
                self.theme().extended_palette().danger.strong.color,
            )
        } else if let Some(latest) = &self.update_latest_tag {
            match super::super::compare_release_versions(&self.update_current_version, latest) {
                Some(std::cmp::Ordering::Greater) => (
                    format!("New release available: {latest} (current {current_tag})."),
                    Color::from_rgb(0.90, 0.63, 0.22),
                ),
                Some(std::cmp::Ordering::Equal) => (
                    format!("You are on the latest release ({latest})."),
                    self.theme().extended_palette().success.strong.color,
                ),
                Some(std::cmp::Ordering::Less) => (
                    format!(
                        "Current build {current_tag} is newer than latest reported release {latest}."
                    ),
                    self.theme().extended_palette().background.weak.text,
                ),
                None => (
                    format!("Latest release: {latest} (unable to compare versions)."),
                    self.theme().extended_palette().background.weak.text,
                ),
            }
        } else {
            (
                "Update status unavailable.".to_string(),
                self.theme().extended_palette().background.weak.text,
            )
        };

        let status_box = container(text(status_text).size(text_size(14)).color(status_color))
            .padding(scaled(12.0))
            .width(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(self.surface_panel_bg(theme).into()),
                border: Border {
                    radius: scaled(6.0).into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
                },
                ..Default::default()
            });

        let mut check_button = button(text(if self.update_check_in_progress {
            "Checking..."
        } else {
            "Check for Updates"
        }))
        .padding(Padding {
            top: scaled(8.0),
            right: scaled(14.0),
            bottom: scaled(8.0),
            left: scaled(14.0),
        })
        .style(move |theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Neutral,
                use_kde_buttons,
            )
        });

        if !self.update_check_in_progress && !self.update_action_in_progress {
            check_button = check_button.on_press(Message::UpdatesCheck);
        }

        let self_update_allowed = !self.update_is_package_managed
            && !self.update_check_in_progress
            && !self.update_action_in_progress;
        let mut self_update_button = button(text(if self.update_action_in_progress {
            "Updating..."
        } else {
            "Run Self-Update"
        }))
        .padding(Padding {
            top: scaled(8.0),
            right: scaled(14.0),
            bottom: scaled(8.0),
            left: scaled(14.0),
        })
        .style(move |theme, status| {
            super::super::themed_button_style(
                theme,
                status,
                KdeButtonRole::Primary,
                use_kde_buttons,
            )
        });
        if self_update_allowed {
            self_update_button = self_update_button.on_press(Message::UpdatesRunSelfUpdate);
        }

        let actions = row![check_button, self_update_button]
            .spacing(scaled(10.0))
            .width(Length::Fill);

        let mut details = column![
            row![
                text("Current Version")
                    .size(text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                text(current_tag.clone()).size(text_size(13)),
            ]
            .spacing(scaled(10.0)),
            row![
                text("Install Source")
                    .size(text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                text(self.update_install_source.as_str()).size(text_size(13)),
            ]
            .spacing(scaled(10.0)),
            row![
                text("Recommended Command")
                    .size(text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                container(
                    text(self.update_hint.as_str())
                        .size(text_size(13))
                        .wrapping(Wrapping::WordOrGlyph),
                )
                .width(Length::Fill),
            ]
            .spacing(scaled(10.0))
            .align_y(iced::Alignment::Start)
            .width(Length::Fill),
        ]
        .spacing(scaled(10.0))
        .width(Length::Fill);

        if let Some(path) = &self.update_install_path {
            details = details.push(
                row![
                    text("Detected Binary")
                        .size(text_size(13))
                        .color(self.theme().extended_palette().background.weak.text),
                    container(
                        text(path.as_str())
                            .size(text_size(13))
                            .wrapping(Wrapping::WordOrGlyph),
                    )
                    .width(Length::Fill),
                ]
                .spacing(scaled(10.0))
                .align_y(iced::Alignment::Start)
                .width(Length::Fill),
            );
        }

        if self.update_is_package_managed {
            details = details.push(
                text("Self-update is disabled for package-managed installs. Use the recommended command above.")
                    .size(text_size(12))
                    .color(self.theme().extended_palette().background.weak.text),
            );
        } else {
            details = details.push(
                text("Self-update is enabled for this install.")
                    .size(text_size(12))
                    .color(self.theme().extended_palette().background.weak.text),
            );
        }

        let details_box = container(details)
            .padding(scaled(16.0))
            .width(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(self.surface_panel_bg(theme).into()),
                border: Border {
                    radius: scaled(6.0).into(),
                    width: 1.0,
                    color: self.surface_border_color(theme),
                },
                ..Default::default()
            });

        let mut content = column![status_box, actions, details_box]
            .spacing(scaled(16.0))
            .width(Length::Fill);

        if let Some(notice) = &self.update_notice {
            let notice_color = if self.update_notice_is_error {
                self.theme().extended_palette().danger.strong.color
            } else {
                self.theme().extended_palette().success.strong.color
            };
            content = content.push(
                text(notice.as_str())
                    .size(text_size(13))
                    .color(notice_color),
            );
        }

        content.into()
    }
}
