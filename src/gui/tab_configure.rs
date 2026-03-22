//! Configure tab UI and local message types.
//!
//! This module owns the "Configure" tab presentation only: group cards at the top,
//! running/custom process pools at the bottom, and drag-and-drop between them.
//! It does not mutate app state directly; controls emit [`Message`] values that are
//! forwarded as `AppMessage::ConfigureMessage(...)` to the app-level update logic.

use iced::font;
use iced::widget::container::Style as ContainerStyle;
use iced::widget::{
    button, checkbox, column, container, row, scrollable, text, text_input, Column, Container, Row,
    Space,
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

    // A group with no affinity and no priority is an emergent blacklist.
    let has_affinity = g.affinity.is_some();
    let has_priority = g.priority.is_some() || g.niceness.is_some();

    if !has_affinity && !has_priority {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::slash_circle_fill(),
            Color::from_rgb(1.0, 0.45, 0.45),
        ));
    } else {
        if has_affinity {
            flags = flags.push(group_flag_icon(
                iced_fonts::bootstrap::cpu_fill(),
                Color::from_rgb(0.55, 0.94, 0.72),
            ));
        }
        if has_priority {
            flags = flags.push(group_flag_icon(
                iced_fonts::bootstrap::stars(),
                Color::from_rgb(0.92, 0.68, 1.0),
            ));
        }
    }

    if g.capture_sub_processes {
        flags = flags.push(group_flag_icon(
            iced_fonts::bootstrap::diagram_two_fill(),
            Color::from_rgb(0.55, 0.85, 1.0),
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
        // Child-inherited processes show the hierarchy icon.
        AssignedProcessSource::ChildInherited => vec![(
            iced_fonts::bootstrap::diagram_two_fill(),
            Color::from_rgb(0.55, 0.85, 1.0),
        )],
    }
}

/// Collects tree-ordered (name, source, prefix) triples for a group's process list.
///
/// Roots are processes not appearing as a child in `parent_to_children`.
/// Children are recursively appended beneath their parent using box-drawing prefixes.
fn collect_tree_items(
    procs: &[(String, AssignedProcessSource)],
    parent_to_children: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<(String, AssignedProcessSource, String)> {
    let child_set: std::collections::HashSet<String> = parent_to_children
        .values()
        .flat_map(|v| v.iter())
        .map(|n| n.to_lowercase())
        .collect();

    let source_map: std::collections::HashMap<String, AssignedProcessSource> =
        procs.iter().map(|(n, s)| (n.to_lowercase(), *s)).collect();

    let mut result: Vec<(String, AssignedProcessSource, String)> = Vec::new();

    for (name, source) in procs {
        if child_set.contains(&name.to_lowercase()) {
            continue; // will appear under its parent
        }
        result.push((name.clone(), *source, String::new()));
        // Collect children as (name, prefix) pairs, then attach their source.
        let mut children: Vec<(String, String)> = Vec::new();
        collect_tree_children(&name.to_lowercase(), "", parent_to_children, &mut children);
        for (child_name, prefix) in children {
            let src = source_map
                .get(&child_name.to_lowercase())
                .copied()
                .unwrap_or(AssignedProcessSource::ChildInherited);
            result.push((child_name, src, prefix));
        }
    }
    result
}

fn collect_tree_children(
    parent_lc: &str,
    indent: &str,
    parent_to_children: &std::collections::HashMap<String, Vec<String>>,
    result: &mut Vec<(String, String)>, // (child_name, prefix)
) {
    let children = match parent_to_children.get(parent_lc) {
        Some(v) => v,
        None => return,
    };
    let n = children.len();
    for (i, child_name) in children.iter().enumerate() {
        let is_last = i == n - 1;
        let connector = if is_last { "└─ " } else { "├─ " };
        let prefix = format!("{}{}", indent, connector);
        result.push((child_name.clone(), prefix));
        let child_indent = format!("{}{}", indent, if is_last { "   " } else { "│  " });
        collect_tree_children(
            &child_name.to_lowercase(),
            &child_indent,
            parent_to_children,
            result,
        );
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
    show_children: bool,
) -> Container<'a, AppMessage> {
    // child_process_parents maps child_name_lower → direct_parent_name_lower.
    // Used to build tree views inside group cards and the custom process panel.
    let child_process_parents = &cache.child_process_parents;

    // Build a global parent_lc → Vec<child_original_name> map for tree rendering.
    // Look up original-case child names from running processes (children are always running).
    let running_original: std::collections::HashMap<String, String> = cache
        .running
        .iter()
        .map(|n| (n.to_lowercase(), n.clone()))
        .collect();
    let mut all_parent_to_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (child_lc, parent_lc) in child_process_parents {
        let child_name = running_original
            .get(child_lc)
            .cloned()
            .unwrap_or_else(|| child_lc.clone());
        all_parent_to_children
            .entry(parent_lc.clone())
            .or_default()
            .push(child_name);
    }
    for v in all_parent_to_children.values_mut() {
        v.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    }
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
        row![
            iced_fonts::bootstrap::diagram_two_fill()
                .size(13)
                .color(if show_children {
                    Color::from_rgb(0.55, 0.85, 1.0)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                }),
            checkbox(show_children)
                .label("Sub-Processes")
                .on_toggle(|_| AppMessage::ToggleShowChildren)
                .text_size(13),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
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

            // For groups with capture_sub_processes, build a parent→children map and
            // render in tree order using box-drawing connectors.
            let pill_list = if g.capture_sub_processes && show_children && !procs.is_empty() {
                // Build parent_name_lc → Vec<child_name_original> for this group's processes.
                let mut parent_to_children: std::collections::HashMap<String, Vec<String>> =
                    std::collections::HashMap::new();
                for (pname, src) in &procs {
                    if *src == AssignedProcessSource::ChildInherited {
                        let child_lc = pname.to_lowercase();
                        if let Some(parent_lc) = child_process_parents.get(&child_lc) {
                            parent_to_children
                                .entry(parent_lc.clone())
                                .or_default()
                                .push(pname.clone());
                        }
                    }
                }
                for children in parent_to_children.values_mut() {
                    children.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
                }

                let tree_items = collect_tree_items(&procs, &parent_to_children);

                tree_items.into_iter().fold(
                    Column::new().spacing(2).padding(Padding {
                        right: 14.0,
                        ..Default::default()
                    }),
                    |col, (pname, source, prefix)| {
                        let is_running = running_set.contains(&pname.to_lowercase());
                        let variants = name_variants(&pname);
                        let mixed = variants.len() > 1;
                        let mut out = col;
                        for is_inaccessible in variants {
                            let drag_msg =
                                AppMessage::ConfigureMessage(Message::DragStarted(pname.clone()));
                            let badges = add_mixed_warning_badge(source_badges(source), mixed);
                            let pill = process_pill_badged_alert(
                                pname.clone(),
                                is_running,
                                badges,
                                is_inaccessible,
                            );
                            let entry: iced::Element<'_, AppMessage> = if prefix.is_empty() {
                                DraggablePill::new(pill, drag_msg).into()
                            } else {
                                row![
                                    text(prefix.clone())
                                        .size(12)
                                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.35)),
                                    DraggablePill::new(pill, drag_msg),
                                ]
                                .align_y(Alignment::Center)
                                .spacing(0)
                                .into()
                            };
                            out = out.push(entry);
                        }
                        out
                    },
                )
            } else {
                // Flat list: exclude ChildInherited entries entirely.
                // When show_children is false, children are hidden.
                // When the group has capture_sub_processes but show_children is true,
                // this branch is never reached; so filtering here is always correct.
                procs
                    .into_iter()
                    .filter(|(_, src)| *src != AssignedProcessSource::ChildInherited)
                    .fold(
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
                                let drag_msg = AppMessage::ConfigureMessage(Message::DragStarted(
                                    pname.clone(),
                                ));
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
                    )
            };

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
        .fold(
            Column::new().spacing(3).padding(Padding {
                right: 14.0,
                ..Default::default()
            }),
            |col, cp| {
                let name = cp.name.clone();
                let name_lc = name.to_lowercase();
                let is_running = running_set.contains(&name_lc);
                let edit_msg = AppMessage::ConfigureMessage(Message::OpenCustomProcessEditor(
                    Some(name.clone()),
                ));

                let variants = name_variants(&name);
                let mixed = variants.len() > 1;
                let mut out = col;
                for is_inaccessible in variants {
                    let drag_msg = AppMessage::ConfigureMessage(Message::DragStarted(name.clone()));
                    let mut cp_badges: Vec<(iced::widget::Text<'static>, Color)> = Vec::new();

                    // Mirror the group card flag icons: blacklist (emergent) → affinity →
                    // priority → capture sub-processes.
                    let has_affinity = cp.affinity.is_some();
                    let has_priority = cp.priority.is_some() || cp.niceness.is_some();

                    if !has_affinity && !has_priority {
                        cp_badges.push((
                            iced_fonts::bootstrap::slash_circle_fill(),
                            Color::from_rgb(1.0, 0.45, 0.45),
                        ));
                    } else {
                        if has_affinity {
                            cp_badges.push((
                                iced_fonts::bootstrap::cpu_fill(),
                                Color::from_rgb(0.55, 0.94, 0.72),
                            ));
                        }
                        if has_priority {
                            cp_badges.push((
                                iced_fonts::bootstrap::stars(),
                                Color::from_rgb(0.92, 0.68, 1.0),
                            ));
                        }
                    }
                    if cp.capture_sub_processes {
                        cp_badges.push((
                            iced_fonts::bootstrap::diagram_two_fill(),
                            Color::from_rgb(0.55, 0.85, 1.0),
                        ));
                    }
                    out = out.push(
                        row![
                            DraggablePill::new(
                                process_pill_badged_alert(
                                    name.clone(),
                                    is_running,
                                    add_mixed_warning_badge(cp_badges, mixed),
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

                // If capture_sub_processes is enabled and the toggle is on, show children.
                if cp.capture_sub_processes && show_children {
                    let mut tree_entries: Vec<(String, String)> = Vec::new(); // (name, prefix)
                    collect_tree_children(&name_lc, "", &all_parent_to_children, &mut tree_entries);
                    for (child_name, prefix) in tree_entries {
                        let child_lc = child_name.to_lowercase();
                        let child_running = running_set.contains(&child_lc);
                        let child_drag =
                            AppMessage::ConfigureMessage(Message::DragStarted(child_name.clone()));
                        let child_pill = process_pill_badged_alert(
                            child_name.clone(),
                            child_running,
                            vec![(
                                iced_fonts::bootstrap::diagram_two_fill(),
                                Color::from_rgb(0.55, 0.85, 1.0),
                            )],
                            false,
                        );
                        out = out.push(
                            row![
                                text(prefix)
                                    .size(12)
                                    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.35)),
                                DraggablePill::new(child_pill, child_drag),
                                // No ellipsis button for child processes.
                            ]
                            .align_y(Alignment::Center)
                            .spacing(0),
                        );
                    }
                }

                out
            },
        );

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
