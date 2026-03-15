use iced::widget::{
    button, checkbox, column, container, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::core::process_config::{AffinityConfig, CustomProcess, ProcessPriority};
use crate::gui::priority::{index_to_priority, priority_to_index, PRIORITY_LABELS};
use crate::gui::Message as AppMessage;

// ─── Custom process editor dialog ─────────────────────────────────────────────

/// Modal dialog for creating or editing a CustomProcess (name + affinity + priority).
pub struct CustomProcessEditor {
    pub open: bool,

    /// Name of the process being edited; empty when creating a new entry.
    pub editing_name: String,

    pub name: String,

    // ── Affinity state ────────────────────────────────────────────────────
    pub affinity_enabled: bool,
    pub core_checks: Vec<bool>,

    // ── Priority state ────────────────────────────────────────────────────
    pub priority_enabled: bool,
    pub priority_index: usize,

    /// Populated when OK is pressed; consumed by the caller.
    pub result: Option<CustomProcess>,
    /// Set when Delete is pressed; consumed by the caller.
    pub delete_requested: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    NameChanged(String),
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

impl CustomProcessEditor {
    pub fn new(existing: Option<&CustomProcess>, num_cores: usize) -> Self {
        let cp = existing.cloned().unwrap_or_else(|| CustomProcess::new(""));
        let editing_name = cp.name.clone();
        let affinity_enabled = cp.affinity.is_some();
        let priority_enabled = cp.priority.is_some();

        let mut core_checks = vec![false; num_cores];
        if let Some(ref aff) = cp.affinity {
            for &c in &aff.core_list {
                if c < num_cores {
                    core_checks[c] = true;
                }
            }
        } else {
            core_checks.iter_mut().for_each(|c| *c = true);
        }

        Self {
            open: true,
            editing_name,
            name: cp.name,
            affinity_enabled,
            core_checks,
            priority_enabled,
            priority_index: cp.priority.as_ref().map_or(2, priority_to_index),
            result: None,
            delete_requested: false,
        }
    }

    pub fn update(&mut self, message: Message, _num_cores: usize) {
        match message {
            Message::NameChanged(name) => self.name = name,
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
                let affinity = if self.affinity_enabled {
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
                let priority: Option<ProcessPriority> = if self.priority_enabled {
                    Some(index_to_priority(self.priority_index))
                } else {
                    None
                };
                self.result = Some(CustomProcess {
                    name: self.name.trim().to_string(),
                    affinity,
                    priority,
                });
                self.open = false;
            }
            Message::Cancel => self.open = false,
            Message::Delete => {
                self.delete_requested = true;
                self.open = false;
            }
        }
    }

    pub fn view(&self) -> Element<'_, AppMessage> {
        let title = if self.editing_name.is_empty() {
            "New Custom Process"
        } else {
            "Edit Custom Process"
        };

        // Name field (editable only when creating)
        let name_row: Element<AppMessage> = if self.editing_name.is_empty() {
            row![
                text("Name:"),
                text_input("exe name…", &self.name)
                    .on_input(|s| AppMessage::CustomProcessEditorMessage(Message::NameChanged(s)))
                    .width(Length::Fill)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        } else {
            row![
                text("Name:"),
                text(self.name.as_str())
                    .size(14)
                    .color(Color::from_rgb(0.86, 0.86, 0.86)),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        };

        // Affinity section
        let affinity_section = {
            let toggle = checkbox("Set CPU affinity", self.affinity_enabled)
                .on_toggle(|_| AppMessage::CustomProcessEditorMessage(Message::ToggleAffinity));

            let mut content = Column::new().spacing(10).push(toggle);

            if self.affinity_enabled {
                let quick_select = row![
                    button("All").on_press(AppMessage::CustomProcessEditorMessage(
                        Message::SelectAllCores
                    )),
                    button("None").on_press(AppMessage::CustomProcessEditorMessage(
                        Message::SelectNoneCores
                    )),
                ]
                .spacing(5);

                let topo = crate::core::topology::get_topology();
                let quick_select = if topo.is_hybrid() {
                    quick_select
                        .push(button("P-cores").on_press(AppMessage::CustomProcessEditorMessage(
                            Message::SelectPCores,
                        )))
                        .push(button("E-cores").on_press(AppMessage::CustomProcessEditorMessage(
                            Message::SelectECores,
                        )))
                } else {
                    quick_select
                };

                let quick_select = {
                    let mut row = quick_select;
                    for (i, _) in topo.get_ccd_groups().iter().enumerate() {
                        row = row.push(
                            button(text(format!("CCD {i}"))).on_press(
                                AppMessage::CustomProcessEditorMessage(Message::SelectCCD(i)),
                            ),
                        );
                    }
                    row
                };

                content = content.push(quick_select);

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

                    let cb = checkbox(lbl, *checked).on_toggle(move |c| {
                        AppMessage::CustomProcessEditorMessage(Message::CoreToggled(i, c))
                    });

                    current_row = current_row.push(cb);
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
                .on_toggle(|_| AppMessage::CustomProcessEditorMessage(Message::TogglePriority));

            let mut content = Column::new().spacing(10).push(toggle);

            if self.priority_enabled {
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
                        .on_press(AppMessage::CustomProcessEditorMessage(
                            Message::PriorityChanged(i),
                        ))
                };
                let priority_grid = column![
                    Row::new().spacing(5)
                        .push(make_btn(0, PRIORITY_LABELS[0]))
                        .push(make_btn(1, PRIORITY_LABELS[1]))
                        .push(make_btn(2, PRIORITY_LABELS[2])),
                    Row::new().spacing(5)
                        .push(make_btn(3, PRIORITY_LABELS[3]))
                        .push(make_btn(4, PRIORITY_LABELS[4]))
                        .push(make_btn(5, PRIORITY_LABELS[5])),
                ].spacing(5);
                content = content.push(priority_grid);
            }
            content
        };

        // Action buttons
        let name_ok = !self.name.trim().is_empty();
        let affinity_ok = !self.affinity_enabled || self.core_checks.iter().any(|&c| c);

        let actions_row = row![
            button("OK")
                .on_press(AppMessage::CustomProcessEditorMessage(Message::Ok))
                .style(move |_, _| button::Style {
                    background: Some(Background::Color(if name_ok && affinity_ok {
                        Color::from_rgb(0.2, 0.4, 0.8)
                    } else {
                        Color::from_rgb(0.3, 0.3, 0.3)
                    })),
                    text_color: Color::WHITE,
                    ..Default::default()
                }),
            button("Cancel")
                .on_press(AppMessage::CustomProcessEditorMessage(Message::Cancel)),
        ]
        .spacing(10);

        let delete_row = if !self.editing_name.is_empty() {
            row![
                Space::with_width(Length::Fill),
                button("Remove").on_press(AppMessage::CustomProcessEditorMessage(Message::Delete))
            ]
            .spacing(10)
        } else {
            Row::new()
        };

        let content = column![
            text(title).size(18),
            name_row,
            container(affinity_section).padding(10),
            container(priority_section).padding(10),
            actions_row,
            delete_row,
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
