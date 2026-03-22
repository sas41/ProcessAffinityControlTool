#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

/// Core application modules.
// `mod` declares a source module (similar to a C# namespace/file being brought into this crate).
mod core;

/// GUI modules and widgets.
mod gui;

// `use` imports names; `::` is namespace/type path navigation like C# `.`.
use std::time::Duration;

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::path::PathBuf;

use core::elevation::is_elevated;
use core::topology::{
    topology_classification_label, topology_details_report, topology_runtime_info, TopologyView,
};
use gui::custom_process_editor::CustomProcessEditor;
use gui::group_editor::GroupEditor;
use gui::process_editor::ProcessEditor;
use gui::tab_configure;
use gui::tab_options;
use gui::tab_status;
use gui::AppCache;
use iced::widget::tooltip;
use iced::widget::tooltip::Position as TooltipPosition;
use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, stack, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Settings, Subscription, Task};
use iced_aw::{TabBar, TabLabel};

/// Embedded PNG bytes for the app icon.
// `&[u8]` is a borrowed byte slice (read-only view), not an owned array.
// `!` calls a macro (compile-time code expansion, roughly like a Roslyn source helper).
const ICON_PNG: &[u8] = include_bytes!("../assets/icon/PACT Logo.png");

/// Decode the embedded PNG icon to RGBA bytes and dimensions.
// `Vec<u8>` uses Rust generics (`<>`), like C# `List<byte>` type arguments.
fn load_icon_rgba() -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(ICON_PNG)
        .expect("Failed to load PACT icon")
        .to_rgba8();
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

/// Hold the live tray icon and menu IDs.
// `pub(crate)` exposes this type inside the current crate, similar to C# `internal`.
pub(crate) struct TrayState {
    /// Keep the icon alive for the tray lifetime.
    _icon: tray_icon::TrayIcon,
    /// ID for the "Show Window" menu item.
    show_id: tray_icon::menu::MenuId,
    /// ID for the "Quit" menu item.
    quit_id: tray_icon::menu::MenuId,
}

