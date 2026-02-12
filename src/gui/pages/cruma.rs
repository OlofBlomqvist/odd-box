use iced::widget::{Column, column, container, radio, row, text};
use iced::{Alignment, Color, Element, Font, Length, Theme};
use iced::widget::text::Wrapping;

use super::super::{CrumaAuthMode, Message, OddBoxGui};

impl OddBoxGui {
    pub(in crate::gui) fn view_cruma_ingress(&self) -> Element<'_, Message> {
        let assignment = self.state.cruma_assignment.load_full();
        let is_disabled = matches!(self.cruma_auth_mode, CrumaAuthMode::Disabled);
        let (status_label, status_color, fqdn, motd) = if is_disabled {
            (
                "Disabled",
                self.theme().extended_palette().background.weak.text,
                "—".to_string(),
                "—".to_string(),
            )
        } else if let Some(a) = assignment.as_ref() {
            (
                "Connected",
                self.theme().extended_palette().success.strong.color,
                a.assigned_domain.clone(),
                a.welcome_message.clone(),
            )
        } else {
            (
                "Waiting for assignment",
                self.theme().extended_palette().background.weak.text,
                "—".to_string(),
                "—".to_string(),
            )
        };

        let status_row = row![
            text("Status:").font(Font::MONOSPACE),
            text(status_label).color(status_color).font(Font::MONOSPACE)
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let fqdn_row = row![
            text("Assigned FQDN:").font(Font::MONOSPACE),
            container(
                text(fqdn)
                    .font(Font::MONOSPACE)
                    .wrapping(Wrapping::WordOrGlyph),
            )
            .width(Length::Fill)
        ]
        .spacing(8)
        .align_y(Alignment::Start)
        .width(Length::Fill);

        let motd_row = row![
            text("MOTD:").font(Font::MONOSPACE),
            container(
                text(motd)
                    .font(Font::MONOSPACE)
                    .wrapping(Wrapping::WordOrGlyph),
            )
            .width(Length::Fill)
        ]
        .spacing(8)
        .align_y(Alignment::Start)
        .width(Length::Fill);

        let connection_box = container(
            column![status_row, fqdn_row, motd_row]
                .spacing(6)
                .width(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(palette.background.weaker.color.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });

        let transports = self.state.cruma_transports.load_full();
        let mut h2_count = 0usize;
        let mut quic_count = 0usize;
        for t in transports.iter() {
            match t.protocol {
                cruma_tunnels_lib::AgentProtocol::Http2 => h2_count += 1,
                cruma_tunnels_lib::AgentProtocol::Quic => quic_count += 1,
            }
        }
        let transport_summary = row![
            text("Channels:").font(Font::MONOSPACE),
            text(format!("{} total (H2: {}, QUIC: {})", transports.len(), h2_count, quic_count))
                .font(Font::MONOSPACE)
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut transport_list = Column::new().spacing(4);
        if transports.is_empty() {
            transport_list =
                transport_list.push(text("No active channels.").font(Font::MONOSPACE).size(super::super::text_size(12)));
        } else {
            for t in transports.iter() {
                let proto = match t.protocol {
                    cruma_tunnels_lib::AgentProtocol::Http2 => "H2",
                    cruma_tunnels_lib::AgentProtocol::Quic => "QUIC",
                };
                transport_list = transport_list.push(
                    text(format!("{proto} #{}  {}", t.instance_idx, t.addr))
                        .font(Font::MONOSPACE)
                        .size(super::super::text_size(12)),
                );
            }
        }

        let transports_box = container(
            column![transport_summary, transport_list]
                .spacing(8)
                .width(Length::Fill),
        )
        .padding(16)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(palette.background.weaker.color.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: palette.background.strong.color,
                },
                ..Default::default()
            }
        });

        let mode_header = text("Connection Mode").font(Font::MONOSPACE).size(super::super::text_size(14));

        let disabled_radio = radio(
            "Disabled",
            CrumaAuthMode::Disabled,
            Some(self.cruma_auth_mode),
            Message::CrumaAuthModeChanged,
        );
        let anon_radio = radio(
            "Anonymous",
            CrumaAuthMode::Anonymous,
            Some(self.cruma_auth_mode),
            Message::CrumaAuthModeChanged,
        );
        let auth_radio = radio(
            "Authenticated",
            CrumaAuthMode::Authenticated,
            Some(self.cruma_auth_mode),
            Message::CrumaAuthModeChanged,
        );

        let mut mode_col = Column::new()
            .push(mode_header)
            .push(disabled_radio)
            .push(anon_radio)
            .push(auth_radio)
            .spacing(8);

        if let Some(msg) = &self.cruma_mode_notice {
            mode_col = mode_col.push(
                text(msg)
                    .color(self.theme().extended_palette().background.weak.text)
                    .size(super::super::text_size(12)),
            );
        }

        if self.cruma_auth_mode == CrumaAuthMode::Authenticated {
            mode_col = mode_col.push(
                text("Authenticated credentials are not configured yet.")
                    .color(Color::from_rgb(0.9, 0.6, 0.2))
                    .size(super::super::text_size(12)),
            );
        }

        let mode_box =
            container(mode_col)
                .padding(16)
                .width(Length::Fill)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    iced::widget::container::Style {
                        background: Some(palette.background.weaker.color.into()),
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 1.0,
                            color: palette.background.strong.color,
                        },
                        ..Default::default()
                    }
                });

        column![connection_box, transports_box, mode_box]
            .spacing(16)
            .width(Length::Fill)
            .into()
    }
}
