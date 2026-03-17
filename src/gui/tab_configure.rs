//! Configure tab UI and local message types.
//!
//! This module owns the "Configure" tab presentation only: group cards at the top,
//! running/custom process pools at the bottom, and drag-and-drop between them.
//! It does not mutate app state directly; controls emit [`Message`] values that are
//! forwarded as `AppMessage::ConfigureMessage(...)` to the app-level update logic.

use iced::font;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, Container, Row, Space,
};
use iced::{Alignment, Background, Border, Color, Font, Length, Padding};

use crate::core::pact_instance::AssignedProcessSource;
use crate::core::process_config::ProcessGroup;
use crate::gui::draggable_pill::DraggablePill;
use crate::gui::drop_zone::DropZone;
use crate::gui::widgets::process_pill_badged_alert;
use crate::gui::{AppCache, Message as AppMessage};

/// Maximum number of group cards in each grid row.
const CARDS_PER_ROW: usize = 4;

fn group_flag_icon(
    glyph: iced::widget::Text<'static>,
    color: Color,
) -> Container<'static, AppMessage> {
    container(glyph.size(12).color(color))
        .width(16)
        .height(16)
        .center_x(16)
        .center_y(16)
}

fn group_flag_row(g: &ProcessGroup) -> Row<'static, AppMessage> {
    let mut flags = Row::new().spacing(4).align_y(Alignment::Center);

    if g.is_default {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::award_fill(),
            Color::from_rgb(0.43, 0.73, 1.0),
        ));
    }

    if g.is_auto_mode_group {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::lightning_charge_fill(),
            Color::from_rgb(0.99, 0.85, 0.24),
        ));
    }

    if g.is_blacklist {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::slash_circle_fill(),
            Color::from_rgb(1.0, 0.45, 0.45),
        ));
    }

    if g.affinity.is_some() {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::cpu_fill(),
            Color::from_rgb(0.55, 0.94, 0.72),
        ));
    }

    if g.priority.is_some() {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::stars(),
            Color::from_rgb(0.92, 0.68, 1.0),
        ));
    }

    flags
}

