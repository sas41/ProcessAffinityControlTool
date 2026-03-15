use iced::font;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::{
    Column, Container, Row, Space, button, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Font, Length, Padding};

use crate::gui::draggable_pill::DraggablePill;
use crate::gui::drop_zone::DropZone;
use crate::gui::widgets::{icon_button_content, process_pill};
use crate::gui::{AppCache, Message as AppMessage};

const CARDS_PER_ROW: usize = 4;

#[derive(Debug, Clone)]
pub enum Message {
    AssignProcess(String, String),
    OpenGroupEditor(Option<String>),
    /// Open ProcessEditor for a process already in a group (edit/reassign).
    OpenProcessEditor(Option<String>, String),
    /// Open ProcessEditor for a new process with no pre-selected group.
    OpenProcessEditorGlobal,
    UpdateProcessFilter(String),
    /// Open the custom process editor. `None` = create new, `Some(name)` = edit existing.
    OpenCustomProcessEditor(Option<String>),
    /// A running-process pill started being dragged (deadband exceeded).
    DragStarted(String),
    /// Pill dropped onto a group card.
    DropOnGroup(String, String),
    /// Pill dropped onto the Custom Processes panel — opens the editor with name pre-filled.
    DropOnCustom(String),
    /// Pill dropped onto Running Processes — removes any group/custom assignment.
    DropOnRunning(String),
}

fn card_style() -> ContainerStyle {
    ContainerStyle {
        background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.12))),
        border: Border {
            color: Color::from_rgb(0.35, 0.35, 0.35),
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    }
}

