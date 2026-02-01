use iced::widget::{
    Column, Row, Scrollable, Space, button, checkbox, column, container, pick_list, row,
    scrollable, text, text_input,
};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};
use tracing::Level;

use super::super::{LogLevelPreset, Message, OddBoxGui};

/// Maximum number of log entries to render (for performance)
pub(in crate::gui) const MAX_RENDERED_LOGS: usize = 1000;

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
            text("Monitoring")
                .size(20)
                .color(Color::from_rgb(0.9, 0.9, 0.9)),
            Space::new().width(Length::Fill),
            button(text("Clear Logs").color(Color::from_rgb(0.9, 0.9, 0.9)))
                .padding(Padding {
                    top: 6.0,
                    right: 12.0,
                    bottom: 6.0,
                    left: 12.0,
                })
                .style(|_theme: &Theme, status| {
                    let bg = match status {
                        button::Status::Hovered => Color::from_rgb(0.25, 0.25, 0.28),
                        _ => Color::from_rgb(0.18, 0.18, 0.20),
                    };
                    button::Style {
                        background: Some(bg.into()),
                        text_color: Color::from_rgb(0.9, 0.9, 0.9),
                        border: Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: Color::from_rgb(0.25, 0.25, 0.28),
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

        container(content)
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

        let tail_toggle = checkbox(self.log_auto_tail)
            .label("Tail")
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
            text(log_count).color(Color::from_rgb(0.5, 0.5, 0.55)),
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
                button(text("Clear").color(Color::from_rgb(0.8, 0.8, 0.8)))
                    .padding(Padding {
                        top: 4.0,
                        right: 8.0,
                        bottom: 4.0,
                        left: 8.0,
                    })
                    .style(|_theme: &Theme, status| {
                        let bg = match status {
                            button::Status::Hovered => Color::from_rgb(0.22, 0.22, 0.25),
                            _ => Color::from_rgb(0.18, 0.18, 0.20),
                        };
                        iced::widget::button::Style {
                            background: Some(bg.into()),
                            text_color: Color::from_rgb(0.8, 0.8, 0.8),
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
            let chip = button(text(source.as_str()).color(if is_selected {
                Color::WHITE
            } else {
                Color::from_rgb(0.75, 0.75, 0.75)
            }))
            .padding(Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            })
            .style(move |_theme: &Theme, status| {
                let bg = if is_selected {
                    Color::from_rgb(0.55, 0.35, 0.75)
                } else {
                    match status {
                        button::Status::Hovered => Color::from_rgb(0.22, 0.22, 0.25),
                        _ => Color::from_rgb(0.16, 0.16, 0.18),
                    }
                };
                iced::widget::button::Style {
                    background: Some(bg.into()),
                    text_color: if is_selected {
                        Color::WHITE
                    } else {
                        Color::from_rgb(0.75, 0.75, 0.75)
                    },
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
                text("Filter by source:").color(Color::from_rgb(0.5, 0.5, 0.55)),
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
            return container(text(msg).color(Color::from_rgb(0.5, 0.5, 0.55)))
                .padding(20)
                .width(Length::Fill)
                .into();
        }

        // Limit rendered entries for performance (show most recent)
        let total_filtered = self.cached_log_lines.len();
        let skip_count = total_filtered.saturating_sub(MAX_RENDERED_LOGS);
        let entries_to_render = self.cached_log_lines.iter().skip(skip_count);

        let muted_color = Color::from_rgb(0.45, 0.45, 0.5);
        let message_color = Color::from_rgb(0.85, 0.85, 0.85);

        // Build log entries with metadata row above message
        let rows: Vec<Element<'_, Message>> = entries_to_render
            .map(|line| {
                // Metadata row: level, source, timestamp
                let metadata_row = row![
                    text(line.level_str)
                        .font(Font::MONOSPACE)
                        .color(line.level_color),
                    text(&line.source)
                        .font(Font::MONOSPACE)
                        .color(muted_color),
                    text(&line.timestamp_str)
                        .font(Font::MONOSPACE)
                        .color(muted_color),
                ]
                .spacing(12);

                // Message content
                let message_widget: Element<'_, Message> = if self.log_wrap_enabled {
                    text(&line.message)
                        .font(Font::MONOSPACE)
                        .color(message_color)
                        .wrapping(text::Wrapping::Word)
                        .into()
                } else {
                    // Handle multi-line messages without wrapping
                    let message_lines: Vec<&str> = line.message.lines().collect();
                    if message_lines.len() > 1 {
                        let line_elements: Vec<Element<'_, Message>> = message_lines
                            .into_iter()
                            .map(|msg_line| {
                                text(msg_line)
                                    .font(Font::MONOSPACE)
                                    .color(message_color)
                                    .wrapping(text::Wrapping::None)
                                    .into()
                            })
                            .collect();
                        Column::with_children(line_elements).spacing(1).into()
                    } else {
                        text(&line.message)
                            .font(Font::MONOSPACE)
                            .color(message_color)
                            .wrapping(text::Wrapping::None)
                            .into()
                    }
                };

                // Stack metadata above message
                column![metadata_row, message_widget]
                    .spacing(2)
                    .into()
            })
            .collect();

        let log_column = Column::with_children(rows).spacing(12);

        // Container for log content
        let log_container = container(log_column).padding(Padding {
            top: 10.0,
            right: 15.0,
            bottom: 10.0,
            left: 15.0,
        });

        // Add truncation notice if needed
        let content: Element<'_, Message> = if skip_count > 0 {
            let notice = container(
                text(format!(
                    "Showing last {} of {} entries",
                    MAX_RENDERED_LOGS, total_filtered
                ))
                .color(muted_color),
            )
            .padding(Padding {
                top: 5.0,
                right: 15.0,
                bottom: 5.0,
                left: 15.0,
            });
            column![notice, log_container].into()
        } else {
            log_container.into()
        };

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

        // Anchor to bottom when auto-tail is enabled
        if self.log_auto_tail {
            scroller = scroller.anchor_bottom();
        }

        scroller.into()
    }

    pub(in crate::gui) fn has_active_filter(&self) -> bool {
        !self.log_filter.text.is_empty()
            || !self.log_filter.sources.is_empty()
            || self.log_level_preset != LogLevelPreset::All
    }

    /// Refresh the log cache. Returns true if new logs were added.
    pub(in crate::gui) fn refresh_log_cache(&mut self, force: bool) -> bool {
        let state = self.log_state.read();

        // Early-out if nothing changed and not forced
        let current_last_id = state.last_id();
        if !force && current_last_id == self.last_seen_log_id {
            return false;
        }
        let had_new_logs = current_last_id != self.last_seen_log_id;
        self.last_seen_log_id = current_last_id;

        // Update known sources
        let sources = state.known_sources().clone();
        let mut sources_vec: Vec<String> = sources.into_iter().collect();
        sources_vec.sort();
        self.known_sources = sources_vec;

        self.total_log_count = state.len();

        // Apply filter and cache results
        let filtered = self.log_filter.apply(state.entries());

        self.cached_log_lines = filtered
            .into_iter()
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
            .collect();

        self.last_log_count = self.cached_log_lines.len();
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
