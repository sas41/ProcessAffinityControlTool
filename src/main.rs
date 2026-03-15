mod core;
mod gui;

use std::time::Duration;

use iced::widget::{button, column, container, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Settings, Subscription, Task};
use iced_aw::{TabBar, TabLabel};

// ─── Icon / tray helpers ──────────────────────────────────────────────────────

const ICON_PNG: &[u8] = include_bytes!("../assets/icon/PACT Logo.png");

/// Decode the PNG icon to raw RGBA bytes once.
fn load_icon_rgba() -> (Vec<u8>, u32, u32) {
    use image::GenericImageView;
    let img = image::load_from_memory(ICON_PNG)
        .expect("Failed to load PACT icon")
        .to_rgba8();
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

// ─── Tray state ───────────────────────────────────────────────────────────────

/// Holds the live tray icon and the IDs of its context-menu items so we can
/// identify which item was clicked without allocating new strings each poll.
struct TrayState {
    _icon: tray_icon::TrayIcon,
    show_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

fn create_tray(rgba: &[u8], width: u32, height: u32) -> Option<TrayState> {
    use tray_icon::menu::{Menu, MenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    // On Linux, appindicator looks up icons by name (no path, no extension) inside
    // an icon-theme directory.  Write the PNG to /tmp with a fixed simple name so
    // `set_icon_theme_path(/tmp)` + `set_icon_full(pact-tray-icon)` works correctly.
    #[cfg(target_os = "linux")]
    let icon = {
        let icon_path = std::path::Path::new("/tmp/pact-tray-icon.png");
        // Write a 32×32 downscaled version to /tmp for the panel icon.
        use image::GenericImageView;
        let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
            .and_then(|i| Some(image::DynamicImage::ImageRgba8(i)));
        if let Some(img) = img {
            let small = img.resize(32, 32, image::imageops::FilterType::Lanczos3);
            let _ = small.save(icon_path);
        }
        // tray-icon will also write its own copy; override with_temp_dir_path so
        // it saves alongside ours and appindicator finds both.
        Icon::from_rgba(rgba.to_vec(), width, height).ok()?
    };
    #[cfg(not(target_os = "linux"))]
    let icon = Icon::from_rgba(rgba.to_vec(), width, height).ok()?;

    let show_item = MenuItem::new("Show Window", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let menu = Menu::new();
    let _ = menu.append(&show_item);
    let _ = menu.append(&quit_item);

    let mut builder = TrayIconBuilder::new()
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_tooltip("PACT")
        .with_title("PACT");

    // Point appindicator at /tmp so it finds pact-tray-icon.png by name.
    #[cfg(target_os = "linux")]
    {
        builder = builder.with_temp_dir_path("/tmp");
    }

    let tray = builder.build().ok()?;

    Some(TrayState {
        _icon: tray,
        show_id,
        quit_id,
    })
}

use core::topology::TopologyView;
use gui::AppCache;
use gui::custom_process_editor::CustomProcessEditor;
use gui::group_editor::GroupEditor;
use gui::process_editor::ProcessEditor;
use gui::tab_auto_mode;
use gui::tab_configure;
use gui::tab_options;
use gui::tab_status;

// ─── Application state ────────────────────────────────────────────────────────

pub struct ProcessAffinityApp {
    pub pact: core::pact_instance::PACTInstance,
    pub num_cores: usize,
    pub active_tab: gui::TabId,

    /// Cached display data; refreshed by the Tick subscription.
    pub cache: AppCache,

    /// Topology never changes — computed once at startup.
    pub topo_view: TopologyView,

    pub group_editor: Option<GroupEditor>,
    pub process_editor: Option<ProcessEditor>,
    pub custom_process_editor: Option<CustomProcessEditor>,
    pub new_launcher_name: String,
    /// Process currently being dragged; passed to DropZone widgets each frame.
    pub dragging_process: Option<String>,
    pub groups_help_open: bool,
    pub process_filter: String,
    /// Raw RGBA bytes for the app icon, used to (re)create the tray icon.
    pub icon_rgba: (Vec<u8>, u32, u32),
    /// Live tray icon; `None` when minimize-to-tray is disabled.
    pub tray_state: Option<TrayState>,
}

impl Default for ProcessAffinityApp {
    fn default() -> Self {
        let mut pact = core::pact_instance::PACTInstance::new();
        pact.start_scan_handler();
        let cache = build_cache(&pact);
        let topo_view = core::topology::get_topology().topology_view();
        let icon_rgba = load_icon_rgba();
        let minimize_to_tray = pact
            .pact_process_overwatch
            .user_config_lock()
            .minimize_to_tray;
        let tray_state = if minimize_to_tray {
            create_tray(&icon_rgba.0, icon_rgba.1, icon_rgba.2)
        } else {
            None
        };
        Self {
            num_cores: num_cpus::get(),
            pact,
            active_tab: gui::TabId::default(),
            cache,
            topo_view,
            group_editor: None,
            process_editor: None,
            custom_process_editor: None,
            new_launcher_name: String::new(),
            dragging_process: None,
            groups_help_open: false,
            process_filter: String::new(),
            icon_rgba,
            tray_state,
        }
    }
}

/// Read all display-relevant data from pact into an AppCache.
/// Called once per Tick (1 Hz) and after any mutation, never inside view().
fn build_cache(pact: &core::pact_instance::PACTInstance) -> AppCache {
    AppCache {
        is_scanner_active: pact.pact_process_overwatch.is_scanner_active(),
        is_auto_mode: pact.pact_process_overwatch.is_auto_mode(),
        groups: pact.get_groups(),
        running: pact.get_all_running_processes(),
        assigned: pact.get_assigned_processes(),
        custom_processes: pact.get_custom_processes(),
        protected_count: pact.pact_process_overwatch.protected_process_count(),
        cpu_stats: pact.pact_process_overwatch.cpu_stats(),
        launchers: pact.get_auto_mode_launchers(),
        detections: pact.get_auto_mode_detections(),
        scan_interval: pact.pact_process_overwatch.scan_interval(),
        minimize_to_tray: pact
            .pact_process_overwatch
            .user_config_lock()
            .minimize_to_tray,
    }
}

// ─── Subscription ─────────────────────────────────────────────────────────────

fn subscription(_app: &ProcessAffinityApp) -> Subscription<gui::Message> {
    let tick = iced::time::every(Duration::from_secs(1)).map(|_| gui::Message::Tick);
    let mouse_release = iced::event::listen_with(|event, _status, _id| {
        if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) =
            event
        {
            Some(gui::Message::DragReleased)
        } else {
            None
        }
    });
    let close_req = iced::window::close_requests().map(|_| gui::Message::CloseRequested);
    // Poll tray events every 250 ms (also used when tray is inactive — no-op cost).
    let tray_poll =
        iced::time::every(Duration::from_millis(250)).map(|_| gui::Message::PollTrayEvents);
    Subscription::batch([tick, mouse_release, close_req, tray_poll])
}

// ─── Update ───────────────────────────────────────────────────────────────────

fn update(app: &mut ProcessAffinityApp, message: gui::Message) -> Task<gui::Message> {
    match message {
        gui::Message::Tick => {
            app.cache = build_cache(&app.pact);
        }
        gui::Message::TabSelected(tab) => {
            app.active_tab = tab;
        }
        gui::Message::ImportConfig => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
            {
                app.pact.import_config(p.to_string_lossy().as_ref());
                app.cache = build_cache(&app.pact);
            }
        }
        gui::Message::ExportConfig => {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .save_file()
            {
                app.pact.export_config(p.to_string_lossy().as_ref());
            }
        }
        gui::Message::Exit => {
            app.pact.stop_scan_handler();
        }
        gui::Message::StatusMessage(msg) => {
            match msg {
                gui::tab_status::Message::ToggleScanner => {
                    app.pact.toggle_process_overwatch();
                }
                gui::tab_status::Message::ToggleAutoMode => {
                    app.pact.toggle_auto_mode();
                }
                gui::tab_status::Message::RequestFreshScan => {
                    app.pact.request_fresh_scan();
                }
            }
            app.cache = build_cache(&app.pact);
        }
        gui::Message::ConfigureMessage(msg) => {
            match msg {
                gui::tab_configure::Message::AssignProcess(proc, group) => {
                    app.pact.assign_process(&proc, &group);
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_configure::Message::OpenGroupEditor(name_opt) => {
                    let groups = app.pact.get_groups();
                    app.group_editor = Some(GroupEditor::new(
                        name_opt
                            .as_ref()
                            .and_then(|name| groups.iter().find(|g| g.name == *name)),
                        app.num_cores,
                    ));
                }
                gui::tab_configure::Message::OpenProcessEditor(existing_name, group_name) => {
                    let group_names: Vec<String> = app
                        .pact
                        .get_groups()
                        .iter()
                        .map(|g| g.name.clone())
                        .collect();
                    app.process_editor = Some(match existing_name {
                        Some(name) => ProcessEditor::new_for_edit(name, group_name, group_names),
                        None => ProcessEditor::new_for_add(group_name, group_names),
                    });
                }
                gui::tab_configure::Message::UpdateProcessFilter(s) => {
                    app.process_filter = s;
                }
                gui::tab_configure::Message::OpenProcessEditorGlobal => {
                    let group_names: Vec<String> = app
                        .pact
                        .get_groups()
                        .iter()
                        .map(|g| g.name.clone())
                        .collect();
                    if !group_names.is_empty() {
                        app.process_editor =
                            Some(ProcessEditor::new_for_add(String::new(), group_names));
                    }
                }
                gui::tab_configure::Message::OpenCustomProcessEditor(name_opt) => {
                    let custom_procs = app.pact.get_custom_processes();
                    let existing = name_opt.as_ref().and_then(|n| {
                        custom_procs
                            .iter()
                            .find(|cp| cp.name.eq_ignore_ascii_case(n))
                    });
                    app.custom_process_editor =
                        Some(CustomProcessEditor::new(existing, app.num_cores));
                }
                gui::tab_configure::Message::DragStarted(name) => {
                    app.dragging_process = Some(name);
                }
                gui::tab_configure::Message::DropOnGroup(proc_name, group_name) => {
                    // Remove any custom-process entry for this name (drag from custom → group).
                    app.pact.remove_custom_process(&proc_name);
                    app.pact.assign_process(&proc_name, &group_name);
                    app.dragging_process = None;
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_configure::Message::DropOnRunning(proc_name) => {
                    // Remove from group assignment and/or custom processes.
                    app.pact.unassign_process(&proc_name);
                    app.pact.remove_custom_process(&proc_name);
                    app.dragging_process = None;
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_configure::Message::DropOnCustom(proc_name) => {
                    // Remove any group assignment first (drag from group → custom).
                    app.pact.unassign_process(&proc_name);
                    // Open the editor as a NEW entry (editing_name="") with the name pre-filled.
                    // Using Some(&stub) would set editing_name=proc_name, causing update_custom_process
                    // to be called on a non-existent entry instead of add_custom_process.
                    let mut editor = CustomProcessEditor::new(None, app.num_cores);
                    editor.name = proc_name;
                    app.custom_process_editor = Some(editor);
                    app.dragging_process = None;
                }
            }
        }
        gui::Message::AutoModeMessage(msg) => {
            match msg {
                gui::tab_auto_mode::Message::ToggleAutoMode => {
                    app.pact.toggle_auto_mode();
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_auto_mode::Message::AddLauncher(name) => {
                    app.pact.add_to_auto_mode_launchers(&name);
                    app.new_launcher_name = String::new();
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_auto_mode::Message::RemoveLauncher(name) => {
                    app.pact.remove_from_auto_mode_launchers(&name);
                    app.cache = build_cache(&app.pact);
                }
                gui::tab_auto_mode::Message::UpdateNewLauncherName(name) => {
                    app.new_launcher_name = name;
                    // No cache rebuild — pure UI state change, does not affect PACT data.
                }
            }
        }
        gui::Message::OptionsMessage(msg) => {
            match msg {
                gui::tab_options::Message::SetScanInterval(ms) => {
                    app.pact.pact_process_overwatch.set_scan_interval(ms as u64);
                }
                gui::tab_options::Message::ResetConfig => {
                    app.pact.reset_config();
                }
                gui::tab_options::Message::OpenGitHub => {
                    let _ =
                        open::that("https://github.com/sas41/ProcessAffinityControlTool#readme");
                }
                gui::tab_options::Message::SetMinimizeToTray(enabled) => {
                    app.pact
                        .pact_process_overwatch
                        .user_config_lock_mut()
                        .minimize_to_tray = enabled;
                    core::pact_instance::PACTInstance::save_config(
                        &app.pact.pact_process_overwatch.user_config_lock(),
                    );
                    // Create or drop the tray icon accordingly.
                    if enabled && app.tray_state.is_none() {
                        app.tray_state =
                            create_tray(&app.icon_rgba.0, app.icon_rgba.1, app.icon_rgba.2);
                    } else if !enabled {
                        app.tray_state = None;
                    }
                }
            }
            app.cache = build_cache(&app.pact);
        }
        gui::Message::ProcessEditorMessage(msg) => {
            if let Some(mut editor) = app.process_editor.take() {
                editor.update(msg);
                if !editor.open {
                    if editor.remove_requested {
                        app.pact.unassign_process(&editor.editing_process_name);
                    } else if let Some((name, group)) = editor.result {
                        app.pact.assign_process(&name, &group);
                    }
                    app.cache = build_cache(&app.pact);
                } else {
                    app.process_editor = Some(editor);
                }
            }
        }
        gui::Message::CustomProcessEditorMessage(msg) => {
            if let Some(mut editor) = app.custom_process_editor.take() {
                editor.update(msg, app.num_cores);
                if !editor.open {
                    if editor.delete_requested {
                        app.pact.remove_custom_process(&editor.editing_name);
                    } else if let Some(cp) = editor.result {
                        if editor.editing_name.is_empty() {
                            app.pact.add_custom_process(cp);
                        } else {
                            app.pact.update_custom_process(&editor.editing_name, cp);
                        }
                    }
                    app.cache = build_cache(&app.pact);
                } else {
                    app.custom_process_editor = Some(editor);
                }
            }
        }
        gui::Message::GroupEditorMessage(msg) => {
            if let Some(mut editor) = app.group_editor.take() {
                editor.update(msg, app.num_cores);
                if !editor.open {
                    let editing_name = editor.editing_name.clone();
                    if let Some(new_group) = editor.result {
                        if editing_name.is_empty() {
                            app.pact.add_group(new_group);
                        } else {
                            app.pact.update_group(&editing_name, new_group);
                        }
                    }
                    if editor.delete_requested {
                        app.pact.delete_group(&editing_name);
                    }
                    app.cache = build_cache(&app.pact);
                } else {
                    app.group_editor = Some(editor);
                }
            }
        }
        gui::Message::DragReleased => {
            app.dragging_process = None;
        }
        gui::Message::CloseRequested => {
            if app.cache.minimize_to_tray {
                // Minimize first (signals the compositor), then hide the surface
                // so the window disappears from both view and the taskbar.
                return iced::window::get_latest().and_then(|id| {
                    Task::batch([
                        iced::window::minimize(id, true),
                        iced::window::change_mode(id, iced::window::Mode::Hidden),
                    ])
                });
            } else {
                app.pact.stop_scan_handler();
                return iced::window::get_latest().and_then(iced::window::close);
            }
        }
        gui::Message::PollTrayEvents => {
            // Drive the GTK main loop on Linux so tray/menu events are dispatched.
            #[cfg(target_os = "linux")]
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }

            if let Some(tray) = &app.tray_state {
                // Left/double click restores the window (Windows/macOS).
                if let Ok(ev) = tray_icon::TrayIconEvent::receiver().try_recv() {
                    if matches!(
                        ev.click_type,
                        tray_icon::ClickType::Left | tray_icon::ClickType::Double
                    ) {
                        return iced::window::get_latest().and_then(|id| {
                            iced::window::change_mode(id, iced::window::Mode::Windowed)
                        });
                    }
                }
                // Context-menu items (Linux / all platforms).
                if let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if ev.id == tray.show_id {
                        return iced::window::get_latest().and_then(|id| {
                            iced::window::change_mode(id, iced::window::Mode::Windowed)
                        });
                    } else if ev.id == tray.quit_id {
                        app.pact.stop_scan_handler();
                        return iced::window::get_latest().and_then(iced::window::close);
                    }
                }
            }
        }
        gui::Message::ShowGroupsHelp => {
            app.groups_help_open = true;
        }
        gui::Message::HideGroupsHelp => {
            app.groups_help_open = false;
        }
        gui::Message::GroupEditorResult(_) => {}
        gui::Message::GroupEditorDelete(_) => {}
        gui::Message::GroupEditorClosed => {
            app.group_editor = None;
        }
    }
    Task::none()
}

// ─── View ─────────────────────────────────────────────────────────────────────

fn view_groups_help() -> Element<'static, gui::Message> {
    use iced::widget::Space;

    fn section(
        title: &'static str,
        body: &'static str,
    ) -> iced::widget::Column<'static, gui::Message> {
        column![
            text(title).size(14).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
            text(body).size(13).color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(4)
    }

    let content = column![
        text("How Groups Work").size(18).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
        Space::with_height(4),
        text("Groups let you apply CPU affinity and/or priority settings to a set of processes. Assign processes to a group and they will be managed automatically on each scan.").size(13).color(Color::from_rgb(0.75, 0.75, 0.75)),
        Space::with_height(8),
        section("Affinity", "Restricts which CPU cores a group's processes may run on. Useful for isolating workloads to P-cores, E-cores, or a specific CCD."),
        section("Priority", "Sets the OS scheduling priority for processes in the group. Higher priority means more CPU time relative to other processes."),
        section("Default Group", "Processes not explicitly assigned to any group automatically land here. Only one group can be the default at a time."),
        section("Blacklist", "Processes assigned to a blacklist group are skipped entirely — no affinity or priority changes are applied."),
        section("Custom Processes", "Individual processes with their own affinity and priority, independent of any group."),
        section("Drag & Drop", "Drag any pill from Running Processes onto a group card to assign it, or onto the Custom Processes area to configure it individually.\nDragging any pill to the Running Processes area will remove it from it's group."),
        Space::with_height(8),
        button(text("Close").size(13))
            .on_press(gui::Message::HideGroupsHelp)
            .padding([5, 14]),
    ]
    .spacing(12)
    .padding(24);

    let dialog = container(scrollable(content).height(Length::Shrink))
        .max_width(500)
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

fn view(app: &ProcessAffinityApp) -> Element<'_, gui::Message> {
    let tab_bar = TabBar::new(gui::Message::TabSelected)
        .push(gui::TabId::Status, TabLabel::Text("Status".to_string()))
        .push(
            gui::TabId::Configure,
            TabLabel::Text("Configure".to_string()),
        )
        .push(
            gui::TabId::AutoMode,
            TabLabel::Text("Auto Mode".to_string()),
        )
        .push(gui::TabId::Options, TabLabel::Text("Options".to_string()))
        .set_active_tab(&app.active_tab)
        .tab_width(iced::Length::Shrink)
        .text_size(13.0)
        .padding(iced::Padding::from([5, 14]));

    let content: Element<'_, gui::Message> = match &app.active_tab {
        gui::TabId::Status => tab_status::view(&app.cache, &app.topo_view, app.num_cores).into(),
        gui::TabId::Configure => tab_configure::view(
            &app.cache,
            app.dragging_process.as_deref(),
            &app.process_filter,
        )
        .into(),
        gui::TabId::AutoMode => tab_auto_mode::view(&app.cache, &app.new_launcher_name).into(),
        gui::TabId::Options => tab_options::view(&app.cache, app.num_cores).into(),
    };

    let main_layout = column![tab_bar, content];

    if let Some(editor) = &app.group_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view(app.num_cores))
            .into()
    } else if let Some(editor) = &app.process_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view())
            .into()
    } else if let Some(editor) = &app.custom_process_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view())
            .into()
    } else if app.groups_help_open {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(view_groups_help())
            .into()
    } else {
        main_layout.into()
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() -> iced::Result {
    // tray-icon on Linux uses GTK/libappindicator; initialize GTK before iced
    // takes over the main thread, then pump events during our periodic poll.
    #[cfg(target_os = "linux")]
    gtk::init().expect("Failed to initialize GTK (required for system tray)");

    let settings = Settings {
        default_text_size: 14.into(),
        ..Default::default()
    };

    let (rgba, w, h) = load_icon_rgba();
    let window_icon = iced::window::icon::from_rgba(rgba, w, h).ok();

    iced::application("Process Affinity Control Tool", update, view)
        .settings(settings)
        .subscription(subscription)
        .font(iced_fonts::BOOTSTRAP_FONT_BYTES)
        .window(iced::window::Settings {
            min_size: Some(iced::Size::new(700.0, 560.0)),
            // Handle close manually so minimize-to-tray can intercept it.
            exit_on_close_request: false,
            icon: window_icon,
            ..Default::default()
        })
        .run()
}
