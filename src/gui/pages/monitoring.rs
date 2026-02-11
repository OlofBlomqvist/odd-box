use iced::widget::{
    Column, Row, Scrollable, Space, Stack, button, checkbox, column, container, lazy, mouse_area,
    pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Padding, Theme};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::Level;

use super::super::{LogLevelPreset, Message, OddBoxGui};

/// Maximum number of log entries to render (for performance)
pub(in crate::gui) const MAX_RENDERED_LOGS: usize = 300;

/// Cached/pre-rendered log line for performance
#[derive(Clone)]
pub(in crate::gui) struct CachedLogLine {
    pub id: u64,
    pub level_str: &'static str,
    pub level_color: Color,
    pub source: String,
    pub timestamp_str: String,
    pub message: String,
}

impl OddBoxGui {
    pub(in crate::gui) fn view_monitoring(&self) -> Element<'_, Message> {
        let title_row = row![
            text("Monitoring").size(super::super::text_size(20)),
            Space::new().width(Length::Fill),
            button(text("Clear Logs"))
                .padding(Padding {
                    top: 6.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                })
                .style(|theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let bg = match status {
                        button::Status::Hovered => palette.danger.strong.color,
                        button::Status::Disabled => palette.background.weak.color,
                        _ => palette.danger.weak.color,
                    };
                    let fg = match status {
                        button::Status::Disabled => palette.background.weak.text,
                        _ => Color::WHITE,
                    };
                    button::Style {
                        background: Some(bg.into()),
                        text_color: fg,
                        border: Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: palette.danger.strong.color,
                        },
                        ..Default::default()
                    }
                })
                .on_press(Message::LogsClear),
        ]
        .align_y(iced::Alignment::Center);

        // Filter controls
        let filter_bar = self.view_log_filter_bar();

        // Source filter
        let source_filter = self.view_source_filter();

        // Log entries
        let log_entries = self.view_log_entries();

        let content = column![title_row, filter_bar, source_filter, log_entries]
            .spacing(15)
            .padding(30)
            .width(Length::Fill)
            .height(Length::Fill);

        let base: Element<'_, Message> = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let Some(modal) = &self.log_modal else {
            return base;
        };

        let header = row![
            text("Log Entry").size(super::super::text_size(20)),
            Space::new().width(Length::Fill),
            button(text("Close"))
                .padding(Padding {
                    top: 4.0,
                    right: 10.0,
                    bottom: 4.0,
                    left: 10.0,
                })
                .on_press(Message::LogCloseEntry),
        ]
        .align_y(iced::Alignment::Center);

        let meta = row![
            text(modal.level_str)
                .font(Font::MONOSPACE)
                .color(modal.level_color),
            text(modal.source.clone())
                .font(Font::MONOSPACE)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
            text(modal.timestamp_str.clone())
                .font(Font::MONOSPACE)
                .style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
        ]
        .spacing(12);

        let message = Scrollable::new(
            container(
                text(modal.message.clone())
                    .font(Font::MONOSPACE)
                    .style(|theme: &Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().background.base.text),
                        ..Default::default()
                    })
                    .wrapping(text::Wrapping::Word),
            )
            .padding(Padding {
                top: 8.0,
                right: 8.0,
                bottom: 8.0,
                left: 8.0,
            }),
        )
        .height(Length::Fixed(260.0));

        let actions = row![
            Space::new().width(Length::Fill),
            button(text("Copy"))
                .padding(Padding {
                    top: 6.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                })
                .on_press(Message::LogCopyEntry),
        ];

        let modal_card = container(column![header, meta, message, actions].spacing(14))
            .padding(Padding {
                top: 16.0,
                right: 18.0,
                bottom: 16.0,
                left: 18.0,
            })
            .width(Length::Fixed(720.0))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                iced::widget::container::Style {
                    background: Some(palette.background.weak.color.into()),
                    text_color: Some(palette.background.base.text),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: palette.background.strong.color,
                    },
                    ..Default::default()
                }
            });

        let dimmer = container(column![])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| {
                let mut bg = theme.extended_palette().background.strong.color;
                bg.a = 0.6;
                iced::widget::container::Style {
                    background: Some(bg.into()),
                    ..Default::default()
                }
            });

        let dismiss = mouse_area(dimmer).on_press(Message::LogCloseEntry);

        let overlay = container(mouse_area(modal_card).on_press(Message::NoOp))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        Stack::with_children(vec![base, dismiss.into(), overlay.into()])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_log_filter_bar(&self) -> Element<'_, Message> {
        let search_input = text_input("Search logs...", &self.log_filter.text)
            .on_input(Message::LogFilterTextChanged)
            .padding(8)
            .width(Length::Fixed(250.0));

        let level_picker = pick_list(
            LogLevelPreset::ALL.as_slice(),
            Some(self.log_level_preset),
            Message::LogLevelPresetChanged,
        )
        .padding(Padding {
            top: 6.0,
            right: 10.0,
            bottom: 6.0,
            left: 10.0,
        })
        .width(Length::Fixed(120.0));

        let wrap_toggle = checkbox(self.log_wrap_enabled)
            .label("Wrap")
            .on_toggle(Message::LogToggleWrap);

        let tail_label = if self.log_auto_tail && !self.log_is_at_bottom {
            "Tail (paused)"
        } else {
            "Tail"
        };
        let tail_toggle = checkbox(self.log_auto_tail)
            .label(tail_label)
            .on_toggle(Message::LogToggleAutoTail);

        let log_count = if self.has_active_filter() {
            format!("{} / {} logs", self.last_log_count, self.total_log_count)
        } else {
            format!("{} logs", self.total_log_count)
        };

        row![
            search_input,
            Space::new().width(Length::Fixed(15.0)),
            level_picker,
            Space::new().width(Length::Fill),
            wrap_toggle,
            tail_toggle,
            Space::new().width(Length::Fixed(20.0)),
            text(log_count).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.text),
                ..Default::default()
            }),
        ]
        .spacing(15)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_source_filter(&self) -> Element<'_, Message> {
        if self.known_sources.is_empty() {
            return Space::new().height(Length::Fixed(0.0)).into();
        }

        let mut source_chips: Vec<Element<'_, Message>> = Vec::new();

        // Add "Clear" button if any sources are selected
        if !self.log_filter.sources.is_empty() {
            source_chips.push(
                button(text("Clear"))
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
                                (palette.danger.strong.color, palette.danger.strong.text)
                            }
                            button::Status::Disabled => {
                                (palette.background.weak.color, palette.background.weak.text)
                            }
                            _ => (palette.danger.weak.color, palette.danger.weak.text),
                        };
                        iced::widget::button::Style {
                            background: Some(bg.into()),
                            text_color: fg,
                            border: Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .on_press(Message::LogFilterClearSources)
                    .into(),
            );
        }

        // Add source chips
        for source in &self.known_sources {
            let is_selected = self.log_filter.sources.contains(source);
            let source_clone = source.clone();
            let chip = button(text(source.as_str()))
                .padding(Padding {
                    top: 4.0,
                    right: 8.0,
                    bottom: 4.0,
                    left: 8.0,
                })
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let (bg, fg) = if is_selected {
                        (palette.primary.strong.color, palette.primary.strong.text)
                    } else {
                        match status {
                            button::Status::Hovered => {
                                (palette.background.weak.color, palette.background.weak.text)
                            }
                            _ => (
                                palette.background.weaker.color,
                                palette.background.weak.text,
                            ),
                        }
                    };
                    iced::widget::button::Style {
                        background: Some(bg.into()),
                        text_color: fg,
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })
                .on_press(Message::LogFilterToggleSource(source_clone, !is_selected));
            source_chips.push(chip.into());
        }

        let chips_row = Row::with_children(source_chips).spacing(6).wrap();

        container(
            column![
                text("Filter by source:").style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }),
                chips_row,
            ]
            .spacing(6),
        )
        .padding(Padding {
            top: 5.0,
            right: 0.0,
            bottom: 5.0,
            left: 0.0,
        })
        .into()
    }

    fn view_log_entries(&self) -> Element<'_, Message> {
        if self.cached_log_lines.is_empty() {
            let msg = if self.total_log_count == 0 {
                "No logs yet..."
            } else {
                "No logs match the current filter"
            };
            return container(text(msg).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.weak.text),
                ..Default::default()
            }))
            .padding(20)
            .width(Length::Fill)
            .into();
        }

        #[derive(Clone)]
        struct LogEntriesDep {
            rev: u64,
            wrap: bool,
            lines: Arc<Vec<CachedLogLine>>,
        }

        impl Hash for LogEntriesDep {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.rev.hash(state);
                self.wrap.hash(state);
                // `lines` are only used for rendering; hashing them would defeat the purpose.
            }
        }

        let dep = LogEntriesDep {
            rev: self.log_view_rev,
            wrap: self.log_wrap_enabled,
            lines: self.cached_log_lines.clone(),
        };

        let content: Element<'_, Message> = lazy(dep, |dep| {
            // Limit rendered entries for performance (show most recent)
            let total_filtered = dep.lines.len();
            let skip_count = total_filtered.saturating_sub(MAX_RENDERED_LOGS);
            let entries_to_render = dep.lines.iter().skip(skip_count);

            // Build log entries with metadata row above message
            let rows: Vec<Element<'static, Message>> = entries_to_render
                .map(|line| {
                    let metadata_row = row![
                        text(line.level_str)
                            .font(Font::MONOSPACE)
                            .color(line.level_color),
                        text(line.source.clone())
                            .font(Font::MONOSPACE)
                            .style(|theme: &Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().background.weak.text),
                                ..Default::default()
                            }),
                        text(line.timestamp_str.clone())
                            .font(Font::MONOSPACE)
                            .style(|theme: &Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().background.weak.text),
                                ..Default::default()
                            }),
                    ]
                    .spacing(12);

                    let message_widget: Element<'static, Message> = if dep.wrap {
                        text(line.message.clone())
                            .font(Font::MONOSPACE)
                            .style(|theme: &Theme| iced::widget::text::Style {
                                color: Some(theme.extended_palette().background.base.text),
                                ..Default::default()
                            })
                            .wrapping(text::Wrapping::Word)
                            .into()
                    } else {
                        let message_lines: Vec<&str> = line.message.lines().collect();
                        if message_lines.len() > 1 {
                            let line_elements: Vec<Element<'static, Message>> = message_lines
                                .into_iter()
                                .map(|msg_line| {
                                    text(msg_line.to_string())
                                        .font(Font::MONOSPACE)
                                        .style(|theme: &Theme| iced::widget::text::Style {
                                            color: Some(
                                                theme.extended_palette().background.base.text,
                                            ),
                                            ..Default::default()
                                        })
                                        .wrapping(text::Wrapping::None)
                                        .into()
                                })
                                .collect();
                            Column::with_children(line_elements).spacing(1).into()
                        } else {
                            text(line.message.clone())
                                .font(Font::MONOSPACE)
                                .style(|theme: &Theme| iced::widget::text::Style {
                                    color: Some(theme.extended_palette().background.base.text),
                                    ..Default::default()
                                })
                                .wrapping(text::Wrapping::None)
                                .into()
                        }
                    };

                    let entry = column![metadata_row, message_widget].spacing(2);
                    let entry_container = if dep.wrap {
                        container(entry).width(Length::Fill)
                    } else {
                        container(entry).width(Length::Shrink)
                    };
                    mouse_area(entry_container)
                        .on_press(Message::LogOpenEntry(line.id))
                        .into()
                })
                .collect();

            let log_column = Column::with_children(rows).spacing(12);

            let right_pad = if dep.wrap { 40.0 } else { 15.0 };
            let log_container: Element<'static, Message> = container(log_column)
                .padding(Padding {
                    top: 10.0,
                    right: right_pad,
                    bottom: 10.0,
                    left: 15.0,
                })
                .into();

            if skip_count > 0 {
                let notice = container(
                    text(format!(
                        "Showing last {} of {} entries",
                        MAX_RENDERED_LOGS, total_filtered
                    ))
                    .style(|theme: &Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().background.weak.text),
                        ..Default::default()
                    }),
                )
                .padding(Padding {
                    top: 5.0,
                    right: 15.0,
                    bottom: 5.0,
                    left: 15.0,
                });
                let content: Element<'static, Message> = column![notice, log_container].into();
                content
            } else {
                log_container
            }
        })
        .into();

        let mut scroller = if self.log_wrap_enabled {
            Scrollable::new(container(content).width(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
        } else {
            Scrollable::new(container(content).width(Length::Shrink))
                .direction(scrollable::Direction::Both {
                    vertical: scrollable::Scrollbar::default(),
                    horizontal: scrollable::Scrollbar::default(),
                })
                .width(Length::Fill)
                .height(Length::Fill)
        };

        scroller = scroller
            .id(super::super::log_scroll_id())
            .on_scroll(Message::LogViewportChanged);

        scroller.into()
    }

    pub(in crate::gui) fn has_active_filter(&self) -> bool {
        !self.log_filter.text.is_empty()
            || !self.log_filter.sources.is_empty()
            || self.log_level_preset != LogLevelPreset::All
    }

    /// Refresh the log cache. Returns true if new logs were added.
    pub(in crate::gui) fn refresh_log_cache(&mut self, force: bool) -> bool {
        let _ = self.log_state.drain();
        let state = self.log_state.snapshot();

        // Early-out if nothing changed and not forced
        let current_last_id = state.last_id();
        let previous_last_id = self.last_seen_log_id;
        if !force && current_last_id == previous_last_id {
            return false;
        }
        let had_new_logs = current_last_id != previous_last_id;
        self.last_seen_log_id = current_last_id;

        // Update known sources
        let sources = state.known_sources().clone();
        let mut sources_vec: Vec<String> = sources.into_iter().collect();
        sources_vec.sort();
        self.known_sources = sources_vec;

        self.total_log_count = state.len();

        // Apply filter and cache results
        if force || previous_last_id.is_none() {
            let filtered = self.log_filter.apply(state.entries());
            let filtered_len = filtered.len();
            let to_render = filtered
                .into_iter()
                .skip(filtered_len.saturating_sub(MAX_RENDERED_LOGS));

            self.cached_log_lines = Arc::new(
                to_render
                    .map(|entry| {
                        let (level_str, level_color) = Self::level_display(entry.level);
                        let source = entry
                            .thread
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .cloned()
                            .or_else(|| {
                                if entry.source.is_empty() {
                                    None
                                } else {
                                    Some(entry.source.clone())
                                }
                            })
                            .unwrap_or_else(|| "-".to_string());

                        let timestamp_str = entry.timestamp.format("%H:%M:%S%.3f").to_string();

                        CachedLogLine {
                            id: entry.id,
                            level_str,
                            level_color,
                            source,
                            timestamp_str,
                            message: entry.message.clone(),
                        }
                    })
                    .collect(),
            );

            self.last_log_count = filtered_len;
            self.log_view_rev = self.log_view_rev.wrapping_add(1);
        } else if had_new_logs {
            // Incremental update: only process new entries since `previous_last_id`.
            let mut new_entries = Vec::new();
            let mut found_previous = false;

            for entry in state.entries().iter().rev() {
                if Some(entry.id) == previous_last_id {
                    found_previous = true;
                    break;
                }
                new_entries.push(entry);
            }

            if !found_previous {
                // We lost track (e.g. rotation or clear). Fall back to a full rebuild.
                return self.refresh_log_cache(true);
            }

            new_entries.reverse();

            let mut added_any = false;
            let mut next = Vec::with_capacity(
                self.cached_log_lines
                    .len()
                    .saturating_add(new_entries.len()),
            );
            next.extend(self.cached_log_lines.iter().cloned());

            for entry in new_entries {
                if !self.log_filter.matches(entry) {
                    continue;
                }

                let (level_str, level_color) = Self::level_display(entry.level);
                let source = entry
                    .thread
                    .as_ref()
                    .filter(|t| !t.is_empty())
                    .cloned()
                    .or_else(|| {
                        if entry.source.is_empty() {
                            None
                        } else {
                            Some(entry.source.clone())
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                let timestamp_str = entry.timestamp.format("%H:%M:%S%.3f").to_string();

                next.push(CachedLogLine {
                    id: entry.id,
                    level_str,
                    level_color,
                    source,
                    timestamp_str,
                    message: entry.message.clone(),
                });
                self.last_log_count = self.last_log_count.saturating_add(1);
                added_any = true;
            }

            if next.len() > MAX_RENDERED_LOGS {
                let drain = next.len() - MAX_RENDERED_LOGS;
                next.drain(0..drain);
                added_any = true;
            }

            if added_any {
                self.cached_log_lines = Arc::new(next);
                self.log_view_rev = self.log_view_rev.wrapping_add(1);
            }
        }

        had_new_logs
    }

    pub(in crate::gui) fn level_display(level: Level) -> (&'static str, Color) {
        match level {
            Level::TRACE => ("TRC", Color::from_rgb(0.5, 0.5, 0.55)),
            Level::DEBUG => ("DBG", Color::from_rgb(0.4, 0.7, 1.0)),
            Level::INFO => ("INF", Color::from_rgb(0.4, 0.85, 0.4)),
            Level::WARN => ("WRN", Color::from_rgb(1.0, 0.8, 0.3)),
            Level::ERROR => ("ERR", Color::from_rgb(1.0, 0.4, 0.4)),
        }
    }
}
