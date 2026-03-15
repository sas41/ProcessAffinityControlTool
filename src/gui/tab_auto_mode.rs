// Auto Mode tab UI.
use iced::font;

use iced::widget::{
    Column, Container, Space, button, column, container, row, scrollable, text, text_input,
};

use iced::{Alignment, Background, Border, Color, Font, Length, Padding};

use crate::gui::widgets::{colored_button_style, icon};
use crate::gui::{AppCache, Message as AppMessage};

#[derive(Debug, Clone)]
// `derive` auto-implements standard traits (similar to auto-generated C# helpers).
pub enum Message {
    ToggleAutoMode,
    AddLauncher(String),
    RemoveLauncher(String),
    UpdateNewLauncherName(String),
}

// `'a` is a lifetime parameter: returned UI cannot outlive borrowed inputs.
pub fn view<'a>(cache: &'a AppCache, new_launcher_name: &'a str) -> Container<'a, AppMessage> {
    // Toggle color reflects whether auto mode is currently enabled.
    const GREEN: Color = Color::from_rgb(0.13, 0.56, 0.30);
    const GREY: Color = Color::from_rgb(0.22, 0.22, 0.28);

    // Rust `if` is an expression and returns a value (like C# ternary, but block-based).
    let auto_col = if cache.is_auto_mode { GREEN } else { GREY };

    // Toggle sends one message; app state decides enable/disable behavior.
    let auto_btn = button(
        container(
            // `row![...]` is a macro invocation (`!`) that expands to builder code.
            row![
                text("Auto Mode").size(13),
                icon(if cache.is_auto_mode {
                    iced_fonts::bootstrap::toggle_on()
                } else {
                    iced_fonts::bootstrap::toggle_off()
                }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    .on_press(AppMessage::AutoModeMessage(Message::ToggleAutoMode))
    .width(Length::Fixed(140.0))
    .style(colored_button_style(auto_col));

    let description = text(
        "Child processes of registered launchers are automatically routed to the default group.",
    )
    .size(13)
    .color(Color::from_rgb(0.65, 0.65, 0.65));

    // Header combines the mode toggle and short behavior description.
    let header = column![auto_btn, description].spacing(8);

    // Registered launchers are rendered from cache each frame.
    // Right padding keeps remove buttons clear of the scrollbar track.
    // Iterator chain: `iter()` borrows, `cloned()` copies items, `fold` accumulates UI rows.
    let launcher_list = cache.launchers.iter().cloned().fold(
        // `..Default::default()` fills unspecified fields with default values.
        Column::new().spacing(4).padding(Padding {
            right: 14.0,
            ..Default::default()
        }),
        // `|col, name| { ... }` is a closure (roughly a C# lambda with inferred types).
        |col, name| {
            let name_clone = name.clone();

            col.push(
                row![
                    text(name).size(13).color(Color::from_rgb(0.86, 0.86, 0.86)),
                    Space::new().width(Length::Fill),
                    button(text("[x]").size(11))
                        .on_press(AppMessage::AutoModeMessage(Message::RemoveLauncher(
                            name_clone,
                        )))
                        .padding([2, 4])
                        // `_` ignores closure parameters we do not need.
                        .style(|_, _| button::Style {
                            background: None,
                            text_color: Color::from_rgb(0.6, 0.6, 0.6),
                            ..Default::default()
                        }),
                ]
                .align_y(Alignment::Center),
            )
        },
    );

    // Text box keeps local draft name for a launcher to register.
    let new_launcher_input = text_input("launcher name…", new_launcher_name)
        .on_input(|s| AppMessage::AutoModeMessage(Message::UpdateNewLauncherName(s)))
        .width(Length::Fill);

    // Add is active only when input is non-empty; otherwise sends a no-op update.
    let add_btn = button(text("Add").size(13)).on_press(if !new_launcher_name.is_empty() {
        AppMessage::AutoModeMessage(Message::AddLauncher(new_launcher_name.to_string()))
    } else {
        AppMessage::AutoModeMessage(Message::UpdateNewLauncherName(String::new()))
    });

    // Left card: editable launcher allow-list used by auto mode routing.
    let launchers_card = container(
        column![
            text("Launchers").size(15).font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            }),
            scrollable(launcher_list)
                .width(Length::Fill)
                .height(Length::Fill),
            row![new_launcher_input, add_btn]
                .spacing(6)
                .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(10),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.10))),
        border: Border {
            color: Color::from_rgb(0.3, 0.3, 0.3),
            width: 1.0,
            radius: 4.0.into(),
            // `.into()` performs explicit type conversion via trait-based conversion rules.
        },
        ..Default::default()
    });

    // Detected names are session-only observations rendered read-only.
    let detected_list = cache.detections.iter().cloned().fold(
        Column::new().spacing(4).padding(Padding {
            right: 14.0,
            ..Default::default()
        }),
        |col, name| col.push(text(name).size(13).color(Color::from_rgb(0.65, 0.65, 0.65))),
    );

    // Right card: scrollable view of detections collected this run.
    let detected_card = container(
        column![
            text("Detected this session").size(15).font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            }),
            scrollable(detected_list)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(8)
        .padding(10),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.10))),
        border: Border {
            color: Color::from_rgb(0.3, 0.3, 0.3),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    // Cards are shown side by side and share remaining vertical space.
    let two_cols = row![launchers_card, detected_card]
        .spacing(10)
        .height(Length::Fill);

    // Tab layout: header first, then the two list cards.
    let content = column![header, two_cols]
        .spacing(12)
        .padding(10)
        .height(Length::Fill);

    // Return a single root container for the tab host.
    container(content)
}
