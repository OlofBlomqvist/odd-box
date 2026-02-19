use std::hash::Hash;
use std::time::Instant;

use iced::theme;
use iced::widget::{
    Scrollable, Space, button, checkbox, column, container, keyed_column, lazy, pick_list,
    row, scrollable, text, text_input,
};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};
use tracing::Level;

use super::super::{KdeButtonRole, LogLevelPreset, Message, OddBoxGui, scaled, text_size};

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
            text("Monitoring").size(text_size(20)),
            Space::new().width(Length::Fill),
            button(text("Clear Logs"))
                .padding(Padding {
                    top: scaled(6.0),
                    right: scaled(12.0),
                    bottom: scaled(6.0),
                    left: scaled(12.0),
                })
                .style(move |theme: &Theme, status| {
                    super::super::themed_button_style(theme, status, KdeButtonRole::Danger, use_kde_buttons)
                })
                .on_press(Message::LogsClear),
        ]
        .align_y(iced::Alignment::Center);

        // Filter controls
        let filter_bar = self.view_log_filter_bar(true);

        // Log entries
        let log_entries = container(self.view_log_entries())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &Theme| {
                let base_bg = theme.extended_palette().background.base.color;
                let logs_bg = if theme.extended_palette().is_dark {
                    theme::palette::mix(base_bg, Color::BLACK, 0.33)
                } else {
                    base_bg
                };
                container::Style {
                    background: Some(logs_bg.into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: self.surface_border_color(theme),
                    },
                    ..Default::default()
                }
            });

        let content = column![title_row, filter_bar, log_entries]
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

    pub(in crate::gui) fn view_log_filter_bar(&self, show_log_count: bool) -> Element<'_, Message> {
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

        let mut bar = row![
            search_input,
            Space::new().width(Length::Fixed(15.0)),
            level_picker,
            Space::new().width(Length::Fill),
            wrap_toggle,
            tail_toggle,
        ]
        .spacing(15)
        .align_y(iced::Alignment::Center);

        if show_log_count {
            let filtered = self.log_state.filtered_snapshot();
            let log_count = if self.has_active_filter() {
                format!("{} / {} logs", filtered.filtered_count, filtered.total_count)
            } else {
                format!("{} logs", filtered.total_count)
            };

            bar = bar
                .push(Space::new().width(Length::Fixed(20.0)))
                .push(text(log_count).style(|theme: &Theme| iced::widget::text::Style {
                    color: Some(theme.extended_palette().background.weak.text),
                    ..Default::default()
                }));
        }

        bar.into()
    }

    pub(in crate::gui) fn view_log_entries(&self) -> Element<'_, Message> {
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
                        let level_str = level_label(entry.level);
                        let entry_level = entry.level;
                        
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
                                .style(move |theme: &Theme| iced::widget::text::Style {
                                    color: Some(level_color(theme, entry_level)),
                                    ..Default::default()
                                }),
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
            || self.log_level_preset != LogLevelPreset::All
    }





}

/// Convert a tracing Level to a display string and color
pub(in crate::gui) fn level_label(level: Level) -> &'static str {
    match level {
        Level::TRACE => "TRC",
        Level::DEBUG => "DBG",
        Level::INFO => "INF",
        Level::WARN => "WRN",
        Level::ERROR => "ERR",
    }
}

pub(in crate::gui) fn level_color(theme: &Theme, level: Level) -> Color {
    if theme.extended_palette().is_dark {
        match level {
            Level::TRACE => Color::from_rgb(0.55, 0.55, 0.62),
            Level::DEBUG => Color::from_rgb(0.45, 0.73, 1.0),
            Level::INFO => Color::from_rgb(0.43, 0.86, 0.43),
            Level::WARN => Color::from_rgb(1.0, 0.8, 0.35),
            Level::ERROR => Color::from_rgb(1.0, 0.45, 0.45),
        }
    } else {
        match level {
            Level::TRACE => Color::from_rgb(0.33, 0.33, 0.38),
            Level::DEBUG => Color::from_rgb(0.13, 0.36, 0.70),
            Level::INFO => Color::from_rgb(0.10, 0.50, 0.20),
            Level::WARN => Color::from_rgb(0.68, 0.42, 0.02),
            Level::ERROR => Color::from_rgb(0.70, 0.13, 0.13),
        }
    }
}
