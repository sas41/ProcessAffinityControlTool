use eframe::egui::{self, Color32, RichText};

use crate::core::process_config::{AffinityConfig, ProcessGroup};
use crate::gui::priority::{index_to_priority, priority_label, priority_to_index, PRIORITY_LABELS};

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

    pub fn show(&mut self, ctx: &egui::Context, num_cores: usize) {
        if !self.open {
            return;
        }

        let title = if self.editing_name.is_empty() {
            "New Group"
        } else {
            "Edit Group"
        };
        let mut open = self.open;

        // ── Editor window ─────────────────────────────────────────────────
        egui::Window::new(title)
            .open(&mut open)
            .resizable(true)
            .min_width(460.0)
            .show(ctx, |ui| {
                // ── Group name field ──────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.name);
                });

                ui.add_space(4.0);

                // ── Flag checkboxes (Default / Blacklist) ─────────────────
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.is_default, "Default group")
                        .on_hover_text("Processes not assigned to any other group go here.");
                    ui.add_space(12.0);
                    ui.checkbox(&mut self.is_blacklist, "Blacklist")
                        .on_hover_text("Processes in this group are skipped entirely.");
                });
                // Default and Blacklist are mutually exclusive.
                if self.is_default && self.is_blacklist {
                    self.is_blacklist = false;
                }

                ui.separator();

                // ── Affinity section ──────────────────────────────────────
                ui.horizontal(|ui| {
                    // ── "Set CPU affinity" toggle ─────────────────────────
                    ui.checkbox(&mut self.affinity_enabled, "Set CPU affinity");
                    if !self.affinity_enabled {
                        ui.label(
                            RichText::new("(leave affinity unchanged)")
                                .italics()
                                .color(Color32::GRAY),
                        );
                    }
                });

                if self.affinity_enabled && !self.is_blacklist {
                    // ── Quick-select buttons ──────────────────────────────
                    ui.horizontal(|ui| {
                        // ── All cores ─────────────────────────────────────
                        if ui.small_button("All").clicked() {
                            self.core_checks.iter_mut().for_each(|c| *c = true);
                        }
                        // ── No cores ──────────────────────────────────────
                        if ui.small_button("None").clicked() {
                            self.core_checks.iter_mut().for_each(|c| *c = false);
                        }
                        let topo = crate::core::topology::get_topology();
                        if topo.is_hybrid() {
                            // ── P-cores only ──────────────────────────────
                            if ui.small_button("P-cores").clicked() {
                                let pc = topo.get_performance_cores();
                                self.core_checks.iter_mut().for_each(|c| *c = false);
                                for i in pc {
                                    if i < self.core_checks.len() {
                                        self.core_checks[i] = true;
                                    }
                                }
                            }
                            // ── E-cores only ──────────────────────────────
                            if ui.small_button("E-cores").clicked() {
                                let ec = topo.get_efficiency_cores();
                                self.core_checks.iter_mut().for_each(|c| *c = false);
                                for i in ec {
                                    if i < self.core_checks.len() {
                                        self.core_checks[i] = true;
                                    }
                                }
                            }
                        }
                        // ── CCD quick-select buttons (one per CCD) ────────
                        for (i, grp) in topo.get_ccd_groups().iter().enumerate() {
                            if ui.small_button(format!("CCD {i}")).clicked() {
                                self.core_checks.iter_mut().for_each(|c| *c = false);
                                for &idx in grp {
                                    if idx < self.core_checks.len() {
                                        self.core_checks[idx] = true;
                                    }
                                }
                            }
                        }
                    });

                    // ── Core checkbox grid (8 per row) ────────────────────
                    let topo = crate::core::topology::get_topology();
                    let procs = topo.processors();
                    egui::Grid::new("ge_core_grid")
                        .num_columns(8)
                        .spacing([6.0, 4.0])
                        .show(ui, |ui| {
                            for (i, checked) in self.core_checks.iter_mut().enumerate() {
                                // Label each logical core with its kind prefix (P/E).
                                let lbl =
                                    if let Some(p) = procs.iter().find(|p| p.logical_index == i) {
                                        match p.kind {
                                            crate::core::topology::CoreKind::Pcore => {
                                                format!("P{i}")
                                            }
                                            crate::core::topology::CoreKind::Ecore => {
                                                format!("E{i}")
                                            }
                                            _ => format!("{i}"),
                                        }
                                    } else {
                                        format!("{i}")
                                    };
                                let rt = if *checked {
                                    RichText::new(&lbl).color(Color32::LIGHT_GREEN)
                                } else {
                                    RichText::new(&lbl).color(Color32::GRAY)
                                };
                                // ── Core checkbox ─────────────────────────
                                ui.checkbox(checked, rt);
                                if (i + 1) % 8 == 0 {
                                    ui.end_row();
                                }
                            }
                            if num_cores % 8 != 0 {
                                ui.end_row();
                            }
                        });
                }

                ui.separator();

                // ── Priority section ──────────────────────────────────────
                ui.horizontal(|ui| {
                    // ── "Set priority" toggle ─────────────────────────────
                    ui.checkbox(&mut self.priority_enabled, "Set priority");
                    if !self.priority_enabled {
                        ui.label(
                            RichText::new("(leave priority unchanged)")
                                .italics()
                                .color(Color32::GRAY),
                        );
                    }
                    // ── Priority level dropdown ───────────────────────────
                    if self.priority_enabled && !self.is_blacklist {
                        egui::ComboBox::from_id_salt("ge_priority")
                            .selected_text(priority_label(self.priority_index))
                            .show_ui(ui, |ui| {
                                for (i, &lbl) in PRIORITY_LABELS.iter().enumerate() {
                                    ui.selectable_value(&mut self.priority_index, i, lbl);
                                }
                            });
                    }
                });

                ui.add_space(8.0);

                // ── Action buttons row ────────────────────────────────────
                let name_ok = !self.name.trim().is_empty();
                let affinity_ok = !self.affinity_enabled || self.core_checks.iter().any(|&c| c);

                ui.horizontal(|ui| {
                    // ── OK button ─────────────────────────────────────────
                    if ui
                        .add_enabled(name_ok && affinity_ok, egui::Button::new("OK"))
                        .clicked()
                    {
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

                    // ── Cancel button ─────────────────────────────────────
                    if ui.button("Cancel").clicked() {
                        self.open = false;
                    }

                    // ── Delete button (existing groups only) ──────────────
                    if !self.editing_name.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Delete Group").color(Color32::LIGHT_RED),
                                    )
                                    .stroke(egui::Stroke::new(1.0, Color32::LIGHT_RED)),
                                )
                                .on_hover_text(
                                    "Remove this group. Its processes will be moved to the \
                                     default group, or have their affinity/priority restored \
                                     if no default group exists.",
                                )
                                .clicked()
                            {
                                self.delete_requested = true;
                                self.open = false;
                            }
                        });
                    }
                });
            });

        self.open = open;
    }
}
