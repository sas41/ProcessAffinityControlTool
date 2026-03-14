use eframe::egui::{self, Color32, RichText};

use crate::gui::group_editor::GroupEditor;
use crate::gui::widgets::{process_pill, process_pill_edit};
use crate::ProcessAffinityApp;

// ─── Drag-and-drop payload ────────────────────────────────────────────────────

/// Drag-and-drop payload: a process name and where it came from.
#[derive(Clone)]
pub struct DragPayload {
    pub name: String,
    /// None = came from a bottom panel; Some(g) = came from group g.
    pub source_group: Option<String>,
}

// ─── Configure tab ────────────────────────────────────────────────────────────

impl ProcessAffinityApp {
    pub fn tab_configure(&mut self, ui: &mut egui::Ui) {
        let groups = self.pact.get_groups();
        let num_cores = self.num_cores;

        // Mutations collected during the UI pass and applied afterwards.
        let mut assign_to: Option<(String, String)> = None;
        let mut open_editor: Option<usize> = None;
        let mut open_new_group = false;
        let mut edit_custom: Option<String> = None;

        // ── Layout constants ──────────────────────────────────────────────
        const CARDS_PER_ROW: usize = 4;

        // Vertical split: top half = group grid, bottom half = panels.
        let avail_h = ui.available_height();
        let item_gap = ui.spacing().item_spacing.y;
        let section_h = (avail_h - item_gap) / 2.0;

        // ── Data snapshots ────────────────────────────────────────────────
        let running = self.pact.get_all_running_processes();
        let running_set: std::collections::HashSet<String> =
            running.iter().map(|s| s.to_lowercase()).collect();
        let assigned_names: std::collections::HashSet<String> = self
            .pact
            .get_assigned_processes()
            .into_iter()
            .map(|(n, _)| n)
            .collect();

        // Read popup-trigger state before drawing (avoids double borrow).
        let popup_group: Option<String> =
            ui.memory(|m| m.data.get_temp(egui::Id::new("add_to_group_popup")));
        let popup_custom: Option<String> =
            ui.memory(|m| m.data.get_temp(egui::Id::new("add_custom_popup")));

        // ── Group card grid ───────────────────────────────────────────────
        // One ui.columns(4) per row. ui.columns handles equal-width
        // splitting and gaps between columns automatically.
        let total_slots = groups.len() + 1; // last slot = new-group (+) card
        let num_rows = (total_slots + CARDS_PER_ROW - 1) / CARDS_PER_ROW;
        let row_h = (section_h - item_gap * (num_rows as f32 - 1.0)) / num_rows as f32;

        for row in 0..num_rows {
            // ── Card row via ui.columns ───────────────────────────────────
            ui.columns(CARDS_PER_ROW, |cols| {
                for col in 0..CARDS_PER_ROW {
                    let slot = row * CARDS_PER_ROW + col;
                    if slot >= total_slots {
                        break;
                    }

                    let ui = &mut cols[col];
                    let card_w = ui.available_width();

                    if slot < groups.len() {
                        // ── Group card ────────────────────────────────────
                        let gi = slot;
                        let g = groups[gi].clone();
                        let gname = g.name.clone();
                        let procs = self.pact.get_processes_in_group(&gname);

                        // ── Card footprint ────────────────────────────────
                        let (card_rect, _) =
                            ui.allocate_exact_size(egui::vec2(card_w, row_h), egui::Sense::hover());

                        // Detect drop-target for border highlight.
                        let drop_response = ui.interact(
                            card_rect,
                            egui::Id::new(format!("drop_{gi}")),
                            egui::Sense::hover(),
                        );
                        let is_drop_target =
                            egui::DragAndDrop::has_payload_of_type::<DragPayload>(ui.ctx())
                                && drop_response.contains_pointer();

                        // ── Card background + border ──────────────────────
                        let border_col = if is_drop_target {
                            Color32::from_rgb(100, 180, 255)
                        } else {
                            Color32::from_gray(90)
                        };
                        ui.painter()
                            .rect_filled(card_rect, 10.0, Color32::from_gray(20));
                        ui.painter().rect_stroke(
                            card_rect,
                            10.0,
                            egui::Stroke::new(1.5, border_col),
                            egui::StrokeKind::Inside,
                        );

                        // ── Card interior (natural egui layout) ───────────
                        let inner_rect = card_rect.shrink(8.0);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                            ui.set_clip_rect(inner_rect);

                            // ── Card header (title + buttons) ─────────────
                            ui.horizontal(|ui| {
                                // ── Group name label ──────────────────────
                                let badge = if g.is_default {
                                    "★ "
                                } else if g.is_blacklist {
                                    "🚫 "
                                } else {
                                    ""
                                };
                                ui.label(RichText::new(format!("{badge}{}", g.name)).strong());
                                // ── Header buttons (✏ +) ──────────────────
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // ── Edit button ───────────────────
                                        if ui.small_button("✏").clicked() {
                                            open_editor = Some(gi);
                                        }
                                        // ── Add process button ─────────────
                                        if ui.small_button("+").clicked() {
                                            ui.memory_mut(|m| {
                                                m.data.insert_temp::<String>(
                                                    egui::Id::new("add_to_group_popup"),
                                                    gname.clone(),
                                                );
                                            });
                                        }
                                    },
                                );
                            });

                            // ── Header / body separator ────────────────────
                            ui.separator();

                            // ── Process pill list (scrollable) ────────────
                            egui::ScrollArea::vertical()
                                .id_salt(format!("grp_scroll_{gi}"))
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                                    // ── Process pill (draggable) ──────────
                                    for pname in &procs {
                                        let is_running =
                                            running_set.contains(&pname.to_lowercase());
                                        let pid = egui::Id::new(format!("pill_g{gi}_{pname}"));
                                        let payload = DragPayload {
                                            name: pname.clone(),
                                            source_group: Some(gname.clone()),
                                        };
                                        ui.dnd_drag_source(pid, payload, |ui| {
                                            process_pill(ui, pname, is_running);
                                        });
                                    }
                                });
                        });

