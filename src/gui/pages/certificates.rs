use iced::widget::{column, container, row, text};
use iced::{Border, Color, Element, Font, Length, Theme};

use crate::gui::components::table::{self, Column, Table};

use super::super::{Message, OddBoxGui, scaled, text_size};

impl OddBoxGui {
    pub(in crate::gui) fn view_certificates(&self) -> Element<'_, Message> {
        let guard = self.state.cruma_cert_status.load();
        let snapshot_opt = guard.as_ref().as_ref();

        match snapshot_opt {
            None => {
                // CertManager hasn't produced a snapshot yet — either the
                // cruma tunnel hasn't started or the first check cycle
                // hasn't completed.
                let notice = container(
                    text("Waiting for certificate status… (cruma tunnel may not be active yet)")
                        .font(Font::MONOSPACE)
                        .size(text_size(13))
                        .color(self.theme().extended_palette().background.weak.text),
                )
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

                column![notice]
                    .spacing(scaled(16.0))
                    .width(Length::Fill)
                    .into()
            }
            Some(snapshot) => {
                let summary = self.view_cert_summary(snapshot);
                let acme_table = self.view_acme_certs_table(snapshot);
                let selfsigned_table = self.view_selfsigned_certs_table(snapshot);
                let events_table = self.view_acme_events_table(snapshot);

                column![summary, acme_table, selfsigned_table, events_table]
                    .spacing(scaled(16.0))
                    .width(Length::Fill)
                    .into()
            }
        }
    }

    // ── Summary panel ───────────────────────────────────────────────────

    fn view_cert_summary(
        &self,
        snapshot: &cruma_tunnels_lib::CertStatusSnapshot,
    ) -> Element<'_, Message> {
        let theme = self.theme();
        let palette = theme.extended_palette();

        let total_acme = snapshot.acme_certs.len();
        let total_selfsigned = snapshot.selfsigned_certs.len();
        let total = total_acme + total_selfsigned;

        let valid_acme = snapshot.acme_certs.iter().filter(|(_, i)| i.is_valid).count();
        let valid_selfsigned = snapshot.selfsigned_certs.iter().filter(|(_, i)| i.is_valid).count();
        let expired = (total_acme - valid_acme) + (total_selfsigned - valid_selfsigned);
        let expiring_soon = snapshot.any_acme_expiring_soon();

        let (status_text, status_color) = if total == 0 {
            (
                "No local certificates — TLS is terminated by cruma.io on your behalf",
                palette.background.weak.text,
            )
        } else if expired > 0 {
            (
                "Attention needed — expired certificates detected",
                Color::from_rgb(0.85, 0.25, 0.25),
            )
        } else if expiring_soon {
            (
                "Certificates expiring soon — renewal recommended",
                Color::from_rgb(0.90, 0.63, 0.22),
            )
        } else {
            (
                "All certificates valid",
                palette.success.strong.color,
            )
        };

        let status_icon = if total == 0 {
            "ℹ"
        } else if expired > 0 {
            "⚠"
        } else if expiring_soon {
            "⏳"
        } else {
            "✓"
        };

        let has_valid_acme = snapshot.acme_certs.iter().any(|(_, i)| i.is_valid);
        let tls_mode = if has_valid_acme {
            "TLS terminated locally (valid ACME certificate present)"
        } else {
            "TLS terminated by cruma.io (no valid local ACME certificate)"
        };

        let checked_at = snapshot
            .checked_at
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string();

        let weak_text = palette.background.weak.text;

        let status_row = row![
            text(status_icon).size(text_size(16)),
            text(status_text)
                .font(Font::MONOSPACE)
                .size(text_size(13))
                .color(status_color),
        ]
        .spacing(scaled(8.0))
        .align_y(iced::Alignment::Center);

        let stats_row = row![
            stat_badge("ACME", total_acme, valid_acme),
            stat_badge("Self-Signed", total_selfsigned, valid_selfsigned),
            stat_badge("Events", snapshot.acme_events.len(), snapshot.acme_events.len()),
        ]
        .spacing(scaled(12.0));

        let detail_col = column![
            row![
                text("Last checked:").font(Font::MONOSPACE).size(text_size(12)).color(weak_text),
                text(checked_at).font(Font::MONOSPACE).size(text_size(12)).color(weak_text),
            ].spacing(scaled(8.0)),
            row![
                text("TLS mode:").font(Font::MONOSPACE).size(text_size(12)).color(weak_text),
                text(tls_mode).font(Font::MONOSPACE).size(text_size(12)).color(weak_text),
            ].spacing(scaled(8.0)),
        ]
        .spacing(scaled(4.0));

        container(
            column![status_row, stats_row, detail_col]
                .spacing(scaled(10.0))
                .width(Length::Fill),
        )
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
        })
        .into()
    }

    // ── ACME certificates table ─────────────────────────────────────────

    fn view_acme_certs_table(
        &self,
        snapshot: &cruma_tunnels_lib::CertStatusSnapshot,
    ) -> Element<'_, Message> {
        let header = text("ACME Certificates")
            .font(Font::MONOSPACE)
            .size(text_size(15));

        if snapshot.acme_certs.is_empty() {
            let empty = text("No ACME certificates cached locally.")
                .font(Font::MONOSPACE)
                .size(text_size(12))
                .color(self.theme().extended_palette().background.weak.text);

            return container(column![header, empty].spacing(scaled(8.0)))
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
                })
                .into();
        }

        let columns = vec![
            Column::fixed("Status", scaled(60.0)),
            Column::portion("Domain", 2),
            Column::portion("Issuer", 2),
            Column::fixed("Not Before", scaled(135.0)),
            Column::fixed("Not After", scaled(135.0)),
            Column::fixed("Remaining", scaled(110.0)),
            Column::portion("SANs", 2),
        ];

        let mut tbl = Table::new(columns);
        for (domain, info) in &snapshot.acme_certs {
            let remaining = format_duration_approx(info.seconds_until_expiry);
            let sans_display = if info.sans.len() > 1 {
                info.sans.join(", ")
            } else {
                "—".to_string()
            };

            tbl = tbl.push_row(vec![
                cert_status_cell(info),
                table::text_cell(domain),
                table::text_cell(&info.issuer),
                table::text_cell(info.not_before.format("%Y-%m-%d %H:%M").to_string()),
                table::text_cell(info.not_after.format("%Y-%m-%d %H:%M").to_string()),
                remaining_cell(&remaining, info.seconds_until_expiry, info.is_valid),
                table::text_cell(sans_display),
            ]);
        }

        column![header, tbl.build()]
            .spacing(scaled(8.0))
            .width(Length::Fill)
            .into()
    }

    // ── Self-signed certificates table ──────────────────────────────────

    fn view_selfsigned_certs_table(
        &self,
        snapshot: &cruma_tunnels_lib::CertStatusSnapshot,
    ) -> Element<'_, Message> {
        let header = text("Self-Signed Certificates")
            .font(Font::MONOSPACE)
            .size(text_size(15));

        if snapshot.selfsigned_certs.is_empty() {
            let empty = text("No self-signed certificates cached locally.")
                .font(Font::MONOSPACE)
                .size(text_size(12))
                .color(self.theme().extended_palette().background.weak.text);

            return container(column![header, empty].spacing(scaled(8.0)))
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
                })
                .into();
        }

        let columns = vec![
            Column::fixed("Status", scaled(60.0)),
            Column::portion("Domain", 2),
            Column::fixed("Not Before", scaled(135.0)),
            Column::fixed("Not After", scaled(135.0)),
            Column::fixed("Remaining", scaled(110.0)),
        ];

        let mut tbl = Table::new(columns);
        for (domain, info) in &snapshot.selfsigned_certs {
            let remaining = format_duration_approx(info.seconds_until_expiry);

            tbl = tbl.push_row(vec![
                cert_status_cell(info),
                table::text_cell(domain),
                table::text_cell(info.not_before.format("%Y-%m-%d %H:%M").to_string()),
                table::text_cell(info.not_after.format("%Y-%m-%d %H:%M").to_string()),
                remaining_cell(&remaining, info.seconds_until_expiry, info.is_valid),
            ]);
        }

        column![header, tbl.build()]
            .spacing(scaled(8.0))
            .width(Length::Fill)
            .into()
    }

    // ── ACME events table ───────────────────────────────────────────────

    fn view_acme_events_table(
        &self,
        snapshot: &cruma_tunnels_lib::CertStatusSnapshot,
    ) -> Element<'_, Message> {
        let header = text("ACME Challenge Activity")
            .font(Font::MONOSPACE)
            .size(text_size(15));

        if snapshot.acme_events.is_empty() {
            let empty = text("No ACME challenge activity recorded yet.")
                .font(Font::MONOSPACE)
                .size(text_size(12))
                .color(self.theme().extended_palette().background.weak.text);

            return container(column![header, empty].spacing(scaled(8.0)))
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
                })
                .into();
        }

        let columns = vec![
            Column::fixed("", scaled(30.0)),
            Column::fixed("Time", scaled(80.0)),
            Column::portion("Domain", 2),
            Column::portion("Cert Key", 2),
            Column::portion("Event", 3),
        ];

        let mut tbl = Table::new(columns);

        // Show the most recent events (up to 50)
        let events = &snapshot.acme_events;
        let start = events.len().saturating_sub(50);

        for event in &events[start..] {
            let (icon, description) = format_acme_event_kind(&event.kind);
            let time_str = event.timestamp.format("%H:%M:%S").to_string();

            tbl = tbl.push_row(vec![
                table::text_cell(icon),
                table::text_cell(time_str),
                table::text_cell(&event.domain),
                table::text_cell(&event.cert_key),
                table::text_cell(description),
            ]);
        }

        let mut col = column![header, tbl.build()]
            .spacing(scaled(8.0))
            .width(Length::Fill);

        if start > 0 {
            col = col.push(
                text(format!("{} older events omitted", start))
                    .font(Font::MONOSPACE)
                    .size(text_size(11))
                    .color(self.theme().extended_palette().background.weak.text),
            );
        }

        col.into()
    }
}

