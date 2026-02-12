use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, row, text};
use iced::{Border, Color, Element, Length, Padding, Theme};

use super::super::{Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_updates(&self) -> Element<'_, Message> {
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
                    format!("Current build {current_tag} is newer than latest reported release {latest}."),
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

        let status_box = container(
            text(status_text)
                .size(super::super::text_size(14))
                .color(status_color),
        )
        .padding(12)
        .width(Length::Fill)
        .style(|theme: &Theme| {
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
        });

        let mut check_button = button(text(if self.update_check_in_progress {
            "Checking..."
        } else {
            "Check for Updates"
        }))
        .padding(Padding {
            top: 8.0,
            right: 14.0,
            bottom: 8.0,
            left: 14.0,
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
            top: 8.0,
            right: 14.0,
            bottom: 8.0,
            left: 14.0,
        });
        if self_update_allowed {
            self_update_button = self_update_button.on_press(Message::UpdatesRunSelfUpdate);
        }

        let actions = row![check_button, self_update_button]
            .spacing(10)
            .width(Length::Fill);

        let mut details = column![
            row![
                text("Current Version")
                    .size(super::super::text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                text(current_tag.clone()).size(super::super::text_size(13)),
            ]
            .spacing(10),
            row![
                text("Install Source")
                    .size(super::super::text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                text(self.update_install_source.as_str()).size(super::super::text_size(13)),
            ]
            .spacing(10),
            row![
                text("Recommended Command")
                    .size(super::super::text_size(13))
                    .color(self.theme().extended_palette().background.weak.text),
                container(
                    text(self.update_hint.as_str())
                        .size(super::super::text_size(13))
                        .wrapping(Wrapping::WordOrGlyph),
                )
                .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Start)
            .width(Length::Fill),
        ]
        .spacing(10)
        .width(Length::Fill);

        if let Some(path) = &self.update_install_path {
            details = details.push(
                row![
                    text("Detected Binary")
                        .size(super::super::text_size(13))
                        .color(self.theme().extended_palette().background.weak.text),
                    container(
                        text(path.as_str())
                            .size(super::super::text_size(13))
                            .wrapping(Wrapping::WordOrGlyph),
                    )
                    .width(Length::Fill),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Start)
                .width(Length::Fill),
            );
        }

        if self.update_is_package_managed {
            details = details.push(
                text("Self-update is disabled for package-managed installs. Use the recommended command above.")
                    .size(super::super::text_size(12))
                    .color(self.theme().extended_palette().background.weak.text),
            );
        } else {
            details = details.push(
                text("Self-update is enabled for this install.")
                    .size(super::super::text_size(12))
                    .color(self.theme().extended_palette().background.weak.text),
            );
        }

        let details_box =
            container(details)
                .padding(16)
                .width(Length::Fill)
                .style(|theme: &Theme| {
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
                });

        let mut content = column![status_box, actions, details_box]
            .spacing(16)
            .width(Length::Fill);

        if let Some(notice) = &self.update_notice {
            let notice_color = if self.update_notice_is_error {
                self.theme().extended_palette().danger.strong.color
            } else {
                self.theme().extended_palette().success.strong.color
            };
            content = content.push(
                text(notice.as_str())
                    .size(super::super::text_size(13))
                    .color(notice_color),
            );
        }

        content.into()
    }
}