                        // Accept a dropped pill onto this card.
                        if let Some(payload) = drop_response.dnd_release_payload::<DragPayload>() {
                            if payload.source_group.as_deref() != Some(&gname) {
                                assign_to = Some((payload.name.clone(), gname.clone()));
                            }
                        }
                    } else {
                        // ── "New group" (+) card ──────────────────────────
                        let (card_rect, card_resp) =
                            ui.allocate_exact_size(egui::vec2(card_w, row_h), egui::Sense::click());

                        // ── New-group card background + border ─────────────
                        ui.painter()
                            .rect_filled(card_rect, 10.0, Color32::from_gray(20));
                        ui.painter().rect_stroke(
                            card_rect,
                            10.0,
                            egui::Stroke::new(
                                1.5,
                                if card_resp.hovered() {
                                    Color32::from_gray(140)
                                } else {
                                    Color32::from_gray(90)
                                },
                            ),
                            egui::StrokeKind::Inside,
                        );
                        // ── "+" icon ───────────────────────────────────────
                        ui.painter().text(
                            card_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+",
                            egui::FontId::proportional(32.0),
                            Color32::from_gray(70),
                        );
                        if card_resp.clicked() {
                            open_new_group = true;
                        }
                    }
                }
            });
        }

        // ── Bottom panels (Running / Custom) ──────────────────────────────
        let custom: Vec<(String, String)> = self.pact.get_assigned_processes();
        let unassigned: Vec<String> = running
            .iter()
            .filter(|n| !assigned_names.contains(*n))
            .cloned()
            .collect();

        // ── Bottom panel two-column row ───────────────────────────────────
        ui.columns(2, |cols| {
            // ── Running Processes card ────────────────────────────────────
            {
                let ui = &mut cols[0];
                let panel_w = ui.available_width();

                // ── Card footprint ────────────────────────────────────────
                let (card_rect, _) =
                    ui.allocate_exact_size(egui::vec2(panel_w, section_h), egui::Sense::hover());

                // Highlight when a pill is dragged over.
                let drop_response = ui.interact(
                    card_rect,
                    egui::Id::new("drop_panel_0"),
                    egui::Sense::hover(),
                );
                let is_drop_target =
                    egui::DragAndDrop::has_payload_of_type::<DragPayload>(ui.ctx())
                        && drop_response.contains_pointer();

                // ── Card background + border ──────────────────────────────
                let border_col = if is_drop_target {
                    Color32::from_rgb(100, 180, 255)
                } else {
                    Color32::from_gray(90)
                };
                ui.painter()
                    .rect_filled(card_rect, 10.0, Color32::from_gray(20));
                ui.painter().rect_stroke(
                    card_rect,
                    10.0,
                    egui::Stroke::new(1.5, border_col),
                    egui::StrokeKind::Inside,
                );

                // ── Card interior (natural egui layout) ──────────────────
                let inner_rect = card_rect.shrink(8.0);
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                    ui.set_clip_rect(inner_rect);

                    // ── "Running Processes" title ──────────────────────────
                    ui.label(RichText::new("Running Processes").strong());

                    // ── Separator ─────────────────────────────────────────
                    ui.separator();

                    // ── Process pill list (scrollable) ────────────────────
                    egui::ScrollArea::vertical()
                        .id_salt("panel_scroll_0")
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                            // ── Running process pills ──────────────────────
                            for pname in &unassigned {
                                let pid = egui::Id::new(format!("pill_run_{pname}"));
                                let payload = DragPayload {
                                    name: pname.clone(),
                                    source_group: None,
                                };
                                ui.dnd_drag_source(pid, payload, |ui| {
                                    process_pill(ui, pname, true);
                                });
                            }
                        });
                });

                let _ = drop_response;
            }

            // ── Custom Processes card ─────────────────────────────────────
            {
                let ui = &mut cols[1];
                let panel_w = ui.available_width();

                // ── Card footprint ────────────────────────────────────────
                let (card_rect, _) =
                    ui.allocate_exact_size(egui::vec2(panel_w, section_h), egui::Sense::hover());

                // ── Card background + border ──────────────────────────────
                ui.painter()
                    .rect_filled(card_rect, 10.0, Color32::from_gray(20));
                ui.painter().rect_stroke(
                    card_rect,
                    10.0,
                    egui::Stroke::new(1.5, Color32::from_gray(90)),
                    egui::StrokeKind::Inside,
                );

                // ── Card interior (natural egui layout) ──────────────────
                let inner_rect = card_rect.shrink(8.0);
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                    ui.set_clip_rect(inner_rect);

                    // ── Header: title + text field + "+" button ────────────
                    ui.horizontal(|ui| {
                        // ── "Custom Processes" title ───────────────────────
                        ui.label(RichText::new("Custom Processes").strong());
                        // ── Add-custom input + button ──────────────────────
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // ── "+" button ────────────────────────────────
                            if ui.small_button("+").clicked() && !self.new_process_name.is_empty() {
                                let n = self.new_process_name.clone();
                                ui.memory_mut(|m| {
                                    m.data.insert_temp::<String>(
                                        egui::Id::new("add_custom_popup"),
                                        n,
                                    );
                                });
                            }
                            // ── exe name text field ────────────────────────
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_process_name)
                                    .desired_width(100.0)
                                    .hint_text("exe name…"),
                            );
                        });
                    });

                    // ── Separator ─────────────────────────────────────────
                    ui.separator();

                    // ── Process pill list (scrollable) ────────────────────
                    egui::ScrollArea::vertical()
                        .id_salt("panel_scroll_1")
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                            // ── Custom process pills (with ✏) ──────────────
                            for (pname, gname) in &custom {
                                let pid = egui::Id::new(format!("pill_cust_{pname}"));
                                let payload = DragPayload {
                                    name: pname.clone(),
                                    source_group: Some(gname.clone()),
                                };
                                ui.dnd_drag_source(pid, payload, |ui| {
                                    process_pill_edit(ui, pname, &mut edit_custom);
                                });
                            }
                        });
                });
            }
        }); // ui.columns

        // ── Popups ────────────────────────────────────────────────────────

        // ── "Add process to group" popup ──────────────────────────────────
        if let Some(ref tg) = popup_group {
            let tg = tg.clone();
            egui::Window::new(format!("Add process to '{tg}'"))
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    // ── Process name field ────────────────────────────────
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_process_name);
                    });
                    // ── Add / Cancel buttons ──────────────────────────────
                    ui.horizontal(|ui| {
                        if ui.button("Add").clicked() && !self.new_process_name.is_empty() {
                            let n = self.new_process_name.drain(..).collect::<String>();
                            assign_to = Some((n, tg.clone()));
                            ui.memory_mut(|m| {
                                m.data.remove::<String>(egui::Id::new("add_to_group_popup"));
                            });
                        }
                        if ui.button("Cancel").clicked() {
                            ui.memory_mut(|m| {
                                m.data.remove::<String>(egui::Id::new("add_to_group_popup"));
                            });
                        }
                    });
                });
        }

        // ── "Assign custom process to group" popup ────────────────────────
        if let Some(ref pn) = popup_custom {
            let pn = pn.clone();
            egui::Window::new("Assign to group")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Assign \"{pn}\" to:"));
                    // ── Group buttons ─────────────────────────────────────
                    let cg = self.pact.get_groups();
                    for g in &cg {
                        if ui.button(&g.name).clicked() {
                            assign_to = Some((pn.clone(), g.name.clone()));
                            self.new_process_name.clear();
                            ui.memory_mut(|m| {
                                m.data.remove::<String>(egui::Id::new("add_custom_popup"));
                            });
                        }
                    }
                    // ── Cancel button ─────────────────────────────────────
                    if ui.button("Cancel").clicked() {
                        ui.memory_mut(|m| {
                            m.data.remove::<String>(egui::Id::new("add_custom_popup"));
                        });
                    }
                });
        }

        // ── Apply mutations ───────────────────────────────────────────────
        // Pencil click on a custom pill → open the assign-to-group popup.
        if let Some(pname) = edit_custom {
            ui.memory_mut(|m| {
                m.data
                    .insert_temp::<String>(egui::Id::new("add_custom_popup"), pname);
            });
        }
        if let Some((proc, group)) = assign_to {
            self.pact.assign_process(&proc, &group);
        }
        if let Some(gi) = open_editor {
            if let Some(g) = groups.get(gi) {
                self.group_editor = Some(GroupEditor::new(Some(g), num_cores));
            }
        }
        if open_new_group {
            self.group_editor = Some(GroupEditor::new(None, num_cores));
        }
    }
}
