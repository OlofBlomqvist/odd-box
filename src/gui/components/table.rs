use iced::border;
use iced::widget::{Column as IcedColumn, Row, button, column, container, text};
use iced::{Border, Color, Element, Font, Length, Padding, Theme, theme};

/// Column width specification
#[derive(Clone, Copy)]
pub enum ColumnWidth {
    /// Fixed width in pixels
    Fixed(f32),
    /// Proportional width (like CSS flex)
    Portion(u16),
}

impl From<ColumnWidth> for Length {
    fn from(w: ColumnWidth) -> Self {
        match w {
            ColumnWidth::Fixed(px) => Length::Fixed(px),
            ColumnWidth::Portion(p) => Length::FillPortion(p),
        }
    }
}

/// Column definition
pub struct Column {
    pub title: &'static str,
    pub width: ColumnWidth,
}

impl Column {
    pub fn new(title: &'static str, width: ColumnWidth) -> Self {
        Self { title, width }
    }

    pub fn fixed(title: &'static str, width: f32) -> Self {
        Self {
            title,
            width: ColumnWidth::Fixed(width),
        }
    }

    pub fn portion(title: &'static str, portion: u16) -> Self {
        Self {
            title,
            width: ColumnWidth::Portion(portion),
        }
    }
}

/// Table builder for creating consistent tables across the GUI
pub struct Table<'a, M: Clone + 'a> {
    columns: Vec<Column>,
    rows: Vec<RowKind<'a, M>>,
    row_messages: Vec<Option<M>>,
    header_background: Option<Color>,
    row_even_background: Option<Color>,
    row_odd_background: Option<Color>,
    border_color: Option<Color>,
    spacing: f32,
    row_padding: Padding,
    header_padding: Padding,
    hover_message: Option<M>,
}

