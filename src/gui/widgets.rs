use iced::widget::{container, text, Column, Row};
use iced::{widget::button, Alignment, Background, Border, Color, Shadow, Vector};

/// Build a reusable button style closure from one base color.
///
/// Iced asks for a function (`Theme`, `Status`) -> `Style`, so we capture
/// `base` once and map each runtime button state to a visual variant:
/// - `Active`: unchanged base color.
/// - `Hovered`: brighter version for affordance.
/// - `Pressed`: darker version plus drop shadow for depth.
/// - `Disabled`: same hue, lower alpha to signal inactivity.
pub fn colored_button_style(
    base: iced::Color,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    // Rust `impl Fn(...) -> ...` is like returning a delegate/lambda with a hidden concrete type.
    move |_, status| {
        // `move` captures outer variables by value; `_` means "argument intentionally unused".
        // `match` is Rust's exhaustive switch-expression (must cover all enum variants).
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
                // `..Default::default()` keeps any fields not listed at their default values.
                ..Default::default()
            },
            snap: false,
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

/// Normalize inline icon glyphs to the standard 14px size.
/// `Text<'static>` uses a lifetime; `'static` means the text data can live for the whole program.
pub fn icon(glyph: iced::widget::Text<'static>) -> iced::widget::Text<'static> {
    glyph.size(14.0)
}

/// Render a 16px icon centered in a fixed 28x28 hit target.
pub fn icon_button_content(
    glyph: iced::widget::Text<'static>,
) -> container::Container<'static, crate::gui::Message> {
    container(glyph.size(16.0))
        .width(28)
        .height(28)
        .center_x(28)
        .center_y(28)
}

/// Compact bordered pill for a process name.
///
/// `is_running` drives a simple visual state mapping:
/// - running: brighter text/border/background for emphasis;
/// - stopped: dimmed palette to de-emphasize inactive processes.
pub fn process_pill<Message>(
    name: String,
    is_running: bool,
) -> container::Container<'static, Message> {
    // `<Message>` is a generic type parameter (similar to C# `T`).
    // `(a, b, c)` on the left destructures a tuple returned by the `if` expression.
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

/// Numeric stat badge with value over label.
/// Used in the Status tab header for quick at-a-glance counts.
/// `'a` is a named lifetime tying `label: &'a str` to the returned `Column<'a, ...>`.
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

/// Small filled color swatch followed by a label.
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
