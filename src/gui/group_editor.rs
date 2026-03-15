use iced::widget::{
    button, checkbox, column, container, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::core::process_config::{AffinityConfig, ProcessGroup};
use crate::gui::priority::{index_to_priority, priority_to_index, PRIORITY_LABELS};
use crate::gui::topology_diagram::draw_core_selector;
use crate::gui::Message as AppMessage;

fn group_core_toggle_message(index: usize, checked: bool) -> AppMessage {
    AppMessage::GroupEditorMessage(Message::CoreToggled(index, checked))
}

pub trait ProcessGroupExt {
    // `trait` is Rust's interface contract (like a C# interface).
    /// Local helper used by the editor when creating a new group draft.
    fn default_new() -> ProcessGroup;
}

impl ProcessGroupExt for ProcessGroup {
    fn default_new() -> ProcessGroup {
        ProcessGroup {
            name: String::new(),
            affinity: None,
            priority: None,
            is_default: false,
            is_blacklist: false,
            is_auto_mode_group: false,
        }
    }
}

/// Modal editor state for creating/updating one `ProcessGroup`.
///
/// Responsibility split:
/// - `view()` renders controls from this state and emits editor `Message`s.
/// - `update()` applies those messages and records the final intent (`result`/delete/cancel).
///
/// The parent screen owns persistence; this type only manages in-dialog edits.
pub struct GroupEditor {
    pub open: bool,

    pub editing_name: String,

    pub name: String,
    pub is_blacklist: bool,
    pub is_default: bool,
    pub is_auto_mode_group: bool,

    pub affinity_enabled: bool,
    // One checkbox entry per logical core index.
    pub core_checks: Vec<bool>,

    pub priority_enabled: bool,
    pub priority_index: usize,

    pub result: Option<ProcessGroup>,

    pub delete_requested: bool,
}

#[derive(Debug, Clone)]
/// UI events produced by `GroupEditor::view()` and consumed by `GroupEditor::update()`.
pub enum Message {
    NameChanged(String),
    ToggleDefault,
    ToggleBlacklist,
    ToggleAutoModeGroup,
    ToggleAffinity,
    TogglePriority,
    PriorityChanged(usize),
    CoreToggled(usize, bool),
    SelectAllCores,
    SelectNoneCores,
    SelectPCores,
    SelectECores,
    SelectCCD(usize),
    Ok,
    Cancel,
    Delete,
}

impl GroupEditor {
    fn selector_len(num_cores: usize) -> usize {
        let topo = crate::core::topology::get_topology();
        let max_logical = topo
            .topology_view()
            .groups
            .iter()
            .flat_map(|g| g.physical_cores.iter())
            .flat_map(|c| c.threads.iter())
            .map(|t| t.logical_index)
            .max()
            .map(|i| i + 1)
            .unwrap_or(0);
        num_cores.max(max_logical)
    }

    /// Builds an editor from an existing group or a new empty draft.
    ///
    /// Selection behavior:
    /// - Existing affinity selects only listed cores.
    /// - No affinity defaults to all cores selected in the UI.
    pub fn new(group: Option<&ProcessGroup>, num_cores: usize) -> Self {
        // `Option<T>` is Rust's nullable wrapper: `Some(value)` or `None`.
        let g = group.cloned().unwrap_or_else(ProcessGroup::default_new);
        // `unwrap_or_else(f)` calls `f` only when the option is `None` (lazy fallback).
        let editing_name = g.name.clone();
        let affinity_enabled = g.affinity.is_some();
        let priority_enabled = g.priority.is_some();

        // `vec![x; n]` builds a vector with `n` repeated copies of `x`.
        let selector_len = Self::selector_len(num_cores);
        let mut core_checks = vec![false; selector_len];
        // `if let` pattern-matches only one case and skips the rest.
        if let Some(ref aff) = g.affinity {
            // `ref` borrows in a pattern (avoid moving out of `g.affinity`).
            for &c in &aff.core_list {
                // `&c` destructures a reference and copies the inner `usize`.
                if c < num_cores {
                    core_checks[c] = true;
                }
            }
        } else {
            // `|c| *c = true` is closure syntax; `*c` dereferences `&mut bool`.
            core_checks.iter_mut().for_each(|c| *c = true);
        }

        Self {
            open: true,
            editing_name,
            name: g.name,
            is_blacklist: g.is_blacklist,
            is_default: g.is_default,
            is_auto_mode_group: g.is_auto_mode_group,
            affinity_enabled,
            core_checks,
            priority_enabled,
            priority_index: g.priority.as_ref().map_or(2, priority_to_index),
            result: None,
            delete_requested: false,
        }
    }

    /// Applies one editor event to local dialog state.
    ///
    /// Message flow (high level):
    /// 1) `view()` emits `Message` wrapped as `AppMessage::GroupEditorMessage`.
    /// 2) Parent routes it here.
    /// 3) `Ok` stores `result`, `Cancel` just closes, `Delete` sets `delete_requested`.
    pub fn update(&mut self, message: Message, _num_cores: usize) {
        // `match` is a value/enum switch with exhaustive pattern handling.
        match message {
            Message::NameChanged(name) => self.name = name,
            Message::ToggleDefault => {
                self.is_default = !self.is_default;
                if self.is_default {
                    self.is_blacklist = false;
                }
            }
            Message::ToggleBlacklist => {
                self.is_blacklist = !self.is_blacklist;
                if self.is_blacklist {
                    self.is_default = false;
                    self.is_auto_mode_group = false;
                }
            }
            Message::ToggleAutoModeGroup => {
                self.is_auto_mode_group = !self.is_auto_mode_group;
                if self.is_auto_mode_group {
                    self.is_blacklist = false;
                }
            }
            Message::ToggleAffinity => self.affinity_enabled = !self.affinity_enabled,
            Message::TogglePriority => self.priority_enabled = !self.priority_enabled,
            Message::PriorityChanged(index) => self.priority_index = index,
            Message::CoreToggled(index, checked) => {
                if index < self.core_checks.len() {
                    self.core_checks[index] = checked;
                }
            }
            Message::SelectAllCores => {
                self.core_checks.iter_mut().for_each(|c| *c = true);
            }
            Message::SelectNoneCores => {
                self.core_checks.iter_mut().for_each(|c| *c = false);
            }
            Message::SelectPCores => {
                let topo = crate::core::topology::get_topology();
                let pc = topo.get_performance_cores();
                self.core_checks.iter_mut().for_each(|c| *c = false);
                for i in pc {
                    if i < self.core_checks.len() {
                        self.core_checks[i] = true;
                    }
                }
            }
            Message::SelectECores => {
                let topo = crate::core::topology::get_topology();
                let ec = topo.get_efficiency_cores();
                self.core_checks.iter_mut().for_each(|c| *c = false);
                for i in ec {
                    if i < self.core_checks.len() {
                        self.core_checks[i] = true;
                    }
                }
            }
            Message::SelectCCD(index) => {
                let topo = crate::core::topology::get_topology();
                let ccds = topo.get_ccd_groups();
                if let Some(grp) = ccds.get(index) {
                    // Quick-select actions replace the previous selection, not merge with it.
                    self.core_checks.iter_mut().for_each(|c| *c = false);
                    for &idx in grp {
                        if idx < self.core_checks.len() {
                            self.core_checks[idx] = true;
                        }
                    }
                }
            }
            Message::Ok => {
                // Blacklist groups are match-only filters; they do not apply affinity/priority.
                let affinity = if self.affinity_enabled && !self.is_blacklist {
                    let cores: Vec<usize> = self
                        .core_checks
                        .iter()
                        .enumerate()
                        // `filter_map` keeps entries by returning `Some`, drops with `None`.
                        .filter_map(|(i, &c)| if c { Some(i) } else { None })
                        .collect();
                    Some(AffinityConfig::new(cores))
                } else {
                    None
                };
                let priority = if self.priority_enabled && !self.is_blacklist {
                    Some(index_to_priority(self.priority_index))
                } else {
                    None
                };
                self.result = Some(ProcessGroup {
                    name: self.name.trim().to_string(),
                    affinity,
                    priority,
                    is_default: self.is_default,
                    is_blacklist: self.is_blacklist,
                    is_auto_mode_group: self.is_auto_mode_group,
                });
                self.open = false;
            }
            Message::Cancel => {
                self.open = false;
            }
            Message::Delete => {
                self.delete_requested = true;
                self.open = false;
            }
        }
    }

    pub fn view(&self, _num_cores: usize, topology_group_repeat: usize) -> Element<'_, AppMessage> {
        // `'_` is an inferred lifetime placeholder (borrowed UI tree tied to `&self`).
        let title = if self.editing_name.is_empty() {
            "New Group"
        } else {
            "Edit Group"
        };

        // `row![...]` / `column![...]` are macros (`!`) that build widget lists.
        let name_row = row![
            text("Name:"),
            text_input("", &self.name)
                .on_input(|s| AppMessage::GroupEditorMessage(Message::NameChanged(s)))
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let flags_row = row![
            checkbox(self.is_default)
                .label("Default group")
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleDefault)),
            Space::new().width(12.0),
            checkbox(self.is_blacklist)
                .label("Blacklist")
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleBlacklist)),
            Space::new().width(12.0),
            checkbox(self.is_auto_mode_group)
                .label("Auto Mode group")
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleAutoModeGroup)),
        ]
        .spacing(10);

        let affinity_section = {
            let toggle = checkbox(self.affinity_enabled)
                .label("Set CPU affinity")
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleAffinity));

            let mut content = Column::new().spacing(10).push(toggle);

            if self.affinity_enabled && !self.is_blacklist {
                let quick_select = row![
                    button("All").on_press(AppMessage::GroupEditorMessage(Message::SelectAllCores)),
                    button("None")
                        .on_press(AppMessage::GroupEditorMessage(Message::SelectNoneCores)),
                ]
                .spacing(5);

                let topo = crate::core::topology::get_topology();
                let quick_select = if topo.is_hybrid() {
                    quick_select
                        .push(
                            button("P-cores")
                                .on_press(AppMessage::GroupEditorMessage(Message::SelectPCores)),
                        )
                        .push(
                            button("E-cores")
                                .on_press(AppMessage::GroupEditorMessage(Message::SelectECores)),
                        )
                } else {
                    quick_select
                };

                let quick_select = {
                    let mut row = quick_select;
                    for (i, _) in topo.get_ccd_groups().iter().enumerate() {
                        row = row.push(
                            button(text(format!("CCD {i}")))
                                .on_press(AppMessage::GroupEditorMessage(Message::SelectCCD(i))),
                        );
                    }
                    row
                };

                content = content.push(quick_select);

                let topo_view = topo.topology_view();
                content = content.push(draw_core_selector(
                    &topo_view,
                    &self.core_checks,
                    topology_group_repeat,
                    group_core_toggle_message,
                ));
            }
            content
        };

        let priority_section = {
            let toggle = checkbox(self.priority_enabled)
                .label("Set priority")
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::TogglePriority));

            let mut content = Column::new().spacing(10).push(toggle);

            if self.priority_enabled && !self.is_blacklist {
                let make_btn = |i: usize, lbl: &'static str| {
                    let selected = self.priority_index == i;
                    button(lbl)
                        .style(move |_, _| button::Style {
                            background: Some(Background::Color(if selected {
                                Color::from_rgb(0.2, 0.4, 0.8)
                            } else {
                                Color::from_rgb(0.3, 0.3, 0.3)
                            })),
                            text_color: Color::WHITE,
                            // `..Default::default()` fills remaining fields from defaults.
                            ..Default::default()
                        })
                        .on_press(AppMessage::GroupEditorMessage(Message::PriorityChanged(i)))
                };
                let priority_grid = column![
                    Row::new()
                        .spacing(5)
                        .push(make_btn(0, PRIORITY_LABELS[0]))
                        .push(make_btn(1, PRIORITY_LABELS[1]))
                        .push(make_btn(2, PRIORITY_LABELS[2])),
                    Row::new()
                        .spacing(5)
                        .push(make_btn(3, PRIORITY_LABELS[3]))
                        .push(make_btn(4, PRIORITY_LABELS[4]))
                        .push(make_btn(5, PRIORITY_LABELS[5])),
                ]
                .spacing(5);
                content = content.push(priority_grid);
            }
            content
        };

        let name_ok = !self.name.trim().is_empty();
        let affinity_ok = !self.affinity_enabled || self.core_checks.iter().any(|&c| c);

        let ok_button = {
            let btn = button("OK").style(move |_, _status| {
                if name_ok && affinity_ok {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.2, 0.4, 0.8))),
                        text_color: Color::WHITE,
                        ..Default::default()
                    }
                } else {
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                        text_color: Color::WHITE,
                        ..Default::default()
                    }
                }
            });

            if name_ok && affinity_ok {
                btn.on_press(AppMessage::GroupEditorMessage(Message::Ok))
            } else {
                btn
            }
        };

        let actions_row = row![
            ok_button,
            button("Cancel").on_press(AppMessage::GroupEditorMessage(Message::Cancel)),
        ]
        .spacing(10);

        let delete_actions = if !self.editing_name.is_empty() {
            row![
                Space::new().width(Length::Fill),
                button("Delete Group").on_press(AppMessage::GroupEditorMessage(Message::Delete))
            ]
            .spacing(10)
        } else {
            Row::new()
        };

        let content = column![
            text(title).size(18),
            name_row,
            flags_row,
            container(affinity_section).padding(10),
            container(priority_section).padding(10),
            actions_row,
            delete_actions,
        ]
        .spacing(15)
        .padding(20);

        let dialog = container(scrollable(content).height(Length::Shrink))
            .max_width(860)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(0.14, 0.14, 0.14))),
                border: Border {
                    color: Color::from_rgb(0.38, 0.38, 0.38),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

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
