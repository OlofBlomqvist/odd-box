//! Custom GUI page for odd-box self-update functionality.
//!
//! This page displays the current version, allows checking for updates,
//! and triggers the self-update mechanism from within the GUI.

use cruma::gui::{CustomPage, CustomPageContext, CustomPayload, Message};
use cruma::iced::widget::{button, column, container, row, text};
use cruma::iced::{Element, Length, Task};

use crate::self_update;

const PAGE_ID: &str = "odd-box-updates";

/// Action IDs for this page's UI interactions.
const ACTION_CHECK: u32 = 1;
const ACTION_RUN_UPDATE: u32 = 2;

/// State for the Updates custom page.
pub struct UpdatesPage {
    current_version: String,
    install_source: self_update::InstallSourceInfo,
    latest_version: Option<String>,
    check_in_progress: bool,
    update_in_progress: bool,
    notice: Option<Notice>,
}

#[derive(Clone)]
enum Notice {
    Info(String),
    Success(String),
    Error(String),
}

impl UpdatesPage {
    pub fn new() -> Self {
        let install_source = self_update::install_source_info();
        Self {
            current_version: self_update::current_version().to_string(),
            install_source,
            latest_version: None,
            check_in_progress: false,
            update_in_progress: false,
            notice: None,
        }
    }
}

impl CustomPage for UpdatesPage {
    fn id(&self) -> &str {
        PAGE_ID
    }

    fn title(&self) -> &str {
        "Updates"
    }

    fn icon_char(&self) -> char {
        '↑'
    }

    fn view(&self, _ctx: CustomPageContext) -> Element<'_, Message> {
        let mut content = column![].spacing(16).width(Length::Fill);

        // ── Header ─────────────────────────────────────────────────────
        content = content.push(text("Self-Update").size(22));

        // ── Current version ────────────────────────────────────────────
        content = content.push(
            row![
                text("Current version:").width(Length::Fixed(180.0)),
                text(format!("v{}", self.current_version)),
            ]
            .spacing(8),
        );

        // ── Install source ─────────────────────────────────────────────
        content = content.push(
            row![
                text("Install method:").width(Length::Fixed(180.0)),
                text(self.install_source.source),
            ]
            .spacing(8),
        );

        if let Some(ref path) = self.install_source.resolved_path {
            content = content.push(
                row![
                    text("Binary path:").width(Length::Fixed(180.0)),
                    text(path).size(13),
                ]
                .spacing(8),
            );
        }

        // ── Latest version / check result ──────────────────────────────
        if let Some(ref latest) = self.latest_version {
            let is_newer = self_update::current_version() != latest.trim_start_matches('v');

            content = content.push(
                row![
                    text("Latest version:").width(Length::Fixed(180.0)),
                    text(format!("{latest}")),
                    if is_newer {
                        text("  (update available)").style(|_theme| {
                            cruma::iced::widget::text::Style {
                                color: Some(cruma::iced::Color::from_rgb(0.2, 0.7, 0.3)),
                            }
                        })
                    } else {
                        text("  (up to date)").style(|_theme| cruma::iced::widget::text::Style {
                            color: Some(cruma::iced::Color::from_rgb(0.5, 0.5, 0.5)),
                        })
                    },
                ]
                .spacing(8),
            );
        }

        // ── Action buttons ─────────────────────────────────────────────
        let check_label = if self.check_in_progress {
            "Checking…"
        } else {
            "Check for updates"
        };

        let mut check_btn = button(text(check_label));
        if !self.check_in_progress && !self.update_in_progress {
            check_btn = check_btn.on_press(Message::CustomPageAction {
                page_id: PAGE_ID.into(),
                action_id: ACTION_CHECK,
                payload: CustomPayload::None,
            });
        }

        let mut buttons = row![check_btn].spacing(12);

        if self.install_source.package_managed {
            // Show hint instead of update button for package-managed installs
            content = content.push(
                container(
                    text(format!(
                        "This installation is managed by {}. To update, run:\n  {}",
                        self.install_source.source, self.install_source.update_hint
                    ))
                    .size(14),
                )
                .padding(12)
                .width(Length::Fill),
            );
        } else {
            // Show update button for manual installs
            let show_update_btn = self
                .latest_version
                .as_ref()
                .map(|v| self_update::current_version() != v.trim_start_matches('v'))
                .unwrap_or(false);

            if show_update_btn && !self.update_in_progress {
                let update_btn =
                    button(text("Install update")).on_press(Message::CustomPageAction {
                        page_id: PAGE_ID.into(),
                        action_id: ACTION_RUN_UPDATE,
                        payload: CustomPayload::None,
                    });
                buttons = buttons.push(update_btn);
            }

            if self.update_in_progress {
                buttons = buttons.push(text("Updating…"));
            }
        }