/// Create the tray icon and menu.
// `Option<T>` is Rust's nullable container: `Some(value)` or `None` (like nullable/optional values).
fn create_tray(rgba: &[u8], width: u32, height: u32) -> Option<TrayState> {
    use tray_icon::menu::{Menu, MenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    // On Linux appindicator expects an icon file in /tmp.
    #[cfg(target_os = "linux")]
    let icon = {
        let icon_path = std::path::Path::new("/tmp/pact-tray-icon.png");
        let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
            .map(image::DynamicImage::ImageRgba8);
        // `if let` is pattern-based conditional unpacking (like `if (x is T v)`).
        if let Some(img) = img {
            let small = img.resize(32, 32, image::imageops::FilterType::Lanczos3);
            let _ = small.save(icon_path);
        }
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
    let builder = TrayIconBuilder::new()
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_tooltip("PACT")
        .with_title("PACT");

    // #[cfg(target_os = "linux")]
    // {
    //     builder = builder.with_temp_dir_path("/tmp");
    // }

    let tray = builder.build().ok()?;
    Some(TrayState {
        _icon: tray,
        show_id,
        quit_id,
    })
}

/// Store all UI and runtime state for the app.
// `pub` makes items visible to other modules/crates (similar to C# `public`).
pub struct ProcessAffinityApp {
    /// Core process-affinity runtime.
    pub pact: core::pact_instance::PACTInstance,

    /// Logical CPU count.
    pub num_cores: usize,

    /// Currently selected tab.
    pub active_tab: gui::TabId,

    /// Cached data consumed by tab views.
    pub cache: AppCache,

    /// CPU topology snapshot loaded at startup.
    pub topo_view: TopologyView,

    /// Whether the process currently has elevated privileges.
    pub is_elevated: bool,

    /// Whether to show inaccessible-process list overlay.
    pub inaccessible_list_open: bool,

    /// Whether to show topology-details overlay.
    pub topology_details_open: bool,

    /// Label describing how topology grouping was classified.
    pub topology_classification_label: String,

    /// Runtime environment details used in topology report UI.
    pub topology_os_label: String,
    pub topology_hypervisor_on: bool,
    pub topology_accuracy_warnings: Vec<String>,

    /// Detailed topology report rendered in the topology-details modal.
    pub topology_details_report: String,

    /// Flash state for copy action in topology modal.
    pub topology_copy_notice_ticks: u8,

    /// Multiplier used to duplicate topology groups for layout testing.
    pub topology_group_repeat: usize,

    /// Group editor modal state.
    pub group_editor: Option<GroupEditor>,

    /// Process editor modal state.
    pub process_editor: Option<ProcessEditor>,

    /// Custom process editor modal state.
    pub custom_process_editor: Option<CustomProcessEditor>,

    /// Process currently being dragged.
    pub dragging_process: Option<String>,

    /// Whether the groups-help overlay is visible.
    pub groups_help_open: bool,

    /// Whether child processes are shown in the Configure tab tree views.
    pub show_children: bool,

    /// Process-list filter text.
    pub process_filter: String,

    /// Animation phase for Configure search pulse.
    pub search_pulse_phase: f32,

    /// RGBA icon bytes and dimensions.
    pub icon_rgba: (Vec<u8>, u32, u32),

    /// Live tray handle when available.
    pub(crate) tray_state: Option<TrayState>,

    /// Track currently open UI windows to prevent duplicates.
    pub window_ids: Vec<iced::window::Id>,

    /// True while a window open request has been issued but not yet observed.
    pub window_open_pending: bool,
}

// `impl Trait for Type` defines a trait implementation (similar to implementing a C# interface/base contract).
impl Default for ProcessAffinityApp {
    /// Build the initial application state.
    fn default() -> Self {
        let mut pact = core::pact_instance::PACTInstance::new();
        pact.start_scan_handler();
        let cache = build_cache(&pact);
        let topo_view = core::topology::get_topology().topology_view();
        let runtime_info = topology_runtime_info();
        let icon_rgba = load_icon_rgba();
        let tray_state = create_tray(&icon_rgba.0, icon_rgba.1, icon_rgba.2);
        Self {
            num_cores: num_cpus::get(),
            pact,
            active_tab: gui::TabId::default(),
            cache,
            topo_view,
            is_elevated: is_elevated(),
            inaccessible_list_open: false,
            topology_details_open: false,
            topology_classification_label: topology_classification_label().to_string(),
            topology_os_label: runtime_info.os_label,
            topology_hypervisor_on: runtime_info.hypervisor_on,
            topology_accuracy_warnings: runtime_info.accuracy_warnings,
            topology_details_report: topology_details_report(),
            topology_copy_notice_ticks: 0,
            topology_group_repeat: 1,
            group_editor: None,
            process_editor: None,
            custom_process_editor: None,
            dragging_process: None,
            groups_help_open: false,
            show_children: true,
            process_filter: String::new(),
            search_pulse_phase: 0.0,
            icon_rgba,
            tray_state,
            window_ids: Vec::new(),
            window_open_pending: false,
        }
    }
}

/// Build an `AppCache` snapshot from the current runtime state.
fn build_cache(pact: &core::pact_instance::PACTInstance) -> AppCache {
    AppCache {
        is_scanner_active: pact.pact_process_overwatch.is_scanner_active(),
        is_elevated: is_elevated(),
        groups: pact.get_groups(),
        running: pact.get_all_running_processes(),
        assigned: pact.get_assigned_processes(),
        custom_processes: pact.get_custom_processes(),
        protected_count: pact.pact_process_overwatch.protected_process_count(),
        protected_names: pact.get_protected_processes(),
        managed_count: pact.pact_process_overwatch.managed_process_count(),
        cpu_stats: pact.pact_process_overwatch.cpu_stats(),
        scan_interval: pact.pact_process_overwatch.scan_interval(),
        launch_minimized: pact.launch_minimized(),
        child_process_parents: pact
            .pact_process_overwatch
            .capture_sub_processes_data()
            .into_iter()
            .map(|(child, parent_lc, _group)| (child.to_lowercase(), parent_lc))
            .collect(),
    }
}

/// Register periodic and event-driven subscriptions.
fn subscription(_app: &ProcessAffinityApp) -> Subscription<gui::Message> {
    // `|_| ...` is a closure (lambda); `_` ignores the input parameter.
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
    let close_req = iced::window::close_requests().map(gui::Message::CloseRequested);
    let window_opened = iced::window::open_events().map(gui::Message::WindowOpened);
    let window_closed = iced::window::close_events().map(gui::Message::WindowClosed);
    let tray_poll =
        iced::time::every(Duration::from_millis(250)).map(|_| gui::Message::PollTrayEvents);
    let search_pulse =
        iced::time::every(Duration::from_millis(33)).map(|_| gui::Message::SearchPulse);
    Subscription::batch([
        tick,
        mouse_release,
        close_req,
        window_opened,
        window_closed,
        tray_poll,
        search_pulse,
    ])
}

fn main_window_settings(icon: Option<iced::window::Icon>) -> iced::window::Settings {
    #[cfg(target_os = "linux")]
    let mut settings = iced::window::Settings {
        size: iced::Size::new(1024.0, 568.0),
        min_size: Some(iced::Size::new(700.0, 560.0)),
        exit_on_close_request: false,
        icon,
        ..Default::default()
    };

    #[cfg(not(target_os = "linux"))]
    let settings = iced::window::Settings {
        size: iced::Size::new(1024.0, 568.0),
        min_size: Some(iced::Size::new(700.0, 560.0)),
        exit_on_close_request: false,
        icon,
        ..Default::default()
    };

    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id =
            "com.sas41.processaffinitycontroltool".to_string();
    }

    settings
}

fn open_or_focus_main_window(app: &mut ProcessAffinityApp) -> Task<gui::Message> {
    if let Some(&existing) = app.window_ids.last() {
        return iced::window::gain_focus(existing);
    }

    if app.window_open_pending {
        return Task::none();
    }

    let icon =
        iced::window::icon::from_rgba(app.icon_rgba.0.clone(), app.icon_rgba.1, app.icon_rgba.2)
            .ok();
    let (_, open_task) = iced::window::open(main_window_settings(icon));
    app.window_open_pending = true;
    open_task.map(gui::Message::WindowOpened)
}

#[cfg(target_os = "windows")]
fn enforce_single_instance() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    let name: Vec<u16> = "Global\\ProcessAffinityControlTool.Singleton"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = match CreateMutexW(None, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => handle,
            Err(_) => return true,
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return false;
        }

        true
    }
}