pub fn view<'a>(
    cache: &'a AppCache,
    dragging: Option<&'a str>,
    process_filter: &'a str,
) -> Container<'a, AppMessage> {
    let running_set: std::collections::HashSet<String> =
        cache.running.iter().map(|s| s.to_lowercase()).collect();
    let assigned_names: std::collections::HashSet<String> = cache
        .assigned
        .iter()
        .map(|(n, _)| n.clone())
        .chain(cache.custom_processes.iter().map(|cp| cp.name.clone()))
        .collect();

    let mut by_group: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (proc, grp) in &cache.assigned {
        by_group
            .entry(grp.to_lowercase())
            .or_default()
            .push(proc.clone());
    }

    let dropping = dragging.map(|s| s.to_string());

    // ── Action bar ────────────────────────────────────────────────────────────
    let has_groups = !cache.groups.is_empty();

    let action_bar = row![
        button(text("Add Group").size(13))
            .on_press(AppMessage::ConfigureMessage(Message::OpenGroupEditor(None)))
            .padding([5, 10]),
        {
            let btn = button(text("Add Process").size(13)).padding([5, 10]);
            if has_groups {
                btn.on_press(AppMessage::ConfigureMessage(
                    Message::OpenProcessEditorGlobal,
                ))
            } else {
                btn
            }
        },
        Space::new().width(Length::Fill),
        button(text("Help").size(13))
            .on_press(AppMessage::ShowGroupsHelp)
            .padding([5, 10]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    // ── Group cards ───────────────────────────────────────────────────────────
    let all_cards: Vec<iced::Element<'a, AppMessage>> = cache
        .groups
        .iter()
        .map(|g| {
            let gname = g.name.clone();
            let gname_lower = gname.to_lowercase();

            let procs: Vec<String> = by_group.get(&gname_lower).cloned().unwrap_or_default();

            let header = row![
                text(gname.clone()).size(14).font(Font {
                    weight: font::Weight::Bold,
                    ..Default::default()
                }),
                Space::new().width(Length::Fill),
                button(icon_button_content(iced_fonts::bootstrap::three_dots()))
                    .on_press(AppMessage::ConfigureMessage(Message::OpenGroupEditor(
                        Some(gname.clone(),)
                    )))
                    .padding(0),
            ]
            .align_y(Alignment::Center)
            .spacing(4)
            .width(Length::Fill);

            let pill_list = procs.into_iter().fold(
                Column::new().spacing(2).padding(Padding {
                    right: 14.0,
                    ..Default::default()
                }),
                |col, pname| {
                    let is_running = running_set.contains(&pname.to_lowercase());
                    let drag_msg =
                        AppMessage::ConfigureMessage(Message::DragStarted(pname.clone()));
                    col.push(DraggablePill::new(
                        process_pill(pname, is_running),
                        drag_msg,
                    ))
                },
            );

            let card = container(
                column![
                    header,
                    container(Space::new().height(1.0))
                        .width(Length::Fill)
                        .style(|_| ContainerStyle {
                            background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                            ..Default::default()
                        }),
                    scrollable(pill_list)
                        .width(Length::Fill)
                        .height(Length::Fixed(120.0)),
                ]
                .spacing(6)
                .padding(8),
            )
            .width(Length::Fill)
            .style(move |_| card_style());

            let gname_drop = gname.clone();
            DropZone::new(card, dropping.clone(), move |proc_name| {
                AppMessage::ConfigureMessage(Message::DropOnGroup(proc_name, gname_drop.clone()))
            })
            .into()
        })
        .collect();

    // Lay cards into rows of at most CARDS_PER_ROW; each card fills its share.
    let mut grid = Column::new().spacing(8);
    let mut iter = all_cards.into_iter();
    loop {
        let mut current_row = Row::new().spacing(8);
        let mut count = 0;
        while count < CARDS_PER_ROW {
            if let Some(card) = iter.next() {
                current_row = current_row.push(card);
                count += 1;
            } else {
                break;
            }
        }
        if count == 0 {
            break;
        }
        grid = grid.push(current_row.width(Length::Fill));
    }

    // ── Bottom panels ─────────────────────────────────────────────────────────
    let filter_lower = process_filter.to_lowercase();
    let unassigned: Vec<String> = cache
        .running
        .iter()
        .filter(|n| !assigned_names.contains(*n))
        .filter(|n| filter_lower.is_empty() || n.to_lowercase().contains(&filter_lower))
        .cloned()
        .collect();

    let running_card = container(
        column![
            row![
                text("Running Processes").size(15).font(Font {
                    weight: font::Weight::Bold,
                    ..Default::default()
                }),
                Space::new().width(Length::Fill),
                text_input("Filter…", process_filter)
                    .on_input(|s| AppMessage::ConfigureMessage(Message::UpdateProcessFilter(s)))
                    .width(Length::Fixed(160.0))
                    .size(13),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            container(Space::new().height(1.0))
                .width(Length::Fill)
                .style(|_| ContainerStyle {
                    background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                    ..Default::default()
                }),
            scrollable(unassigned.into_iter().fold(
                Column::new().spacing(2).padding(Padding {
                    right: 14.0,
                    ..Default::default()
                }),
                |col, pname| {
                    let msg = AppMessage::ConfigureMessage(Message::DragStarted(pname.clone()));
                    col.push(DraggablePill::new(process_pill(pname, true), msg))
                }
            ))
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(6)
        .padding(8),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| card_style());

    let custom_header = row![
        text("Custom Processes").size(15).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
        Space::new().width(Length::Fill),
        button(icon_button_content(iced_fonts::bootstrap::plus()))
            .on_press(AppMessage::ConfigureMessage(
                Message::OpenCustomProcessEditor(None),
            ))
            .padding(0),
    ]
    .align_y(Alignment::Center)
    .spacing(6);

    let custom_list = cache
        .custom_processes
        .iter()
        .fold(Column::new().spacing(3), |col, cp| {
            let name = cp.name.clone();
            let drag_msg = AppMessage::ConfigureMessage(Message::DragStarted(name.clone()));
            let edit_msg =
                AppMessage::ConfigureMessage(Message::OpenCustomProcessEditor(Some(name.clone())));
            col.push(
                row![
                    DraggablePill::new(process_pill(name, false), drag_msg),
                    Space::new().width(Length::Fill),
                    button(icon_button_content(iced_fonts::bootstrap::three_dots()))
                        .on_press(edit_msg)
                        .padding(0),
                ]
                .align_y(Alignment::Center)
                .spacing(4),
            )
        });

    let custom_card = container(
        column![
            custom_header,
            container(Space::new().height(1.0))
                .width(Length::Fill)
                .style(|_| ContainerStyle {
                    background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                    ..Default::default()
                }),
            scrollable(custom_list)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(6)
        .padding(8),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| card_style());

    let custom_drop_zone = DropZone::new(custom_card, dropping.clone(), |proc_name| {
        AppMessage::ConfigureMessage(Message::DropOnCustom(proc_name))
    });

    let running_drop_zone = DropZone::new(running_card, dropping.clone(), |proc_name| {
        AppMessage::ConfigureMessage(Message::DropOnRunning(proc_name))
    });

    // Minimum height enforced by wrapping in a container; panels grow beyond this.
    let bottom_row = container(
        row![running_drop_zone, custom_drop_zone]
            .spacing(10)
            .height(Length::Fill),
    )
    .height(Length::FillPortion(2));

    let content = column![
        action_bar,
        scrollable(grid)
            .width(Length::Fill)
            .height(Length::FillPortion(3)),
        bottom_row,
    ]
    .spacing(10)
    .padding(10)
    .height(Length::Fill);

    container(content)
}
