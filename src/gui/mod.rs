// `crate::...` starts from this Rust crate's root module (similar to a C# root namespace).
use crate::core::process_config::{CustomProcess, ProcessGroup};
use crate::core::process_overwatch::CpuStats;

/// Shared UI state snapshot used by all tabs.
/// Refreshed by `Message::Tick` so views can read cached values without
/// locking core state every frame.
// `#[derive(...)]` asks the compiler to auto-implement listed traits.
#[derive(Debug, Clone, Default)]
// `pub` = public visibility; `struct` groups named fields (like a C# class/record data shape).
pub struct AppCache {
    pub is_scanner_active: bool,
    pub is_auto_mode: bool,
    // `Vec<T>` is Rust's growable array/list (roughly `List<T>` in C#).
    pub groups: Vec<ProcessGroup>,
    pub running: Vec<String>,
    /// Explicit group assignments as `(process_name, group_name)`.
    // `(A, B)` is a tuple: fixed-size pair without field names.
    pub assigned: Vec<(String, String)>,
    /// Per-process affinity/priority settings, independent of groups.
    pub custom_processes: Vec<CustomProcess>,
    pub protected_count: usize,
    pub cpu_stats: CpuStats,
    pub launchers: Vec<String>,
    pub detections: Vec<String>,
    pub scan_interval: u64,
}

// `mod` declares child modules loaded from other files.
pub mod custom_process_editor;
pub mod draggable_pill;
pub mod drop_zone;
pub mod group_editor;
pub mod priority;
pub mod process_editor;
/// Auto Mode tab: manage scanner/assignment automation behavior.
pub mod tab_auto_mode;
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
    /// Automation controls for background scanning/assignment.
    AutoMode,
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
    AutoModeMessage(tab_auto_mode::Message),
    OptionsMessage(tab_options::Message),

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

    GroupEditorMessage(group_editor::Message),
    // `Option<T>` means maybe-a-value: `Some(T)` or `None` (nullable-like, but explicit).
    GroupEditorResult(Option<String>),
    GroupEditorDelete(String),
    GroupEditorClosed,
    ProcessEditorMessage(process_editor::Message),
    CustomProcessEditorMessage(custom_process_editor::Message),
}