#[cfg(target_os = "linux")]
fn lock_file_path() -> Option<PathBuf> {
    let mut dir = dirs::runtime_dir().or_else(dirs::data_local_dir)?;
    dir.push("process_affinity_control_tool.lock");
    Some(dir)
}

#[cfg(target_os = "linux")]
fn enforce_single_instance() -> bool {
    use std::fs::OpenOptions;

    let Some(path) = lock_file_path() else {
        return true;
    };

    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return true,
    };

    let fd = file.as_raw_fd();
    unsafe {
        if libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) != 0 {
            return false;
        }
    }

    std::mem::forget(file);
    true
}

#[cfg(target_os = "macos")]
fn lock_file_path() -> Option<PathBuf> {
    let mut dir = dirs::runtime_dir().or_else(dirs::data_local_dir)?;
    dir.push("process_affinity_control_tool.lock");
    Some(dir)
}

#[cfg(target_os = "macos")]
fn enforce_single_instance() -> bool {
    use std::fs::OpenOptions;

    let Some(path) = lock_file_path() else {
        return true;
    };

    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return true,
    };

    let fd = file.as_raw_fd();
    unsafe {
        if libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) != 0 {
            return false;
        }
    }

    std::mem::forget(file);
    true
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn enforce_single_instance() -> bool {
    true
}