fn small_icon_button_content(glyph: iced::widget::Text<'static>) -> Container<'static, AppMessage> {
    container(glyph.size(13.0))
        .width(20)
        .height(20)
        .center_x(20)
        .center_y(20)
}

fn source_badges(source: AssignedProcessSource) -> Vec<(iced::widget::Text<'static>, Color)> {
    match source {
        AssignedProcessSource::Explicit => Vec::new(),
        AssignedProcessSource::Default => vec![(
            iced_fonts::bootstrap::award_fill(),
            Color::from_rgb(0.43, 0.73, 1.0),
        )],
        AssignedProcessSource::AutoMode => vec![(
            iced_fonts::bootstrap::lightning_charge_fill(),
            Color::from_rgb(0.99, 0.85, 0.24),
        )],
    }
}

fn add_mixed_warning_badge(
    mut badges: Vec<(iced::widget::Text<'static>, Color)>,
    show: bool,
) -> Vec<(iced::widget::Text<'static>, Color)> {
    if show {
        badges.push((
            iced_fonts::bootstrap::exclamation_triangle_fill(),
            Color::from_rgb(0.95, 0.84, 0.20),
        ));
    }
    badges
}

/// User intents emitted by controls inside the Configure tab.
///
/// These messages are local to this tab. The view wraps each one in
/// `AppMessage::ConfigureMessage(...)` so the parent update loop can route it.
#[derive(Debug, Clone)]
pub enum Message {
    /// Assign a process name to a group name.
    /// Rust note for C#: this is a tuple-style enum case (payload values, no named fields).
    AssignProcess(String, String),
    /// Open group editor (`None` new, `Some` edit).
    /// `Option<T>` is Rust's nullable-like sum type (`Some(value)` or `None`).
    OpenGroupEditor(Option<String>),
    /// Open process editor (`Option<group_name>`, `process_name`).
    OpenProcessEditor(Option<String>, String),
    /// Open process editor with no preselected group.
    OpenProcessEditorGlobal,
    /// Update running-process filter text.
    UpdateProcessFilter(String),
    /// Open custom-process editor (`None` new, `Some` edit).
    OpenCustomProcessEditor(Option<String>),
    /// Drag operation started for this process name.
    DragStarted(String),
    /// Process dropped on group (`process_name`, `group_name`).
    DropOnGroup(String, String),
    /// Process dropped on custom panel.
    DropOnCustom(String),
    /// Process dropped on running panel.
    DropOnRunning(String),
}

/// Shared style for cards in this tab.
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

/// Builds the Configure tab.
///
/// Layout overview:
/// - Top action row for add/help actions.
/// - Scrollable group grid (assignment targets).
/// - Bottom row split into Running (left) and Custom (right) process panels.
///
/// Vertical sizing uses a 3:2 split (`FillPortion(3)` above `FillPortion(2)`),
/// so the group editor area stays dominant while keeping both process pools visible.
pub fn view<'a>(
    // `'a` is a lifetime parameter: returned UI can borrow data for at most this scope.
    cache: &'a AppCache,
    // `&T` is a shared borrow (roughly a read-only reference).
    dragging: Option<&'a str>,
    process_filter: &'a str,
    search_pulse_phase: f32,
) -> Container<'a, AppMessage> {
    // Normalize once for case-insensitive matching.
    let running_set: std::collections::HashSet<String> =
        // `::` is Rust's path separator (similar to C# namespace/type qualification).
        cache.running.iter().map(|s| s.to_lowercase()).collect();
    let mut running_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for name in &cache.running {
        *running_counts.entry(name.to_lowercase()).or_default() += 1;
    }

    let mut inaccessible_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for name in &cache.protected_names {
        *inaccessible_counts.entry(name.to_lowercase()).or_default() += 1;
    }

    let name_variants = |name: &str| -> Vec<bool> {
        let key = name.to_lowercase();
        let total = running_counts.get(&key).copied().unwrap_or(0);
        let inaccessible = inaccessible_counts.get(&key).copied().unwrap_or(0);

        if total == 0 {
            return vec![false];
        }

        let mut out = Vec::new();
        if total > inaccessible {
            out.push(false);
        }
        if inaccessible > 0 {
            out.push(true);
        }
        if out.is_empty() {
            out.push(false);
        }
        out
    };

    let assigned_names: std::collections::HashSet<String> = cache
        .assigned
        .iter()
        .map(|a| a.name.clone())
        .chain(cache.custom_processes.iter().map(|cp| cp.name.clone()))
        .collect();

    let mut by_group: std::collections::HashMap<String, Vec<(String, AssignedProcessSource)>> =
        std::collections::HashMap::new();

    for assigned in &cache.assigned {
        by_group
            .entry(assigned.group.to_lowercase())
            .or_default()
            .push((assigned.name.clone(), assigned.source));
    }

    let dropping = dragging.map(|s| s.to_string());
    let has_groups = !cache.groups.is_empty();
    let filter_lower = process_filter.to_lowercase();
    let pulse = if process_filter.is_empty() {
        0.0
    } else {
        (search_pulse_phase.sin() * 0.5) + 0.5
    };
    let search_bg = Color::from_rgb(
        0.14 + (0.10 * pulse),
        0.18 + (0.10 * pulse),
        0.24 + (0.12 * pulse),
    );

    let action_bar = row![
        // `row![...]` is a macro call (`!`) that expands into widget-building code.
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
        text_input("Search processes...", process_filter)
            .on_input(|s| AppMessage::ConfigureMessage(Message::UpdateProcessFilter(s)))
            .width(Length::Fixed(220.0))
            .size(13)
            .style(move |theme, status| {
                let mut style = iced::widget::text_input::default(theme, status);
                if !process_filter.is_empty() {
                    style.background = Background::Color(search_bg);
                }
                style
            }),
        Space::new().width(Length::Fill),
        button(text("Help").size(13))
            .on_press(AppMessage::ShowGroupsHelp)
            .padding([5, 10]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let all_cards: Vec<iced::Element<'a, AppMessage>> = cache
        .groups
        .iter()
        .map(|g| {
            let gname = g.name.clone();
            let gname_lower = gname.to_lowercase();

            let all_group_procs: Vec<(String, AssignedProcessSource)> =
                by_group.get(&gname_lower).cloned().unwrap_or_default();
            let group_name_match = !filter_lower.is_empty() && gname_lower.contains(&filter_lower);
            let procs: Vec<(String, AssignedProcessSource)> = all_group_procs
                .iter()
                .filter(|(p, _)| {
                    filter_lower.is_empty()
                        || group_name_match
                        || p.to_lowercase().contains(&filter_lower)
                })
                .cloned()
                .collect();

            let header = row![
                row![
                    text(gname.clone()).size(14).font(Font {
                        weight: font::Weight::Bold,
                        ..Default::default()
                    }),
                    group_flag_row(g),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                Space::new().width(Length::Fill),
                button(small_icon_button_content(
                    iced_fonts::bootstrap::three_dots(),
                ))
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
                |col, (pname, source)| {
                    let is_running = running_set.contains(&pname.to_lowercase());
                    let variants = name_variants(&pname);
                    let mixed = variants.len() > 1;
                    let mut out = col;
                    for is_inaccessible in variants {
                        let drag_msg =
                            AppMessage::ConfigureMessage(Message::DragStarted(pname.clone()));
                        let base_badges = if source == AssignedProcessSource::Explicit {
                            Vec::new()
                        } else {
                            source_badges(source)
                        };
                        let badges = add_mixed_warning_badge(base_badges, mixed);
                        let pill = process_pill_badged_alert(
                            pname.clone(),
                            is_running,
                            badges,
                            is_inaccessible,
                        );
                        out = out.push(DraggablePill::new(pill, drag_msg));
                    }
                    out
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
            // `move` captures referenced locals by value; `|_|` is a closure ignoring its input.
            .style(move |_| card_style());

            let gname_drop = gname.clone();
            DropZone::new(card, dropping.clone(), move |proc_name| {
                AppMessage::ConfigureMessage(Message::DropOnGroup(proc_name, gname_drop.clone()))
            })
            .into()
        })
        .collect();

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

    let mut seen_unassigned = std::collections::HashSet::new();
    let unassigned: Vec<String> = cache
        .running
        .iter()
        .filter(|n| !assigned_names.contains(*n))
        .filter(|n| filter_lower.is_empty() || n.to_lowercase().contains(&filter_lower))
        .filter(|n| seen_unassigned.insert(n.to_lowercase()))
        .cloned()
        .collect();

    let running_card = container(
        column![
            row![text("Running Processes").size(15).font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            }),]
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
                    let variants = name_variants(&pname);
                    let mixed = variants.len() > 1;
                    let mut out = col;
                    for is_inaccessible in variants {
                        let msg = AppMessage::ConfigureMessage(Message::DragStarted(pname.clone()));
                        out = out.push(DraggablePill::new(
                            process_pill_badged_alert(
                                pname.clone(),
                                true,
                                add_mixed_warning_badge(Vec::new(), mixed),
                                is_inaccessible,
                            ),
                            msg,
                        ));
                    }
                    out
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
        button(small_icon_button_content(iced_fonts::bootstrap::plus()))
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
        .filter(|cp| filter_lower.is_empty() || cp.name.to_lowercase().contains(&filter_lower))
        .fold(Column::new().spacing(3), |col, cp| {
            let name = cp.name.clone();
            let is_running = running_set.contains(&name.to_lowercase());
            let edit_msg =
                AppMessage::ConfigureMessage(Message::OpenCustomProcessEditor(Some(name.clone())));

            let variants = name_variants(&name);
            let mixed = variants.len() > 1;
            let mut out = col;
            for is_inaccessible in variants {
                let drag_msg = AppMessage::ConfigureMessage(Message::DragStarted(name.clone()));
                out = out.push(
                    row![
                        DraggablePill::new(
                            process_pill_badged_alert(
                                name.clone(),
                                is_running,
                                add_mixed_warning_badge(Vec::new(), mixed),
                                is_inaccessible,
                            ),
                            drag_msg,
                        ),
                        Space::new().width(Length::Fill),
                        button(small_icon_button_content(
                            iced_fonts::bootstrap::three_dots(),
                        ))
                        .on_press(edit_msg.clone())
                        .padding(0),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(4),
                );
            }
            out
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

    // Keep the lower process panels at 2/5 of vertical space.
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
