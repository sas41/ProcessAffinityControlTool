mod core;
mod gui;

use eframe::egui;
use eframe::{App, Frame};

use gui::group_editor::GroupEditor;

// ─── Application state ────────────────────────────────────────────────────────

pub struct ProcessAffinityApp {
    pub pact: core::pact_instance::PACTInstance,
    pub num_cores: usize,

    pub active_tab: usize, // 0=Status 1=Configure 2=AutoMode 3=Options

    // Configure tab — text input for adding custom process names
    pub new_process_name: String,

    // Group editor dialog
    pub group_editor: Option<GroupEditor>,

    // Auto mode tab
    pub new_launcher_name: String,
}

impl Default for ProcessAffinityApp {
    fn default() -> Self {
        let mut pact = core::pact_instance::PACTInstance::new();
        pact.start_scan_handler();
        Self {
            num_cores: num_cpus::get(),
            pact,
            active_tab: 0,
            new_process_name: String::new(),
            group_editor: None,
            new_launcher_name: String::new(),
        }
    }
}

// ─── App trait ────────────────────────────────────────────────────────────────

impl App for ProcessAffinityApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // ── Menu bar ──
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Import Config…").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            self.pact.import_config(p.to_string_lossy().as_ref());
                        }
                        ui.close();
                    }
                    if ui.button("Export Config…").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .save_file()
                        {
                            self.pact.export_config(p.to_string_lossy().as_ref());
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("GitHub…").clicked() {
                        let _ = open::that(
                            "https://github.com/sas41/ProcessAffinityControlTool#readme",
                        );
                        ui.close();
                    }
                });
            });
        });

        // ── Tab bar ──
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, lbl) in ["Status", "Configure", "Auto Mode", "Options"]
                    .iter()
                    .enumerate()
                {
                    if ui.selectable_label(self.active_tab == i, *lbl).clicked() {
                        self.active_tab = i;
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            0 => self.tab_status(ui),
            1 => self.tab_configure(ui),
            2 => self.tab_auto_mode(ui),
            3 => self.tab_options(ui),
            _ => {}
        });

        // ── Group editor dialog ──
        if let Some(ed) = &mut self.group_editor {
            ed.show(ctx, self.num_cores);
        }
        // Consume save result
        let result = self.group_editor.as_mut().and_then(|e| e.result.take());
        if let Some(new_group) = result {
            let old_name = self
                .group_editor
                .as_ref()
                .map(|e| e.editing_name.clone())
                .unwrap_or_default();
            if old_name.is_empty() {
                self.pact.add_group(new_group);
            } else {
                self.pact.update_group(&old_name, new_group);
            }
        }
        // Consume delete request
        let delete_name = self.group_editor.as_ref().and_then(|e| {
            if e.delete_requested {
                Some(e.editing_name.clone())
            } else {
                None
            }
        });
        if let Some(name) = delete_name {
            self.pact.delete_group(&name);
        }
        if self.group_editor.as_ref().map_or(false, |e| !e.open) {
            self.group_editor = None;
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.pact.stop_scan_handler();
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 660.0])
            .with_min_inner_size([700.0, 480.0])
            .with_title("Process Affinity Control Tool"),
        ..Default::default()
    };
    eframe::run_native(
        "Process Affinity Control Tool",
        options,
        Box::new(|_cc| Ok(Box::new(ProcessAffinityApp::default()))),
    )
}
