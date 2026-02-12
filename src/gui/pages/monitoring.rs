use std::hash::Hash;
use std::time::Instant;

use iced::widget::{
    Row, Scrollable, Space, button, checkbox, column, container, keyed_column, lazy, pick_list,
    row, scrollable, text, text_input,
};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};
use tracing::Level;

use super::super::{LogLevelPreset, Message, OddBoxGui};

/// Dependency for lazy log entries widget - only rebuilds when these change
/// IMPORTANT: This must be cheap to compute since it runs every frame
#[derive(Clone, Hash)]
struct LogEntriesDeps {
    /// Last filtered log ID (changes when filter results change)
    last_filtered_id: Option<u64>,
    /// First filtered log ID (changes when old entries are trimmed)
    first_filtered_id: Option<u64>,
    /// Number of filtered entries (cheap check for changes)
    filtered_count: usize,
    /// Whether word wrap is enabled
    wrap: bool,
}

/// Maximum number of log entries to render for performance
pub(in crate::gui) const MAX_RENDERED_LOGS: usize = 100;

impl OddBoxGui {
    pub(in crate::gui) fn view_monitoring(&self) -> Element<'_, Message> {
        let start = Instant::now();
        let use_kde_buttons = self.use_kde_system_styles();
        
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
                .style(move |theme: &Theme, status| {
                    if use_kde_buttons {
                        return super::super::kde_danger_button_style(theme, status);
                    }
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
        let log_entries = container(self.view_log_entries())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| {
                container::Style {
                    background: Some(self.surface_panel_bg(theme).into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
                    },
                    ..Default::default()
                }
            });

        let content = column![title_row, filter_bar, source_filter, log_entries]
            .spacing(15)
            .padding(30)
            .width(Length::Fill)
            .height(Length::Fill);

        let result: Element<'_, Message> = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 5 {
            tracing::warn!("view_monitoring took {:?}", elapsed);
        }

        result
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

