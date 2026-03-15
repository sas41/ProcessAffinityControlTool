use iced::font;
use iced::widget::{Row, Space, button, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Font, Length};

use crate::gui::Message as AppMessage;

// ─── Process editor dialog ────────────────────────────────────────────────────

/// Modal dialog for adding a new process to a group or reassigning an existing one.
pub struct ProcessEditor {
    pub open: bool,

    /// The original process name when editing an existing assignment.
    /// Empty when adding a new process.
    pub editing_process_name: String,

    /// The value of the name text input.
    pub process_name: String,

    /// Currently selected target group.
    pub selected_group: String,

    /// All available group names to choose from.
    pub available_groups: Vec<String>,

    /// Set to `Some((name, group))` when OK is pressed.
    pub result: Option<(String, String)>,

    /// Set when the Remove button is pressed (edit mode only).
    pub remove_requested: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    NameChanged(String),
    GroupSelected(String),
    Ok,
    Cancel,
    Remove,
}

impl ProcessEditor {
    /// Open for adding a new executable to `target_group`.
    pub fn new_for_add(target_group: String, available_groups: Vec<String>) -> Self {
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

    /// Open for editing an existing process assignment.
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

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::NameChanged(s) => self.process_name = s,
            Message::GroupSelected(g) => self.selected_group = g,
            Message::Ok => {
                let name = self.process_name.trim().to_string();
                if !name.is_empty() && !self.selected_group.is_empty() {
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

    pub fn view(&self) -> Element<'_, AppMessage> {
        let is_editing = !self.editing_process_name.is_empty();
        let title = if is_editing {
            "Configure Process"
        } else {
            "Add Process"
        };

        // ── Name row ──────────────────────────────────────────────────────────
        let name_widget: Element<AppMessage> = if is_editing {
            text(self.process_name.clone())
                .size(14)
                .color(Color::from_rgb(0.86, 0.86, 0.86))
                .into()
        } else {
            text_input("exe name…", &self.process_name)
                .on_input(|s| AppMessage::ProcessEditorMessage(Message::NameChanged(s)))
                .width(Length::Fill)
                .into()
        };

        let name_row = row![
            text("Name:").size(13).color(Color::from_rgb(0.7, 0.7, 0.7)),
            name_widget,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // ── Group selection ───────────────────────────────────────────────────
        let group_buttons =
            self.available_groups
                .iter()
                .cloned()
                .fold(Row::new().spacing(6), |row, gname| {
                    let is_selected = gname == self.selected_group;
                    row.push(
                        button(text(gname.clone()).size(12))
                            .on_press(AppMessage::ProcessEditorMessage(Message::GroupSelected(
                                gname,
                            )))
                            .style(move |_, _| button::Style {
                                background: Some(Background::Color(if is_selected {
                                    Color::from_rgb(0.2, 0.4, 0.8)
                                } else {
                                    Color::from_rgb(0.22, 0.22, 0.22)
                                })),
                                text_color: Color::WHITE,
                                border: Border {
                                    radius: 4.0.into(),
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

        // ── Action buttons ────────────────────────────────────────────────────
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

        // ── Dialog container ──────────────────────────────────────────────────
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

        // Full-screen dimmed backdrop with dialog centred inside
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