enum RowKind<'a, M: Clone + 'a> {
    Cells(Vec<Element<'a, M>>),
    Full(Element<'a, M>),
}

fn header_background(theme: &Theme) -> Color {
    let palette = theme.extended_palette();
    if palette.is_dark {
        theme::palette::mix(theme.palette().background, Color::BLACK, 0.32)
    } else {
        palette.background.strong.color
    }
}

fn row_background(theme: &Theme, is_even: bool) -> Color {
    let palette = theme.extended_palette();
    if palette.is_dark {
        if is_even {
            theme::palette::mix(theme.palette().background, Color::BLACK, 0.16)
        } else {
            theme::palette::mix(theme.palette().background, Color::BLACK, 0.23)
        }
    } else if is_even {
        palette.background.weakest.color
    } else {
        palette.background.weaker.color
    }
}

fn table_border_color(theme: &Theme) -> Color {
    let palette = theme.extended_palette();
    if palette.is_dark {
        theme::palette::mix(theme.palette().background, Color::BLACK, 0.40)
    } else {
        palette.background.strong.color
    }
}

impl<'a, M: Clone + 'a> Table<'a, M> {
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            row_messages: Vec::new(),
            header_background: None,
            row_even_background: None,
            row_odd_background: None,
            border_color: None,
            spacing: 10.0,
            row_padding: Padding {
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
                left: 12.0,
            },
            header_padding: Padding {
                top: 12.0,
                right: 12.0,
                bottom: 12.0,
                left: 12.0,
            },
            hover_message: None,
        }
    }

    /// Override table surface colors (header, even row, odd row, border).
    pub fn surface_colors(
        mut self,
        header_bg: Color,
        row_even_bg: Color,
        row_odd_bg: Color,
        border: Color,
    ) -> Self {
        self.header_background = Some(header_bg);
        self.row_even_background = Some(row_even_bg);
        self.row_odd_background = Some(row_odd_bg);
        self.border_color = Some(border);
        self
    }

    /// Enable hover effect on rows (requires a no-op message for button interactivity)
    pub fn hover(mut self, message: M) -> Self {
        self.hover_message = Some(message);
        self
    }

    /// Add a row of cell elements
    pub fn push_row(mut self, cells: Vec<Element<'a, M>>) -> Self {
        self.rows.push(RowKind::Cells(cells));
        self.row_messages.push(None);
        self
    }

    /// Add a clickable row of cell elements
    pub fn push_row_with_message(mut self, cells: Vec<Element<'a, M>>, message: M) -> Self {
        self.rows.push(RowKind::Cells(cells));
        self.row_messages.push(Some(message));
        self
    }

    /// Add a full-width row element (not constrained by column widths)
    pub fn push_full_row(mut self, content: Element<'a, M>) -> Self {
        self.rows.push(RowKind::Full(content));
        self.row_messages.push(None);
        self
    }

    /// Build the table element
    pub fn build(self) -> Element<'a, M> {
        let row_count = self.rows.len();
        let hover_message = self.hover_message.clone();
        let header_background_override = self.header_background;
        let row_even_background_override = self.row_even_background;
        let row_odd_background_override = self.row_odd_background;
        let border_color_override = self.border_color;

        // Build header row
        let header_cells: Vec<Element<'a, M>> = self
            .columns
            .iter()
            .map(|col| {
                container(
                    text(col.title)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Font::MONOSPACE
                        })
                        .style(|theme: &Theme| {
                            let palette = theme.extended_palette();
                            iced::widget::text::Style {
                                color: Some(palette.background.base.text),
                                ..Default::default()
                            }
                        }),
                )
                .width(col.width)
                .clip(true)
                .into()
            })
            .collect();

        let header_row = Row::with_children(header_cells)
            .spacing(self.spacing)
            .padding(self.header_padding)
            .align_y(iced::Alignment::Center);

        let header_container: Element<'a, M> = container(header_row)
            .width(Length::Fill)
            .style(move |theme: &Theme| {
                let header_bg =
                    header_background_override.unwrap_or_else(|| header_background(theme));
                container::Style {
                    background: Some(header_bg.into()),
                    border: Border {
                        radius: border::top(6.0),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            })
            .into();

        // Build data rows with alternating colors
        let data_rows: Vec<Element<'a, M>> = self
            .rows
            .into_iter()
            .zip(self.row_messages.into_iter())
            .enumerate()
            .map(|(idx, (row, msg))| {
                let is_even = idx % 2 == 0;
                let is_last = idx == row_count.saturating_sub(1);

                let content: Element<'a, M> = match row {
                    RowKind::Cells(cells) => {
                        let row_cells: Vec<Element<'a, M>> = cells
                            .into_iter()
                            .zip(self.columns.iter())
                            .map(|(cell, col)| container(cell).width(col.width).clip(true).into())
                            .collect();

                        Row::with_children(row_cells)
                            .spacing(self.spacing)
                            .align_y(iced::Alignment::Center)
                            .into()
                    }
                    RowKind::Full(content) => container(content).width(Length::Fill).into(),
                };

                // Use button for hover effect, otherwise use container
                let row_msg = msg.or_else(|| hover_message.clone());
                if row_msg.is_some() {
                    button(content)
                        .width(Length::Fill)
                        .padding(self.row_padding)
                        .on_press(row_msg.unwrap())
                        .style(move |theme: &Theme, status| {
                            let palette = theme.extended_palette();
                            let base_bg = if is_even {
                                row_even_background_override
                                    .unwrap_or_else(|| row_background(theme, true))
                            } else {
                                row_odd_background_override
                                    .unwrap_or_else(|| row_background(theme, false))
                            };
                            let bg = match status {
                                button::Status::Hovered | button::Status::Pressed => {
                                    if palette.is_dark {
                                        theme::palette::mix(palette.primary.weak.color, base_bg, 0.70)
                                    } else {
                                        palette.primary.weak.color
                                    }
                                }
                                _ => base_bg,
                            };
                            let border_color = match status {
                                button::Status::Hovered | button::Status::Pressed => {
                                    palette.primary.strong.color
                                }
                                _ => Color::TRANSPARENT,
                            };
                            let radius = if is_last {
                                border::bottom(6.0)
                            } else {
                                border::radius(0.0)
                            };
                            button::Style {
                                background: Some(bg.into()),
                                text_color: palette.background.base.text,
                                border: Border {
                                    radius,
                                    width: 1.0,
                                    color: border_color,
                                },
                                ..Default::default()
                            }
                        })
                        .into()
                } else {
                    container(content)
                        .width(Length::Fill)
                        .padding(self.row_padding)
                        .style(move |theme: &Theme| {
                            let bg = if is_even {
                                row_even_background_override
                                    .unwrap_or_else(|| row_background(theme, true))
                            } else {
                                row_odd_background_override
                                    .unwrap_or_else(|| row_background(theme, false))
                            };
                            let radius = if is_last {
                                border::bottom(6.0)
                            } else {
                                border::radius(0.0)
                            };
                            container::Style {
                                background: Some(bg.into()),
                                border: Border {
                                    radius,
                                    width: 0.0,
                                    color: Color::TRANSPARENT,
                                },
                                ..Default::default()
                            }
                        })
                        .into()
                }
            })
            .collect();

        // Combine header and data rows
        let table_content = column![header_container]
            .push(IcedColumn::with_children(data_rows))
            .width(Length::Fill);

        // Wrap in outer container with subtle border
        container(table_content)
            .width(Length::Fill)
            .style(move |theme: &Theme| {
                container::Style {
                    background: None,
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: border_color_override.unwrap_or_else(|| table_border_color(theme)),
                    },
                    ..Default::default()
                }
            })
            .into()
    }
}

/// Helper to create a text cell with monospace font
pub fn text_cell<'a, M: 'a>(content: impl ToString) -> Element<'a, M> {
    text(content.to_string())
        .font(Font::MONOSPACE)
        .wrapping(iced::widget::text::Wrapping::None)
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.base.text),
            ..Default::default()
        })
        .into()
}

/// Helper to create a wrapping text cell (useful for long hostnames)
pub fn wrap_text_cell<'a, M: 'a>(content: impl ToString) -> Element<'a, M> {
    text(content.to_string())
        .font(Font::MONOSPACE)
        .wrapping(iced::widget::text::Wrapping::Word)
        .style(|theme: &Theme| iced::widget::text::Style {
            color: Some(theme.extended_palette().background.base.text),
            ..Default::default()
        })
        .into()
}

/// Helper to create a colored text cell
pub fn colored_text_cell<'a, M: 'a>(content: impl ToString, color: Color) -> Element<'a, M> {
    text(content.to_string())
        .font(Font::MONOSPACE)
        .wrapping(iced::widget::text::Wrapping::None)
        .color(color)
        .into()
}

/// Helper to create a boolean cell (Yes/No)
pub fn bool_cell<'a, M: 'a>(value: bool) -> Element<'a, M> {
    let (label, color) = if value {
        ("Yes", Color::from_rgb(0.4, 0.8, 0.4))
    } else {
        ("No", Color::from_rgb(0.5, 0.5, 0.5))
    };
    text(label)
        .font(Font::MONOSPACE)
        .wrapping(iced::widget::text::Wrapping::None)
        .color(color)
        .into()
}
