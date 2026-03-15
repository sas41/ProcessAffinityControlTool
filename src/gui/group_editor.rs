use iced::widget::{
    button, checkbox, column, container, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::core::process_config::{AffinityConfig, ProcessGroup};
use crate::gui::priority::{index_to_priority, priority_to_index, PRIORITY_LABELS};
use crate::gui::Message as AppMessage;

// ─── ProcessGroup default constructor ────────────────────────────────────────

pub trait ProcessGroupExt {
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
        }
    }
}

// ─── Group editor dialog ──────────────────────────────────────────────────────

/// Editable working copy of a ProcessGroup, displayed as a floating window.
pub struct GroupEditor {
    pub open: bool,

    /// Name of the group being edited; empty when creating a new group.
    pub editing_name: String,

    // ── Editable fields ───────────────────────────────────────────────────
    pub name: String,
    pub is_blacklist: bool,
    pub is_default: bool,

    // ── Affinity state ────────────────────────────────────────────────────
    pub affinity_enabled: bool,
    pub core_checks: Vec<bool>, // one entry per logical core

    // ── Priority state ────────────────────────────────────────────────────
    pub priority_enabled: bool,
    pub priority_index: usize,

    /// Populated when OK is pressed; consumed by the caller.
    pub result: Option<ProcessGroup>,

    /// Set to true when the Delete button is pressed; consumed by the caller.
    pub delete_requested: bool,
}

// Messages for the Group Editor
#[derive(Debug, Clone)]
pub enum Message {
    NameChanged(String),
    ToggleDefault,
    ToggleBlacklist,
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
    pub fn new(group: Option<&ProcessGroup>, num_cores: usize) -> Self {
        let g = group.cloned().unwrap_or_else(ProcessGroup::default_new);
        let editing_name = g.name.clone();
        let affinity_enabled = g.affinity.is_some();
        let priority_enabled = g.priority.is_some();

        let mut core_checks = vec![false; num_cores];
        if let Some(ref aff) = g.affinity {
            for &c in &aff.core_list {
                if c < num_cores {
                    core_checks[c] = true;
                }
            }
        } else {
            // Default all cores on so the user can enable affinity immediately.
            core_checks.iter_mut().for_each(|c| *c = true);
        }

        Self {
            open: true,
            editing_name,
            name: g.name,
            is_blacklist: g.is_blacklist,
            is_default: g.is_default,
            affinity_enabled,
            core_checks,
            priority_enabled,
            priority_index: g.priority.as_ref().map_or(2, priority_to_index),
            result: None,
            delete_requested: false,
        }
    }

    pub fn update(&mut self, message: Message, _num_cores: usize) {
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
                    self.core_checks.iter_mut().for_each(|c| *c = false);
                    for &idx in grp {
                        if idx < self.core_checks.len() {
                            self.core_checks[idx] = true;
                        }
                    }
                }
            }
            Message::Ok => {
                let affinity = if self.affinity_enabled && !self.is_blacklist {
                    let cores: Vec<usize> = self
                        .core_checks
                        .iter()
                        .enumerate()
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

    pub fn view(&self, _num_cores: usize) -> Element<'_, AppMessage> {
        let title = if self.editing_name.is_empty() {
            "New Group"
        } else {
            "Edit Group"
        };

        // Group name field
        let name_row = row![
            text("Name:"),
            text_input("", &self.name)
                .on_input(|s| AppMessage::GroupEditorMessage(Message::NameChanged(s)))
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Flags
        let flags_row = row![
            checkbox("Default group", self.is_default)
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleDefault)),
            Space::with_width(12.0),
            checkbox("Blacklist", self.is_blacklist)
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleBlacklist)),
        ]
        .spacing(10);

        // Affinity section
        let affinity_section = {
            let toggle = checkbox("Set CPU affinity", self.affinity_enabled)
                .on_toggle(|_| AppMessage::GroupEditorMessage(Message::ToggleAffinity));

            let mut content = Column::new().spacing(10).push(toggle);

            if self.affinity_enabled && !self.is_blacklist {
                // Quick select buttons
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

                // Core checkbox grid (8 per row)
                let procs = topo.processors();
                let mut grid_rows = Column::new().spacing(4);
                let mut current_row = Row::new().spacing(6);
                let mut items_in_row = 0;

                for (i, checked) in self.core_checks.iter().enumerate() {
                    let lbl = if let Some(p) = procs.iter().find(|p| p.logical_index == i) {
                        match p.kind {
                            crate::core::topology::CoreKind::Pcore => format!("P{i}"),
                            crate::core::topology::CoreKind::Ecore => format!("E{i}"),
                            _ => format!("{i}"),
                        }
                    } else {
                        format!("{i}")
                    };

                    let checkbox = checkbox(lbl, *checked).on_toggle(move |c| {
                        AppMessage::GroupEditorMessage(Message::CoreToggled(i, c))
                    });

                    current_row = current_row.push(checkbox);
                    items_in_row += 1;

                    if items_in_row >= 8 {
                        grid_rows = grid_rows.push(current_row);
                        current_row = Row::new().spacing(6);
                        items_in_row = 0;
                    }
                }
                if items_in_row > 0 {
                    grid_rows = grid_rows.push(current_row);
                }

                content = content.push(grid_rows);
            }
            content
        };

        // Priority section
        let priority_section = {
            let toggle = checkbox("Set priority", self.priority_enabled)
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

        // Action buttons
        let name_ok = !self.name.trim().is_empty();
        let affinity_ok = !self.affinity_enabled || self.core_checks.iter().any(|&c| c);

        let actions_row = row![
            button("OK")
                .on_press(AppMessage::GroupEditorMessage(Message::Ok))
                .style(move |_, _status| {
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
                }),
            button("Cancel").on_press(AppMessage::GroupEditorMessage(Message::Cancel)),
        ]
        .spacing(10);

        let delete_actions = if !self.editing_name.is_empty() {
            row![
                Space::with_width(Length::Fill),
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
            .max_width(520)
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