/// Central message dispatcher for the UI event loop.
///
/// Iced delivers `gui::Message` values from widgets/subscriptions; this function acts
/// like a reducer and returns an optional async `Task` for follow-up work.
fn update(app: &mut ProcessAffinityApp, message: gui::Message) -> Task<gui::Message> {
    // `match` is an exhaustive pattern switch (closest C# equivalent: `switch` expression/statement).
    // `&mut` is an exclusive mutable borrow (temporary writable reference).
    match message {
        gui::Message::Tick => {
            app.cache = build_cache(&app.pact);
            if app.topology_copy_notice_ticks > 0 {
                app.topology_copy_notice_ticks -= 1;
            }
        }

        gui::Message::SearchPulse => {
            app.search_pulse_phase = (app.search_pulse_phase + 0.14) % std::f32::consts::TAU;
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
                gui::tab_status::Message::OpenInaccessibleList => {
                    app.inaccessible_list_open = true;
                }
                gui::tab_status::Message::OpenTopologyDetails => {
                    app.topology_details_open = true;
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
                    // We clone group names into owned `String`s because the editor stores them
                    // beyond this function call; borrowing from a temporary vector would dangle.
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
                    // Move from custom to group when dropping on a group card.
                    app.pact.remove_custom_process(&proc_name);
                    app.pact.assign_process(&proc_name, &group_name);
                    app.dragging_process = None;
                    app.cache = build_cache(&app.pact);
                }

                gui::tab_configure::Message::DropOnRunning(proc_name) => {
                    app.pact.unassign_process(&proc_name);
                    app.pact.remove_custom_process(&proc_name);
                    app.dragging_process = None;
                    app.cache = build_cache(&app.pact);
                }

                gui::tab_configure::Message::DropOnCustom(proc_name) => {
                    app.pact.unassign_process(&proc_name);
                    // Force "add new" by opening with `None` and pre-filling the name.
                    let mut editor = CustomProcessEditor::new(None, app.num_cores);
                    editor.name = proc_name;

                    app.custom_process_editor = Some(editor);
                    app.dragging_process = None;
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

                gui::tab_options::Message::SetLaunchMinimized(enabled) => {
                    app.pact.set_launch_minimized(enabled);
                }
            }
            app.is_elevated = is_elevated();
            app.cache = build_cache(&app.pact);
        }

        gui::Message::Noop => {}

        gui::Message::OpenInaccessibleList => {
            app.inaccessible_list_open = true;
        }

        gui::Message::CloseInaccessibleList => {
            app.inaccessible_list_open = false;
        }

        gui::Message::OpenTopologyDetails => {
            app.topology_details_open = true;
            app.topology_copy_notice_ticks = 0;
        }

        gui::Message::CloseTopologyDetails => {
            app.topology_details_open = false;
            app.topology_copy_notice_ticks = 0;
        }

        gui::Message::CopyTopologyDetails => {
            return iced::clipboard::write(app.topology_details_report.clone())
                .map(|_: ()| gui::Message::TopologyDetailsCopied);
        }

        gui::Message::TopologyDetailsCopied => {
            app.topology_copy_notice_ticks = 2;
        }

        gui::Message::ProcessEditorMessage(msg) => {
            // `take()` moves the editor out of `app` so we can mutate it freely, then either
            // commit its result or place it back. This is a common ownership pattern in Rust UIs.
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

        gui::Message::CloseRequested(id) => {
            // Close only the current window; the daemon keeps running in the tray.
            return iced::window::close(id);
        }

        gui::Message::WindowOpened(id) => {
            app.window_open_pending = false;
            if !app.window_ids.contains(&id) {
                app.window_ids.push(id);
            }
        }

        gui::Message::WindowClosed(id) => {
            app.window_open_pending = false;
            app.window_ids.retain(|&wid| wid != id);
        }

        gui::Message::PollTrayEvents => {
            // Pump GTK events so Linux tray/menu interactions are delivered.
            #[cfg(target_os = "linux")]
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }

            if let Some(tray) = &app.tray_state {
                // A single user interaction can emit multiple tray/menu events on Windows.
                // Coalesce them into one intent so we do not open duplicate windows.
                let mut request_open_window = false;
                let mut request_quit = false;

                // `try_recv()` is non-blocking, so polling tray events does not stall UI updates.
                while let Ok(ev) = tray_icon::TrayIconEvent::receiver().try_recv() {
                    if matches!(
                        ev,
                        tray_icon::TrayIconEvent::Click {
                            button: tray_icon::MouseButton::Left,
                            button_state: tray_icon::MouseButtonState::Up,
                            ..
                        } | tray_icon::TrayIconEvent::DoubleClick {
                            button: tray_icon::MouseButton::Left,
                            ..
                        }
                    ) {
                        request_open_window = true;
                    }
                }

                while let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if ev.id == tray.show_id {
                        request_open_window = true;
                    } else if ev.id == tray.quit_id {
                        request_quit = true;
                    }
                }

                if request_quit {
                    app.pact.stop_scan_handler();
                    return iced::exit();
                }

                if request_open_window {
                    // Execute one open/focus action per poll tick even if multiple matching
                    // events were drained above.
                    return open_or_focus_main_window(app);
                }
            }
        }

        gui::Message::ShowGroupsHelp => {
            app.groups_help_open = true;
        }
        gui::Message::HideGroupsHelp => {
            app.groups_help_open = false;
        }
        gui::Message::ToggleShowChildren => {
            app.show_children = !app.show_children;
        }

        gui::Message::GroupEditorResult(_) => {}
        gui::Message::GroupEditorDelete(_) => {}
        gui::Message::GroupEditorClosed => {
            app.group_editor = None;
        }
    }

    Task::none()
}