// ── Helper functions ────────────────────────────────────────────────────

fn stat_badge<'a>(label: &str, total: usize, valid: usize) -> Element<'a, Message> {
    let (display, color) = if total == 0 {
        (format!("{label}: 0"), Color::from_rgb(0.5, 0.5, 0.5))
    } else if valid == total {
        (
            format!("{label}: {valid}/{total} valid"),
            Color::from_rgb(0.2, 0.75, 0.35),
        )
    } else {
        (
            format!("{label}: {valid}/{total} valid"),
            Color::from_rgb(0.90, 0.63, 0.22),
        )
    };

    container(
        text(display)
            .font(Font::MONOSPACE)
            .size(text_size(11))
            .color(color),
    )
    .padding(iced::Padding::from([scaled(3.0), scaled(10.0)]))
    .style(move |theme: &Theme| {
        let palette = theme.extended_palette();
        let bg = if palette.is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.04)
        };
        container::Style {
            background: Some(bg.into()),
            border: Border {
                radius: scaled(10.0).into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        }
    })
    .into()
}

const WARN_THRESHOLD_SECS: i64 = 7 * 24 * 60 * 60;
const ERROR_THRESHOLD_SECS: i64 = 24 * 60 * 60;

fn cert_status_cell<'a>(info: &cruma_tunnels_lib::CertInfo) -> Element<'a, Message> {
    let color = if !info.is_valid {
        Color::from_rgb(0.85, 0.25, 0.25)
    } else if info.seconds_until_expiry < ERROR_THRESHOLD_SECS {
        Color::from_rgb(0.9, 0.5, 0.1)
    } else if info.seconds_until_expiry < WARN_THRESHOLD_SECS {
        Color::from_rgb(0.90, 0.63, 0.22)
    } else {
        Color::from_rgb(0.2, 0.75, 0.35)
    };

    container(
        text("●")
            .font(Font::MONOSPACE)
            .size(text_size(13))
            .color(color),
    )
    .center_x(Length::Fill)
    .into()
}

