use iced::border;
use iced::theme::Style;
use iced::widget::{column, container, text, Column as IcedColumn, Row};
use iced::{Border, Color, Element, Font, Length, Padding, Theme};

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
    rows: Vec<Vec<Element<'a, M>>>,
    spacing: f32,
    row_padding: Padding,
    header_padding: Padding,
}

impl<'a, M: Clone + 'a> Table<'a, M> {
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
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
        }
    }

    /// Add a row of cell elements
    pub fn push_row(mut self, cells: Vec<Element<'a, M>>) -> Self {
        self.rows.push(cells);
        self
    }

    /// Build the table element
    pub fn build(self) -> Element<'a, M> {
        let row_count = self.rows.len();

        // Build header row
        let header_cells: Vec<Element<'a, M>> = self
            .columns
            .iter()
            .map(|col| {
                text(col.title)
                    .font({
                        let mut font = Font::MONOSPACE;
                        font.weight = iced::font::Weight::Bold;
                        font
                    })
                    .style(|theme: &Theme|
                        iced::widget::text::Style {
                            color: Some(theme.palette().warning),
                            ..Default::default()
                        }
                    )
                    .width(col.width)
                    .into()
            })
            .collect();

        let header_row = Row::with_children(header_cells)
            .spacing(self.spacing)
            .padding(self.header_padding)
            .align_y(iced::Alignment::Center);

        // Dark header background
        let header_container: Element<'a, M> = container(header_row)
            .width(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(Color::from_rgb(0.12, 0.12, 0.14).into()),
                border: Border {
                    radius: border::top(6.0),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            })
            .into();

        // Build data rows with alternating colors
        let data_rows: Vec<Element<'a, M>> = self
            .rows
            .into_iter()
            .enumerate()
            .map(|(idx, cells)| {
                let is_even = idx % 2 == 0;
                let is_last = idx == row_count.saturating_sub(1);

                // Build row with cells that have explicit widths
                let row_cells: Vec<Element<'a, M>> = cells
                    .into_iter()
                    .zip(self.columns.iter())
                    .map(|(cell, col)| {
                        container(cell)
                            .width(col.width)
                            .into()
                    })
                    .collect();

                let data_row = Row::with_children(row_cells)
                    .spacing(self.spacing)
                    .padding(self.row_padding)
                    .align_y(iced::Alignment::Center);

                container(data_row)
                    .width(Length::Fill)
                    .style(move |_theme: &Theme| {
                        // Alternating dark backgrounds
                        let bg = if is_even {
                            Color::from_rgb(0.16, 0.16, 0.18)
                        } else {
                            Color::from_rgb(0.13, 0.13, 0.15)
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
            })
            .collect();

        // Combine header and data rows
        let table_content = column![header_container]
            .push(IcedColumn::with_children(data_rows))
            .width(Length::Fill);

        // Wrap in outer container with subtle border
        container(table_content)
            .width(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: None,
                border: Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: Color::from_rgb(0.2, 0.2, 0.22),
                },
                ..Default::default()
            })
            .into()
    }
}

/// Helper to create a text cell with monospace font
pub fn text_cell<'a, M: 'a>(content: impl ToString) -> Element<'a, M> {
    text(content.to_string())
        .font(Font::MONOSPACE)
        .color(Color::from_rgb(0.9, 0.9, 0.9))
        .into()
}

/// Helper to create a colored text cell
pub fn colored_text_cell<'a, M: 'a>(content: impl ToString, color: Color) -> Element<'a, M> {
    text(content.to_string())
        .font(Font::MONOSPACE)
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
        .color(color)
        .into()
}