/// Build the groups-help overlay.
fn view_groups_help() -> Element<'static, gui::Message> {
    // `'static` is a lifetime: data/reference validity scope (here, valid for the program lifetime).
    use iced::widget::Space;

    /// Build one titled help section.
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

    let icon_legend = column![
        text("Icons").size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        row![
            iced_fonts::bootstrap::award_fill()
                .size(13)
                .color(Color::from_rgb(0.43, 0.73, 1.0)),
            text("Default group or default-assigned process")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            iced_fonts::bootstrap::slash_circle_fill()
                .size(13)
                .color(Color::from_rgb(1.0, 0.44, 0.44)),
            text("Emergent blacklist — group applies no affinity or priority")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            iced_fonts::bootstrap::cpu_fill()
                .size(13)
                .color(Color::from_rgb(0.55, 0.94, 0.72)),
            text("Group has custom process affinity (cores/threads)")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            iced_fonts::bootstrap::stars()
                .size(13)
                .color(Color::from_rgb(0.92, 0.68, 1.0)),
            text("Group has custom process priority")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            iced_fonts::bootstrap::exclamation_triangle_fill()
                .size(13)
                .color(Color::from_rgb(0.95, 0.84, 0.20)),
            text("Process has both accessible and inaccessible instances")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            iced_fonts::bootstrap::diagram_two_fill()
                .size(13)
                .color(Color::from_rgb(0.55, 0.85, 1.0)),
            text("Group or custom process has Capture Sub-Processes enabled")
                .size(13)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(4);

    let content = column![
        section("Groups", "Groups allow for controlled assignment of cores and priority for a given set of processes."),
        section("Search", "The top search box filters all three areas at once: Groups, Running Processes, and Custom Processes. Configure lists are deduplicated by name and accessibility."),
        section("Drag & Drop", "Drag any process from Running Processes onto a group card to assign it, or onto the Custom Processes area to configure it individually.\nDragging any pill to the Running Processes area will remove it from its group."),
        section("Process Colors", "Bright gray pills are currently running.\nDim gray pills are currently not running.\nRed-background pills are inaccessible (permission-limited), so affinity/priority changes could not be applied.\nA name appears twice only when both accessible and inaccessible instances are present."),
        icon_legend,
        section("Affinity", "Restricts which CPU cores a group's processes may run on. Useful for isolating workloads to P-cores, E-cores, or a specific CCD."),
        section("Priority / Niceness", "Priority buttons are scheduling presets. On Linux, niceness is also supported directly in editors. If a process is inaccessible, priority/niceness changes may be skipped."),
        section("Default Group", "Processes not explicitly assigned to any group automatically land here. Only one group can be the default at a time. These assignments are ephemeral and are not written into config."),
        section("Blacklist (emergent)", "A group with neither Set CPU Affinity nor Set Priority enabled acts as a blacklist — assigned processes are tracked but no settings are changed. The slash icon appears automatically; there is no explicit blacklist toggle."),
        section("Custom Processes", "Individual processes with their own affinity and priority, independent of any group."),
        section("Capture Sub-Processes", "When enabled on a group or custom process, any process whose parent chain includes a configured process automatically inherits its affinity and priority settings. Resolution priority: Explicit assignment → Capture Sub-Processes inheritance → Default group. For example, add Steam to a group and enable Capture Sub-Processes — every game or subprocess Steam spawns will follow that group's settings unless they have their own explicit assignment."),
        Space::new().height(8),
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

    opaque(stack![
        mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
                    ..Default::default()
                })
        )
        .on_press(gui::Message::HideGroupsHelp),
        mouse_area(
            container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
        )
        .on_press(gui::Message::Noop),
    ])
    .into()
}

