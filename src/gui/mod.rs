// `crate::...` starts from this Rust crate's root module (similar to a C# root namespace).
use crate::core::pact_instance::AssignedProcess;
use crate::core::process_config::{CustomProcess, ProcessGroup};
use crate::core::process_overwatch::CpuStats;
use std::collections::HashMap;

/// Shared UI state snapshot used by all tabs.
/// Refreshed by `Message::Tick` so views can read cached values without
/// locking core state every frame.
// `#[derive(...)]` asks the compiler to auto-implement listed traits.
#[derive(Debug, Clone, Default)]
// `pub` = public visibility; `struct` groups named fields (like a C# class/record data shape).
pub struct AppCache {
    pub is_scanner_active: bool,
    pub is_elevated: bool,
    // `Vec<T>` is Rust's growable array/list (roughly `List<T>` in C#).
    pub groups: Vec<ProcessGroup>,
    pub running: Vec<String>,
    /// Effective group assignments shown in the UI.
    pub assigned: Vec<AssignedProcess>,
    /// Per-process affinity/priority settings, independent of groups.
    pub custom_processes: Vec<CustomProcess>,
    pub protected_count: usize,
    pub protected_names: Vec<String>,
    pub managed_count: usize,
    pub cpu_stats: CpuStats,
    pub scan_interval: u64,
    pub launch_minimized: bool,
    /// Maps child process name (lowercase) → direct parent process name (lowercase)
    /// for all processes currently managed via capture_sub_processes propagation.
    /// Used by the Configure tab to render tree views in group cards and custom process panel.
    pub child_process_parents: HashMap<String, String>,
}

// `mod` declares child modules loaded from other files.
pub mod custom_process_editor;
pub mod draggable_pill;
pub mod drop_zone;
pub mod group_editor;
pub mod priority;
pub mod process_editor;
/// Configure tab: edit groups, rules, and process mappings.
pub mod tab_configure;
/// Options tab: app-level preferences, import/export, and utilities.
pub mod tab_options;
/// Status tab: live runtime snapshot (processes, CPU, detections).
pub mod tab_status;
pub mod topology_diagram;
pub mod widgets;

/// Which main screen is currently shown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
// `enum` is a tagged union: one of several named variants.
pub enum TabId {
    #[default]
    // `#[default]` marks which variant `Default::default()` returns.
    /// Runtime overview and current system state.
    Status,
    /// Manual configuration of groups and process behavior.
    Configure,
    /// General app settings and maintenance actions.
    Options,
}

/// Central UI event type.
/// Carries tab-specific messages and app-wide commands through one update path.
#[derive(Debug, Clone)]
pub enum Message {
    // Tab messages.
    StatusMessage(tab_status::Message),
    ConfigureMessage(tab_configure::Message),
    OptionsMessage(tab_options::Message),

    OpenInaccessibleList,
    CloseInaccessibleList,
    OpenTopologyDetails,
    CloseTopologyDetails,
    CopyTopologyDetails,
    TopologyDetailsCopied,

    // Global messages.
    // Variant payload syntax: this case carries a `TabId` value.
    TabSelected(TabId),
    ImportConfig,
    ExportConfig,
    Exit,

    /// Mouse released with no DropZone under cursor — cancels an active drag.
    DragReleased,

    ShowGroupsHelp,
    HideGroupsHelp,
    ToggleShowChildren,

    /// OS close button was pressed; we decide whether to hide or quit.
    CloseRequested(iced::window::Id),
    /// A window was successfully opened.
    WindowOpened(iced::window::Id),
    /// A window was closed.
    WindowClosed(iced::window::Id),
    /// Polls tray icon and menu events at ~250 ms intervals.
    PollTrayEvents,

    /// Refreshes cached display data.
    Tick,
    /// Drives lightweight UI-only animations.
    SearchPulse,

    GroupEditorMessage(group_editor::Message),
    // `Option<T>` means maybe-a-value: `Some(T)` or `None` (nullable-like, but explicit).
    GroupEditorResult(Option<String>),
    GroupEditorDelete(String),
    GroupEditorClosed,
    ProcessEditorMessage(process_editor::Message),
    CustomProcessEditorMessage(custom_process_editor::Message),
}
