use crate::gui::Message as AppMessage;
use iced::font;
use iced::widget::{Row, Space, button, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};
/// Modal editor state for creating or editing one process-to-group assignment.
///
/// Modes are derived from `editing_process_name`:
/// - add mode: empty original name, editable name input
/// - edit mode: original name set, name shown read-only, remove action available
pub struct ProcessEditor {
    /// Whether the dialog is currently visible.
    pub open: bool,

    /// Original process name in edit mode; empty in add mode.
    pub editing_process_name: String,

    /// Current value of the name input.
    pub process_name: String,

    /// Currently selected target group.
    pub selected_group: String,

    /// All available group names.
    pub available_groups: Vec<String>,

    /// Output payload set when OK closes with valid values.
    pub result: Option<(String, String)>,

    /// Set when Remove is pressed in edit mode.
    pub remove_requested: bool,
}

/// Local editor events produced by UI controls.
///
/// `view` maps widget events into these messages and wraps them as
/// `AppMessage::ProcessEditorMessage(...)` for the parent update loop.
// Rust note for C#: `#[derive(...)]` auto-implements listed traits for this type.
#[derive(Debug, Clone)]
pub enum Message {
    /// Name input changed.
    // Rust note for C#: `NameChanged(String)` is an enum variant with payload data.
    NameChanged(String),

    /// Group selection changed.
    GroupSelected(String),

    /// Confirm and close.
    Ok,

    /// Cancel and close.
    Cancel,

    /// Request removal in edit mode and close.
    Remove,
}

impl ProcessEditor {
    /// Opens in add mode.
    pub fn new_for_add(target_group: String, available_groups: Vec<String>) -> Self {
        // Rust note for C#: `Self` means the current impl type (`ProcessEditor`).
        Self {
            open: true,
            editing_process_name: String::new(),
            process_name: String::new(),
            selected_group: target_group,
            available_groups,
            result: None,
            remove_requested: false,
        }
    }

    /// Opens in edit mode.
    pub fn new_for_edit(
        process_name: String,
        current_group: String,
        available_groups: Vec<String>,
    ) -> Self {
        Self {
            open: true,
            editing_process_name: process_name.clone(),
            process_name,
            selected_group: current_group,
            available_groups,
            result: None,
            remove_requested: false,
        }
    }

    /// Applies one editor message to local state.
    ///
    /// Closing behavior:
    /// - `Ok`: validates values, stores `result`, then closes
    /// - `Cancel`: closes without output
    /// - `Remove`: flags `remove_requested`, then closes
    pub fn update(&mut self, msg: Message) {
        // Rust note for C#: `match` is a switch-like exhaustive pattern matcher.
        match msg {
            Message::NameChanged(s) => self.process_name = s,
            Message::GroupSelected(g) => self.selected_group = g,
            Message::Ok => {
                let name = self.process_name.trim().to_string();
                if !name.is_empty() && !self.selected_group.is_empty() {
                    // Rust note for C#: `Some(...)` is the present-value case of `Option<T>`.
                    self.result = Some((name, self.selected_group.clone()));
                }
                self.open = false;
            }
            Message::Cancel => self.open = false,
            Message::Remove => {
                self.remove_requested = true;
                self.open = false;
            }
        }
    }

    /// Builds the modal overlay for the current mode.
    // Rust note for C#: `Element<'_, T>` includes a lifetime; `'_` asks compiler to infer it.
    pub fn view(&self) -> Element<'_, AppMessage> {
        let is_editing = !self.editing_process_name.is_empty();
        // Rust note for C#: `if` is an expression here and returns the chosen string.
        let title = if is_editing {
            "Configure Process"
        } else {
            "Add Process"
        };

        // Name field differs by mode: editable in add mode, read-only in edit mode.
        let name_widget: Element<AppMessage> = if is_editing {
            text(self.process_name.as_str())
                .size(14)
                .color(Color::from_rgb(0.86, 0.86, 0.86))
                .into()
        } else {
            text_input("exe name…", &self.process_name)
                // Rust note for C#: `|s| ...` is a closure (lambda); type is inferred.
                .on_input(|s| AppMessage::ProcessEditorMessage(Message::NameChanged(s)))
                .width(Length::Fill)
                .into()
        };

        // Rust note for C#: `row![...]` is a macro call (`!`) that expands at compile time.
        let name_row = row![
            text("Name:").size(13).color(Color::from_rgb(0.7, 0.7, 0.7)),
            name_widget,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Clone `gname` for label and message payload: iced button handlers own data.
        let group_buttons = self
            .available_groups
            .iter()
            .cloned()
            // Rust note for C#: `fold(init, |acc, x| ...)` reduces items into one value.
            .fold(Row::new().spacing(6), |row, gname| {
                let is_selected = gname == self.selected_group;
                row.push(
                    button(text(gname.clone()).size(12))
                        .on_press(AppMessage::ProcessEditorMessage(Message::GroupSelected(
                            gname,
                        )))
                        // Rust note for C#: `move` makes the closure capture by value.
                        .style(move |_, _| button::Style {
                            background: Some(Background::Color(if is_selected {
                                Color::from_rgb(0.2, 0.4, 0.8)
                            } else {
                                Color::from_rgb(0.22, 0.22, 0.22)
                            })),
                            text_color: Color::WHITE,
                            border: Border {
                                radius: 4.0.into(),
                                // Rust note for C#: `..Default::default()` fills remaining fields.
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                )
            });

        let group_section = column![
            text("Assign to group:")
                .size(13)
                .color(Color::from_rgb(0.7, 0.7, 0.7)),
            group_buttons,
        ]
        .spacing(6);

        // Action row: OK/Cancel in both modes, Remove only in edit mode.
        let can_ok = !self.process_name.trim().is_empty() && !self.selected_group.is_empty();

        let ok_btn = button(text("OK").size(13))
            .on_press(AppMessage::ProcessEditorMessage(Message::Ok))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(if can_ok {
                    Color::from_rgb(0.2, 0.4, 0.8)
                } else {
                    Color::from_rgb(0.25, 0.25, 0.25)
                })),
                text_color: Color::WHITE,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let cancel_btn = button(text("Cancel").size(13))
            .on_press(AppMessage::ProcessEditorMessage(Message::Cancel));

        let left_actions = row![ok_btn, cancel_btn].spacing(8);

        // Edit mode adds a right-aligned Remove button.
        let actions_row: Row<AppMessage> = if is_editing {
            row![
                left_actions,
                Space::new().width(Length::Fill),
                button(text("Remove").size(13))
                    .on_press(AppMessage::ProcessEditorMessage(Message::Remove))
                    .style(|_, _| button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.55, 0.1, 0.1))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.8, 0.2, 0.2),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }),
            ]
        } else {
            row![left_actions]
        };

        let dialog_content = column![
            text(title).size(16).font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            }),
            name_row,
            group_section,
            actions_row,
        ]
        .spacing(16)
        .padding(20);

        let dialog =
            container(dialog_content)
                .max_width(440)
                .style(|_| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgb(0.14, 0.14, 0.14))),
                    border: Border {
                        color: Color::from_rgb(0.38, 0.38, 0.38),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                });

        // Full-screen dimmed backdrop with centered dialog.
        container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
                ..Default::default()
            })
            .into()
    }
}
