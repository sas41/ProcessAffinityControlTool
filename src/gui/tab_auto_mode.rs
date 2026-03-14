use eframe::egui::{self, Color32, RichText};

use crate::ProcessAffinityApp;

impl ProcessAffinityApp {
    pub fn tab_auto_mode(&mut self, ui: &mut egui::Ui) {
        // ── Auto mode toggle button ───────────────────────────────────────
        let auto = self.pact.pact_process_overwatch.is_auto_mode();
        let lbl = if auto {
            "🤖 Auto Mode: ON"
        } else {
            "🤖 Auto Mode: OFF"
        };
        let col = if auto {
            Color32::LIGHT_GREEN
        } else {
            Color32::LIGHT_RED
        };
        if ui.button(RichText::new(lbl).color(col)).clicked() {
            self.pact.toggle_auto_mode();
        }

        // ── Description label ─────────────────────────────────────────────
        ui.label(
            "Child processes of registered launchers are automatically routed to the default group.",
        );

        ui.separator();

        // ── Two-column layout: Launchers | Detections ─────────────────────
        ui.columns(2, |cols| {
            // ── Left column: registered launchers ────────────────────────
            cols[0].vertical(|ui| {
                ui.heading("Launchers");

                let launchers = self.pact.get_auto_mode_launchers();
                let mut to_remove: Option<String> = None;

                // ── Launcher list (scrollable) ────────────────────────────
                egui::ScrollArea::vertical()
                    .id_salt("am_launchers")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for name in &launchers {
                            // ── Launcher row: name + ✕ remove button ──────
                            ui.horizontal(|ui| {
                                ui.label(name.as_str());
                                if ui.small_button("✕").clicked() {
                                    to_remove = Some(name.clone());
                                }
                            });
                        }
                        if launchers.is_empty() {
                            ui.label(RichText::new("(empty)").italics());
                        }
                    });

                if let Some(n) = to_remove {
                    self.pact.remove_from_auto_mode_launchers(&n);
                }

                ui.add_space(4.0);

                // ── Add launcher row: text field + Add button ─────────────
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.new_launcher_name);
                    if ui.button("Add").clicked() && !self.new_launcher_name.is_empty() {
                        let n = self.new_launcher_name.drain(..).collect::<String>();
                        self.pact.add_to_auto_mode_launchers(&n);
                    }
                });
            });

            // ── Right column: detected processes this session ─────────────
            cols[1].vertical(|ui| {
                ui.heading("Detected this session");

                let detections = self.pact.get_auto_mode_detections();

                // ── Detection list (scrollable) ───────────────────────────
                egui::ScrollArea::vertical()
                    .id_salt("am_detections")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for name in &detections {
                            ui.label(name.as_str());
                        }
                        if detections.is_empty() {
                            ui.label(RichText::new("(none detected yet)").italics());
                        }
                    });
            });
        });
    }
}