        let filtered = self.log_state.filtered_snapshot();
        let log_count = if self.has_active_filter() {
            format!("{} / {} logs", filtered.filtered_count, filtered.total_count)
        } else {
            format!("{} logs", filtered.total_count)
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
        let use_kde_buttons = self.use_kde_system_styles();
        let known_sources = self.log_state.filtered_snapshot().known_sources.clone();
        if known_sources.is_empty() {
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
                    .style(move |theme: &Theme, status| {
                        if use_kde_buttons {
                            return super::super::kde_danger_button_style(theme, status);
                        }
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
        for source in known_sources {
            let is_selected = self.log_filter.sources.contains(&source);
            let chip = button(text(source.clone()))
                .padding(Padding {
                    top: 4.0,
                    right: 8.0,
                    bottom: 4.0,
                    left: 8.0,
                })
                .style(move |theme: &Theme, status| {
                    if use_kde_buttons {
                        if is_selected {
                            let selected_status = match status {
                                button::Status::Hovered | button::Status::Pressed => status,
                                _ => button::Status::Pressed,
                            };
                            return super::super::kde_primary_button_style(theme, selected_status);
                        }
                        return super::super::kde_neutral_button_style(theme, status);
                    }
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
                .on_press(Message::LogFilterToggleSource(source, !is_selected));
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
        let start = Instant::now();
        
        // Get pre-filtered snapshot from background task - this is CHEAP (just Arc load)
        let filtered = self.log_state.filtered_snapshot();
        let total_filtered = filtered.filtered_count;

        if total_filtered == 0 {
            let msg = if filtered.total_count == 0 {
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

        // Only render the last MAX_RENDERED_LOGS entries for performance
        let skip_count = total_filtered.saturating_sub(MAX_RENDERED_LOGS);
        
        // CHEAP dependency computation - just grab first/last IDs and count
        let first_filtered_id = filtered.entries.get(skip_count).map(|e| e.id);
        let last_filtered_id = filtered.last_filtered_id;
        
        let wrap = self.log_wrap_enabled;
        let deps = LogEntriesDeps { 
            last_filtered_id,
            first_filtered_id,
            filtered_count: total_filtered,
            wrap,
        };
        
        // Clone what we need for the lazy closure - Arc clones are cheap
        let filtered_entries = filtered.entries.clone();

        // Use lazy to avoid rebuilding widget tree on every frame
        // ALL expensive work happens inside this closure, which only runs when deps change
        let log_rows_lazy = lazy(deps, move |deps| {
            let skip = filtered_entries.len().saturating_sub(MAX_RENDERED_LOGS);
            let entries_to_render: Vec<_> =
                filtered_entries.iter().skip(skip).cloned().collect();
            
            let wrap = deps.wrap;

            // Build log rows using keyed_column for efficient diffing
            let log_rows = keyed_column(
                entries_to_render
                    .into_iter()
                    .map(|entry| {
                        let entry_id = entry.id;
                        
                        // Build the row element
                        let (level_str, level_color) = level_display(entry.level);
                        
                        let source: String = entry
                            .thread
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .map(|t| t.to_string())
                            .or_else(|| {
                                if entry.source.is_empty() {
                                    None
                                } else {
                                    Some(entry.source.to_string())
                                }
                            })
                            .unwrap_or_else(|| "-".to_string());

                        let timestamp_str = entry.timestamp.format("%H:%M:%S%.3f").to_string();
                        let message = entry.message.to_string();

                        let metadata_row = row![
                            text(level_str)
                                .font(Font::MONOSPACE)
                                .color(level_color),
                            text(source.clone())
                                .font(Font::MONOSPACE)
                                .style(|theme: &Theme| iced::widget::text::Style {
                                    color: Some(theme.extended_palette().background.weak.text),
                                    ..Default::default()
                                }),
                            text(timestamp_str.clone())
                                .font(Font::MONOSPACE)
                                .style(|theme: &Theme| iced::widget::text::Style {
                                    color: Some(theme.extended_palette().background.weak.text),
                                    ..Default::default()
                                }),
                        ]
                        .spacing(12);

                        let message_widget: Element<'static, Message> = if wrap {
                            text(message.clone())
                                .width(Length::Fill)
                                .font(Font::MONOSPACE)
                                .style(|theme: &Theme| iced::widget::text::Style {
                                    color: Some(theme.extended_palette().background.base.text),
                                    ..Default::default()
                                })
                                .wrapping(text::Wrapping::Word)
                                .into()
                        } else {
                            text(message.clone())
                                .font(Font::MONOSPACE)
                                .style(|theme: &Theme| iced::widget::text::Style {
                                    color: Some(theme.extended_palette().background.base.text),
                                    ..Default::default()
                                })
                                .wrapping(text::Wrapping::None)
                                .into()
                        };

                        let entry_widget = if wrap {
                            column![metadata_row, message_widget]
                                .spacing(2)
                                .width(Length::Fill)
                        } else {
                            column![metadata_row, message_widget].spacing(2)
                        };
                        
                        let entry_container = if wrap {
                            container(entry_widget).width(Length::Fill)
                        } else {
                            container(entry_widget).width(Length::Shrink)
                        };

                        let row_element: Element<'static, Message> = entry_container.into();

                        (entry_id as usize, row_element)
                    }),
            )
            .spacing(12);
            let log_rows = if wrap {
                log_rows.width(Length::Fill)
            } else {
                log_rows
            };

            let right_pad = if wrap { 40.0 } else { 15.0 };
            let rows_container = container(log_rows)
                .padding(Padding {
                    top: 10.0,
                    right: right_pad,
                    bottom: 10.0,
                    left: 15.0,
                });

            if wrap {
                rows_container.width(Length::Fill)
            } else {
                rows_container
            }
        });

        // Update cached counts for display (these come from background now)
        // Note: self is immutable here, so we just use filtered values directly
        
        // Build the content with optional notice
        let content: Element<'_, Message> = if skip_count > 0 {
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
            let notice: Element<'_, Message> = if wrap {
                notice.width(Length::Fill).into()
            } else {
                notice.into()
            };
            let col = column![notice, log_rows_lazy];
            if wrap {
                col.width(Length::Fill).into()
            } else {
                col.into()
            }
        } else if wrap {
            container(log_rows_lazy).width(Length::Fill).into()
        } else {
            log_rows_lazy.into()
        };

        // Use regular Scrollable
        let mut scroller = if wrap {
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

        let elapsed = start.elapsed();
        if elapsed.as_millis() > 5 {
            tracing::warn!("view_log_entries took {:?}", elapsed);
        }

        scroller.into()
    }

    pub(in crate::gui) fn has_active_filter(&self) -> bool {
        !self.log_filter.text.is_empty()
            || !self.log_filter.sources.is_empty()
            || self.log_level_preset != LogLevelPreset::All
    }





}

/// Convert a tracing Level to a display string and color
pub(in crate::gui) fn level_display(level: Level) -> (&'static str, Color) {
    match level {
        Level::TRACE => ("TRC", Color::from_rgb(0.5, 0.5, 0.55)),
        Level::DEBUG => ("DBG", Color::from_rgb(0.4, 0.7, 1.0)),
        Level::INFO => ("INF", Color::from_rgb(0.4, 0.85, 0.4)),
        Level::WARN => ("WRN", Color::from_rgb(1.0, 0.8, 0.3)),
        Level::ERROR => ("ERR", Color::from_rgb(1.0, 0.4, 0.4)),
    }
}