        content = content.push(buttons);

        // ── Notice / status ────────────────────────────────────────────
        if let Some(ref notice) = self.notice {
            let (msg, color) = match notice {
                Notice::Info(s) => (s.as_str(), cruma::iced::Color::from_rgb(0.4, 0.6, 0.9)),
                Notice::Success(s) => (s.as_str(), cruma::iced::Color::from_rgb(0.2, 0.7, 0.3)),
                Notice::Error(s) => (s.as_str(), cruma::iced::Color::from_rgb(0.9, 0.3, 0.3)),
            };

            content = content.push(
                container(
                    text(msg).style(move |_theme| cruma::iced::widget::text::Style {
                        color: Some(color),
                    }),
                )
                .padding(8),
            );
        }

        content.into()
    }

    fn update(
        &mut self,
        action_id: u32,
        _payload: CustomPayload,
        _ctx: CustomPageContext,
    ) -> Task<Message> {
        match action_id {
            ACTION_CHECK => {
                if self.check_in_progress {
                    return Task::none();
                }
                self.check_in_progress = true;
                self.notice = Some(Notice::Info("Checking for updates…".into()));

                let include_pre = self.current_version.contains('-');

                Task::perform(
                    async move {
                        self_update::find_latest_version(include_pre)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    move |result| match result {
                        Ok(version) => Message::CustomPageAction {
                            page_id: PAGE_ID.into(),
                            action_id: 100, // internal: check result OK
                            payload: CustomPayload::Text(version),
                        },
                        Err(err) => Message::CustomPageAction {
                            page_id: PAGE_ID.into(),
                            action_id: 101, // internal: check result error
                            payload: CustomPayload::Text(err),
                        },
                    },
                )
            }
            100 => {
                // Internal: check succeeded
                self.check_in_progress = false;
                if let CustomPayload::Text(ref version) = _payload {
                    self.latest_version = Some(version.clone());
                    let current = self_update::current_version();
                    let latest_normalized = version.trim_start_matches('v');
                    if current == latest_normalized {
                        self.notice = Some(Notice::Success(
                            "You are running the latest version.".into(),
                        ));
                    } else {
                        self.notice = Some(Notice::Info(format!(
                            "A new version is available: {version}"
                        )));
                    }
                }
                Task::none()
            }
            101 => {
                // Internal: check failed
                self.check_in_progress = false;
                if let CustomPayload::Text(ref err) = _payload {
                    self.notice = Some(Notice::Error(format!("Failed to check: {err}")));
                }
                Task::none()
            }
            ACTION_RUN_UPDATE => {
                if self.update_in_progress || self.install_source.package_managed {
                    return Task::none();
                }
                self.update_in_progress = true;
                self.notice = Some(Notice::Info("Downloading and installing update…".into()));

                Task::perform(
                    async move { self_update::update().await.map_err(|e| e.to_string()) },
                    move |result| match result {
                        Ok(self_update::UpdateAction::Updated) => Message::CustomPageAction {
                            page_id: PAGE_ID.into(),
                            action_id: 200, // internal: update OK
                            payload: CustomPayload::None,
                        },
                        Ok(self_update::UpdateAction::NoUpdateNeeded) => {
                            Message::CustomPageAction {
                                page_id: PAGE_ID.into(),
                                action_id: 201, // internal: no update needed
                                payload: CustomPayload::None,
                            }
                        }
                        Err(err) => Message::CustomPageAction {
                            page_id: PAGE_ID.into(),
                            action_id: 202, // internal: update error
                            payload: CustomPayload::Text(err),
                        },
                    },
                )
            }
            200 => {
                // Internal: update succeeded
                self.update_in_progress = false;
                self.notice = Some(Notice::Success(
                    "Update installed successfully! Please restart odd-box.".into(),
                ));
                Task::none()
            }
            201 => {
                // Internal: no update needed
                self.update_in_progress = false;
                self.notice = Some(Notice::Success(
                    "Already running the latest version.".into(),
                ));
                Task::none()
            }
            202 => {
                // Internal: update failed
                self.update_in_progress = false;
                if let CustomPayload::Text(ref err) = _payload {
                    self.notice = Some(Notice::Error(format!("Update failed: {err}")));
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}