fn remaining_cell<'a>(
    remaining: &str,
    seconds_until_expiry: i64,
    is_valid: bool,
) -> Element<'a, Message> {
    let color = if !is_valid {
        Color::from_rgb(0.85, 0.25, 0.25)
    } else if seconds_until_expiry < ERROR_THRESHOLD_SECS {
        Color::from_rgb(0.9, 0.5, 0.1)
    } else if seconds_until_expiry < WARN_THRESHOLD_SECS {
        Color::from_rgb(0.90, 0.63, 0.22)
    } else {
        Color::from_rgb(0.2, 0.75, 0.35)
    };

    text(remaining.to_string())
        .font(Font::MONOSPACE)
        .color(color)
        .into()
}

fn format_duration_approx(seconds: i64) -> String {
    if seconds < 0 {
        let abs = (-seconds) as u64;
        if abs < 3600 {
            return format!("expired {}m ago", abs / 60);
        } else if abs < 86400 {
            return format!("expired {}h ago", abs / 3600);
        } else {
            return format!("expired {}d ago", abs / 86400);
        }
    }
    let s = seconds as u64;
    if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d {}h", s / 86400, (s % 86400) / 3600)
    }
}

fn format_acme_event_kind(kind: &cruma_tunnels_lib::AcmeEventKind) -> (&'static str, String) {
    use cruma_tunnels_lib::AcmeEventKind;
    match kind {
        AcmeEventKind::IssuanceStarted {
            names,
            challenge_type,
        } => (
            "🔄",
            format!("Issuance started ({}) — {}", challenge_type, names.join(", ")),
        ),
        AcmeEventKind::IssuanceJoined => ("🔗", "Joined existing issuance".to_string()),
        AcmeEventKind::Progress { status } => ("⋯", format!("Progress: {}", status)),
        AcmeEventKind::ChallengeReady {
            identifier,
            challenge_type,
        } => (
            "📋",
            format!("Challenge ready ({}) for {}", challenge_type, identifier),
        ),
        AcmeEventKind::DnsPropagationWaiting {
            identifier,
            expected_value,
        } => (
            "⏳",
            format!("DNS propagation wait: {} (expecting {})", identifier, expected_value),
        ),
        AcmeEventKind::DnsPropagationConfirmed { identifier } => (
            "✔️",
            format!("DNS propagation confirmed: {}", identifier),
        ),
        AcmeEventKind::OrderReady => ("📦", "Order ready".to_string()),
        AcmeEventKind::OrderFinalizing => ("⚙️", "Order finalizing".to_string()),
        AcmeEventKind::IssuanceSucceeded => ("✅", "Certificate issued successfully".to_string()),
        AcmeEventKind::IssuanceFailed { error } => ("❌", format!("Issuance failed: {}", error)),
        AcmeEventKind::ServedFromCache => ("💾", "Served from cache".to_string()),
        AcmeEventKind::DomainBlocked {
            block_seconds,
            reason,
        } => (
            "🚫",
            format!("Domain blocked for {}s: {}", block_seconds, reason),
        ),
        AcmeEventKind::ChallengeCleanup {
            identifier,
            challenge_type,
            success,
        } => {
            if *success {
                ("🧹", format!("Cleanup {} ({}) — ok", identifier, challenge_type))
            } else {
                ("⚠️", format!("Cleanup {} ({}) — failed", identifier, challenge_type))
            }
        }
    }
}
