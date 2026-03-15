use iced::widget::{container, text, Column, Row};
use iced::{widget::button, Shadow, Vector};

/// Coloured button style with distinct hover (brightens) and press (darkens + shadow) states.
/// Pass the base background colour; text is always white.
pub fn colored_button_style(
    base: iced::Color,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    use iced::{Background, Border};
    move |_, status| {
        let bg = match status {
            button::Status::Active => base,
            button::Status::Hovered => iced::Color::from_rgba(
                (base.r * 1.25).min(1.0),
                (base.g * 1.25).min(1.0),
                (base.b * 1.25).min(1.0),
                1.0,
            ),
            button::Status::Pressed => {
                iced::Color::from_rgba(base.r * 0.60, base.g * 0.60, base.b * 0.60, 1.0)
            }
            button::Status::Disabled => iced::Color::from_rgba(base.r, base.g, base.b, 0.4),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: iced::Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            shadow: if matches!(status, button::Status::Pressed) {
                Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                    offset: Vector::new(0.0, 2.0),
                    blur_radius: 4.0,
                }
            } else {
                Shadow::default()
            },
        }
    }
}

/// Render a Bootstrap icon glyph as a plain text widget at the given size.
pub fn icon(glyph: iced_fonts::Bootstrap) -> iced::widget::Text<'static> {
    text(char::from(glyph).to_string())
        .font(iced_fonts::BOOTSTRAP_FONT)
        .size(14.0)
}

/// Render a Bootstrap icon glyph at the given size, centered in a fixed square.
pub fn icon_button_content(
    glyph: iced_fonts::Bootstrap,
) -> container::Container<'static, crate::gui::Message> {
    container(
        text(char::from(glyph).to_string())
            .font(iced_fonts::BOOTSTRAP_FONT)
            .size(16.0),
    )
    .width(28)
    .height(28)
    .center_x(28)
    .center_y(28)
}
use iced::{Alignment, Background, Border, Color};

/// Compact pill showing a process name with a bordered pill shape.
/// Dimmed when the process is not currently running.
pub fn process_pill<Message>(
    name: String,
    is_running: bool,
) -> container::Container<'static, Message> {
    let (text_col, border_col, bg_col) = if is_running {
        (
            Color::from_rgb(0.86, 0.86, 0.86),
            Color::from_rgb(0.45, 0.45, 0.45),
            Color::from_rgba(0.22, 0.22, 0.22, 1.0),
        )
    } else {
        (
            Color::from_rgb(0.41, 0.41, 0.41),
            Color::from_rgb(0.30, 0.30, 0.30),
            Color::from_rgba(0.16, 0.16, 0.16, 1.0),
        )
    };

    container(text(name).size(12).color(text_col))
        .padding([3, 8])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(bg_col)),
            border: Border {
                color: border_col,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
}

/// Numeric stat badge: large coloured number on top, small label below.
/// Used in the Status tab header row.
pub fn stat_badge<'a, Message>(label: &'a str, value: usize, col: Color) -> Column<'a, Message> {
    Column::new()
        .align_x(Alignment::Center)
        .spacing(2)
        .push(
            text(value.to_string())
                .size(24)
                .color(col)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
        )
        .push(text(label).size(12).color(Color::from_rgb(0.7, 0.7, 0.7)))
}

/// Small filled colour square followed by a text label.
/// Used in the topology diagram legend.
pub fn color_swatch<Message: 'static>(col: Color, text_str: String) -> Row<'static, Message> {
    Row::new()
        .align_y(Alignment::Center)
        .spacing(6)
        .push(
            container("")
                .width(12)
                .height(12)
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(col)),
                    ..Default::default()
                }),
        )
        .push(text(text_str).size(12))
}