fn view_inaccessible_processes_modal(names: Vec<String>) -> Element<'static, gui::Message> {
    let list_text = if names.is_empty() {
        "No inaccessible processes detected.".to_string()
    } else {
        names.join("\n")
    };

    let content = column![
        text("Inaccessible Processes").size(18).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        text("Processes that could not be modified due to permission limits.")
            .size(13)
            .color(Color::from_rgb(0.75, 0.75, 0.75)),
        container(
            scrollable(text(list_text).size(12).width(Length::Fill),)
                .width(Length::Fill)
                .height(Length::Fixed(260.0)),
        )
        .width(Length::Fill)
        .padding(10)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.10))),
            border: Border {
                color: Color::from_rgb(0.30, 0.30, 0.30),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }),
        row![button(text("Close").size(13)).on_press(gui::Message::CloseInaccessibleList),]
            .spacing(10)
            .align_y(Alignment::Center),
    ]
    .spacing(12)
    .padding(24);

    let dialog = container(content)
        .max_width(620)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.14, 0.14, 0.14))),
            border: Border {
                color: Color::from_rgb(0.38, 0.38, 0.38),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    opaque(stack![
        mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
                    ..Default::default()
                })
        )
        .on_press(gui::Message::CloseInaccessibleList),
        mouse_area(
            container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
        )
        .on_press(gui::Message::Noop),
    ])
    .into()
}

