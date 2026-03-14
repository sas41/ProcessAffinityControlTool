use eframe::egui::{self, Color32, RichText};

use crate::core::process_config::{AffinityConfig, ProcessGroup};
use crate::gui::topology_diagram::group_color;
use crate::ProcessAffinityApp;

impl ProcessAffinityApp {
    pub fn tab_options(&mut self, ui: &mut egui::Ui) {
        // ── Outer scroll area ─────────────────────────────────────────────
        egui::ScrollArea::vertical()
            .id_salt("opts_scroll")
            .show(ui, |ui| {
                // ── Scan interval section ─────────────────────────────────
                ui.heading("Scan Interval");
                let mut interval = self.pact.pact_process_overwatch.scan_interval();
                ui.horizontal(|ui| {
                    // ── Interval slider ───────────────────────────────────
                    if ui
                        .add(egui::Slider::new(&mut interval, 500u64..=10000u64).suffix(" ms"))
                        .changed()
                    {
                        self.pact.pact_process_overwatch.set_scan_interval(interval);
                    }
                });

                ui.separator();

                // ── Configuration file section ────────────────────────────
                ui.heading("Configuration");
                ui.horizontal(|ui| {
                    // ── Import config button ──────────────────────────────
                    if ui.button("Import Config…").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            self.pact.import_config(p.to_string_lossy().as_ref());
                        }
                    }
                    // ── Export config button ──────────────────────────────
                    if ui.button("Export Config…").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .save_file()
                        {
                            self.pact.export_config(p.to_string_lossy().as_ref());
                        }
                    }
                    // ── Reset to defaults button ──────────────────────────
                    if ui.button("Reset to Defaults").clicked() {
                        self.pact.reset_config();
                    }
                });

                ui.separator();

                // ── CPU topology info section ─────────────────────────────
                ui.heading("CPU Topology");
                let topo = crate::core::topology::get_topology();

                // ── Topology info grid ────────────────────────────────────
                egui::Grid::new("topo_info")
                    .num_columns(2)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Logical processors:");
                        ui.label(topo.total_logical_processors().to_string());
                        ui.end_row();
                        if topo.is_hybrid() {
                            ui.label("P-cores:");
                            ui.label(topo.get_performance_cores().len().to_string());
                            ui.end_row();
                            ui.label("E-cores:");
                            ui.label(topo.get_efficiency_cores().len().to_string());
                            ui.end_row();
                        }
                        let ccd = topo.get_ccd_groups();
                        if !ccd.is_empty() {
                            ui.label("AMD CCDs:");
                            ui.label(ccd.len().to_string());
                            ui.end_row();
                        }
                        let numa = topo.get_numa_groups();
                        if numa.len() > 1 {
                            ui.label("NUMA nodes:");
                            ui.label(numa.len().to_string());
                            ui.end_row();
                        }
                    });

                // ── Topology preset buttons ───────────────────────────────
                // Only shown when the CPU has interesting topology (hybrid or CCD).
                if topo.is_hybrid() || !topo.get_ccd_groups().is_empty() {
                    ui.add_space(4.0);
                    ui.label("Topology preset groups:");
                    ui.horizontal_wrapped(|ui| {
                        if topo.is_hybrid() {
                            // ── Create P-core group button ─────────────────
                            if ui.button("Create P-core group").clicked() {
                                let cores = topo.get_performance_cores();
                                let g = ProcessGroup {
                                    name: "P-cores".into(),
                                    affinity: Some(AffinityConfig::new(cores)),
                                    priority: None,
                                    is_default: false,
                                    is_blacklist: false,
                                };
                                self.pact.add_group(g);
                            }
                            // ── Create E-core group button ─────────────────
                            if ui.button("Create E-core group").clicked() {
                                let cores = topo.get_efficiency_cores();
                                let g = ProcessGroup {
                                    name: "E-cores".into(),
                                    affinity: Some(AffinityConfig::new(cores)),
                                    priority: None,
                                    is_default: false,
                                    is_blacklist: false,
                                };
                                self.pact.add_group(g);
                            }
                        }
                        // ── Create CCD group buttons (one per CCD) ─────────
                        for (i, grp) in topo.get_ccd_groups().iter().enumerate() {
                            let name = format!("CCD {i}");
                            if ui.button(format!("Create {} group", name)).clicked() {
                                let g = ProcessGroup {
                                    name,
                                    affinity: Some(AffinityConfig::new(grp.clone())),
                                    priority: None,
                                    is_default: false,
                                    is_blacklist: false,
                                };
                                self.pact.add_group(g);
                            }
                        }
                    });
                }

                ui.separator();

                // ── Groups overview section ───────────────────────────────
                ui.heading("Groups overview");
                let groups = self.pact.get_groups();
                if groups.is_empty() {
                    ui.label(RichText::new("No groups configured.").italics());
                } else {
                    // ── Groups overview table ─────────────────────────────
                    egui::Grid::new("groups_overview")
                        .num_columns(5)
                        .spacing([12.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // ── Table header row ──────────────────────────
                            ui.strong("Name");
                            ui.strong("Affinity");
                            ui.strong("Priority");
                            ui.strong("Flags");
                            ui.strong("Processes");
                            ui.end_row();

                            // ── Table data rows (one per group) ───────────
                            for (gi, g) in groups.iter().enumerate() {
                                let col = group_color(gi);
                                ui.label(RichText::new(&g.name).color(col));
                                match &g.affinity {
                                    Some(a) => ui.label(format!("{} cores", a.core_list.len())),
                                    None => ui.label(
                                        RichText::new("ignore").italics().color(Color32::GRAY),
                                    ),
                                };
                                match &g.priority {
                                    Some(p) => ui.label(format!("{:?}", p)),
                                    None => ui.label(
                                        RichText::new("ignore").italics().color(Color32::GRAY),
                                    ),
                                };
                                let flags: String = [
                                    g.is_default.then_some("default"),
                                    g.is_blacklist.then_some("blacklist"),
                                ]
                                .iter()
                                .flatten()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ");
                                ui.label(if flags.is_empty() { "-" } else { &flags });
                                let count = self.pact.get_processes_in_group(&g.name).len();
                                ui.label(count.to_string());
                                ui.end_row();
                            }
                        });
                }
            });
    }
}
