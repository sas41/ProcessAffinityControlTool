use eframe::egui::{self, Color32, RichText};

use crate::gui::topology_diagram::{
    build_core_group_map, draw_topology_group, group_color, group_section_color,
};
use crate::gui::widgets::{color_swatch, stat_badge};
use crate::ProcessAffinityApp;

impl ProcessAffinityApp {
    pub fn tab_status(&mut self, ui: &mut egui::Ui) {
        let stats = self.pact.pact_process_overwatch.cpu_stats();
        let groups = self.pact.get_groups();

        // ── Top control bar ───────────────────────────────────────────────
        ui.horizontal(|ui| {
            // ── Pause / Resume scanner button ─────────────────────────────
            let active = self.pact.pact_process_overwatch.is_scanner_active();
            let lbl = if active { "⏸ Pause" } else { "▶ Resume" };
            let col = if active {
                Color32::LIGHT_GREEN
            } else {
                Color32::LIGHT_RED
            };
            if ui.button(RichText::new(lbl).color(col)).clicked() {
                self.pact.toggle_process_overwatch();
            }

            ui.add_space(8.0);

            // ── Auto mode toggle button ───────────────────────────────────
            let auto = self.pact.pact_process_overwatch.is_auto_mode();
            let albl = if auto {
                "🤖 Auto: ON"
            } else {
                "🤖 Auto: OFF"
            };
            let acol = if auto {
                Color32::LIGHT_GREEN
            } else {
                Color32::GRAY
            };
            if ui.button(RichText::new(albl).color(acol)).clicked() {
                self.pact.toggle_auto_mode();
            }

            ui.add_space(8.0);

            // ── Fresh scan button ─────────────────────────────────────────
            if ui.button("🔄 Fresh Scan").clicked() {
                self.pact.request_fresh_scan();
            }
        });

        ui.separator();

        // ── Process count badges ──────────────────────────────────────────
        let running = self.pact.get_all_running_processes();
        let assigned: usize = {
            let cfg = self.pact.pact_process_overwatch.user_config_lock();
            running
                .iter()
                .filter(|n| cfg.process_assignments.get(*n).is_some())
                .count()
        };
        let protected = self.pact.pact_process_overwatch.protected_process_count();

        ui.horizontal(|ui| {
            // ── Total processes badge ─────────────────────────────────────
            stat_badge(ui, "Total", running.len(), Color32::LIGHT_BLUE);
            ui.add_space(12.0);
            // ── Assigned processes badge ──────────────────────────────────
            stat_badge(ui, "Assigned", assigned, Color32::LIGHT_GREEN);
            ui.add_space(12.0);
            // ── Inaccessible processes badge ──────────────────────────────
            stat_badge(ui, "Inaccessible", protected, Color32::LIGHT_RED);
            ui.add_space(12.0);
            // ── Group count badge ─────────────────────────────────────────
            stat_badge(ui, "Groups", groups.len(), Color32::GOLD);
        });

        ui.separator();

        // ── Global CPU usage bar ──────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("CPU Total:").strong());
            ui.add(
                egui::ProgressBar::new((stats.global / 100.0).clamp(0.0, 1.0))
                    .text(format!("{:.0}%", stats.global))
                    .desired_width(300.0),
            );
        });

        ui.add_space(4.0);

        // ── CPU topology diagram + legend (scrollable) ────────────────────
        let core_group_map = build_core_group_map(&groups, self.num_cores);
        let topo_view = crate::core::topology::get_topology().topology_view();

        egui::ScrollArea::vertical()
            .id_salt("status_scroll")
            .show(ui, |ui| {
                // ── Topology diagram ──────────────────────────────────────
                // Each top-level topology group (CCD / P-cluster / E-cluster)
                // is drawn as a separate box, wrapping to the next line when
                // the row is full.
                ui.horizontal_wrapped(|ui| {
                    for (gi, topo_group) in topo_view.groups.iter().enumerate() {
                        let outer_stroke_col = group_section_color(gi);
                        draw_topology_group(
                            ui,
                            topo_group,
                            &stats,
                            &core_group_map,
                            outer_stroke_col,
                        );
                    }
                });

                ui.add_space(6.0);

                // ── Group colour legend ───────────────────────────────────
                if !groups.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        // ── "No group" swatch ─────────────────────────────
                        color_swatch(ui, Color32::from_gray(80), "No group");
                        // ── Per-group colour swatches ─────────────────────
                        for (gi, g) in groups.iter().enumerate() {
                            let lbl = if g.is_blacklist {
                                format!("🚫 {}", g.name)
                            } else if g.is_default {
                                format!("★ {}", g.name)
                            } else {
                                g.name.clone()
                            };
                            color_swatch(ui, group_color(gi), &lbl);
                        }
                    });
                }
            });
    }
}