fn view_topology_details_modal(
    classification_label: String,
    os_label: String,
    hypervisor_on: bool,
    report: String,
    copied_notice: bool,
) -> Element<'static, gui::Message> {
    let content = column![
        text("Topology Details").size(18).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        text(format!("Classification: {classification_label}"))
            .size(13)
            .color(Color::from_rgb(0.75, 0.75, 0.75)),
        text(format!("OS: {os_label}"))
            .size(13)
            .color(Color::from_rgb(0.75, 0.75, 0.75)),
        text(format!(
            "Hypervisor: {}",
            if hypervisor_on { "On" } else { "Off" }
        ))
        .size(13)
        .color(Color::from_rgb(0.75, 0.75, 0.75)),
        text(if copied_notice {
            "Copied report to clipboard"
        } else {
            ""
        })
        .size(12)
        .color(Color::from_rgb(0.55, 0.86, 0.64)),
        container(
            scrollable(text(report).size(12).width(Length::Fill),)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .padding(10)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.10, 0.10, 0.10))),
            border: Border {
                color: Color::from_rgb(0.30, 0.30, 0.30),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }),
        row![
            button(text("Copy").size(13)).on_press(gui::Message::CopyTopologyDetails),
            button(text("Close").size(13)).on_press(gui::Message::CloseTopologyDetails),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    ]
    .spacing(12)
    .padding(24);

    let dialog = container(scrollable(content).height(Length::Shrink))
        .max_width(820)
        .max_height(700)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.14, 0.14, 0.14))),
            border: Border {
                color: Color::from_rgb(0.38, 0.38, 0.38),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    opaque(stack![
        mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
                    ..Default::default()
                })
        )
        .on_press(gui::Message::CloseTopologyDetails),
        mouse_area(
            container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .width(Length::Fill)
            .height(Length::Fill)
        )
        .on_press(gui::Message::Noop),
    ])
    .into()
}

/// Render the app view for the active tab and overlays.
fn view(app: &ProcessAffinityApp) -> Element<'_, gui::Message> {
    let tab_bar = TabBar::new(gui::Message::TabSelected)
        .push(gui::TabId::Status, TabLabel::Text("Status".to_string()))
        .push(
            gui::TabId::Configure,
            TabLabel::Text("Configure".to_string()),
        )
        .push(gui::TabId::Options, TabLabel::Text("Options".to_string()))
        .set_active_tab(&app.active_tab)
        .tab_width(iced::Length::Fixed(112.0))
        .text_size(13.0)
        .padding(iced::Padding::from([5, 14]));

    let mode_label = if app.cache.is_elevated {
        text("Elevated Mode")
            .size(13)
            .color(Color::from_rgb(0.70, 0.92, 0.70))
    } else {
        text("User Mode")
            .size(13)
            .color(Color::from_rgb(0.85, 0.85, 0.85))
    };

    let mode_badge = container(mode_label)
        .padding(iced::Padding::from([5, 14]))
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.20, 0.20, 0.20))),
            border: Border {
                color: Color::from_rgb(0.35, 0.35, 0.35),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });

    let mode_badge: Element<'_, gui::Message> = if app.cache.is_elevated {
        mode_badge.into()
    } else {
        tooltip(
            mode_badge,
            container(
                text(
                    "User mode is recommended on Linux. You can use sudo to manage elevated processes, with no guarantees.",
                )
                .size(12)
                .color(Color::from_rgb(0.88, 0.88, 0.88))
                .width(Length::Shrink),
            )
            .max_width(320)
            .width(Length::Shrink),
            TooltipPosition::Bottom,
        )
        .gap(10)
        .padding(10)
        .snap_within_viewport(true)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.11, 0.11, 0.11))),
            border: Border {
                color: Color::from_rgb(0.40, 0.40, 0.40),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
    };

    let top_row = row![
        container(tab_bar).width(Length::Fill),
        mode_badge,
        Space::new().width(Length::Fixed(10.0))
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let content: Element<'_, gui::Message> = match &app.active_tab {
        gui::TabId::Status => tab_status::view(
            &app.cache,
            &app.topo_view,
            app.num_cores,
            app.topology_group_repeat,
            &app.topology_classification_label,
            &app.topology_accuracy_warnings,
        )
        .into(),
        gui::TabId::Configure => tab_configure::view(
            &app.cache,
            app.dragging_process.as_deref(),
            &app.process_filter,
            app.search_pulse_phase,
            app.show_children,
        )
        .into(),
        gui::TabId::Options => tab_options::view(&app.cache, app.num_cores).into(),
    };

    let main_layout = column![top_row, content];

    if let Some(editor) = &app.group_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view(app.num_cores, app.topology_group_repeat))
            .into()
    } else if let Some(editor) = &app.process_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view())
            .into()
    } else if let Some(editor) = &app.custom_process_editor {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(editor.view(app.topology_group_repeat))
            .into()
    } else if app.groups_help_open {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(view_groups_help())
            .into()
    } else if app.inaccessible_list_open {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(view_inaccessible_processes_modal(
                app.cache.protected_names.clone(),
            ))
            .into()
    } else if app.topology_details_open {
        iced::widget::Stack::new()
            .push(main_layout)
            .push(view_topology_details_modal(
                app.topology_classification_label.clone(),
                app.topology_os_label.clone(),
                app.topology_hypervisor_on,
                app.topology_details_report.clone(),
                app.topology_copy_notice_ticks > 0,
            ))
            .into()
    } else {
        main_layout.into()
    }
}

/// Initialize and run the iced daemon app.
///
/// We run as a daemon so the process can outlive windows and stay available from the
/// tray; windows are opened explicitly during boot and from tray actions.
fn main() -> iced::Result {
    if std::env::args().any(|a| a == "--topology-report") {
        println!("{}", core::topology::topology_report());
        return Ok(());
    }

    if !enforce_single_instance() {
        return Ok(());
    }

    // `Result<T, E>` is a success/error return type (like returning value-or-exception outcome explicitly).
    // Linux tray integration requires GTK initialization on the main thread.
    #[cfg(target_os = "linux")]
    gtk::init().expect("Failed to initialize GTK (required for system tray)");

    let settings = Settings {
        id: Some("com.sas41.processaffinitycontroltool".to_string()),
        default_text_size: 14.into(),
        ..Default::default()
    };

    let (rgba, w, h) = load_icon_rgba();
    let _icon = iced::window::icon::from_rgba(rgba, w, h).ok();

    /// View callback for iced daemon windows.
    fn daemon_view<'a>(
        app: &'a ProcessAffinityApp,
        _id: iced::window::Id,
    ) -> Element<'a, gui::Message> {
        view(app)
    }

    /// Parse `--topology-dup N` / `--topology-dup=N` (or `--topology-repeat`).
    fn parse_topology_group_repeat_arg() -> usize {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = if let Some(v) = arg.strip_prefix("--topology-dup=") {
                Some(v.to_string())
            } else if let Some(v) = arg.strip_prefix("--topology-repeat=") {
                Some(v.to_string())
            } else if arg == "--topology-dup" || arg == "--topology-repeat" {
                args.next()
            } else {
                None
            };

            if let Some(raw) = value {
                return raw.parse::<usize>().ok().filter(|&n| n >= 1).unwrap_or(1);
            }
        }
        1
    }

    /// Create initial state and open the main window.
    fn boot() -> (ProcessAffinityApp, Task<gui::Message>) {
        let mut app = ProcessAffinityApp::default();
        app.topology_group_repeat = parse_topology_group_repeat_arg();
        app.cache = build_cache(&app.pact);
        app.is_elevated = is_elevated();

        if app.cache.launch_minimized {
            (app, Task::none())
        } else {
            let (rgba, w, h) = load_icon_rgba();
            let icon = iced::window::icon::from_rgba(rgba, w, h).ok();
            let (_, open_task) = iced::window::open(main_window_settings(icon));
            app.window_open_pending = true;
            (app, open_task.map(gui::Message::WindowOpened))
        }
    }

    iced::daemon(boot, update, daemon_view)
        .settings(settings)
        .title(|_app: &ProcessAffinityApp, _id| format!("PACT - {}", env!("APP_VERSION")))
        .subscription(subscription)
        .font(iced_fonts::BOOTSTRAP_FONT_BYTES)
        .theme(iced::Theme::Dark)
        .run()
}
