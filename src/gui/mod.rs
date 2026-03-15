use crate::core::process_config::{CustomProcess, ProcessGroup};
use crate::core::process_overwatch::CpuStats;

/// Pre-computed display data refreshed by the Tick subscription once per second.
/// View functions read from this cache instead of locking pact on every frame.
#[derive(Debug, Clone, Default)]
pub struct AppCache {
    pub is_scanner_active: bool,
    pub is_auto_mode: bool,
    pub groups: Vec<ProcessGroup>,
    pub running: Vec<String>,
    /// Processes explicitly assigned to a group: (process_name, group_name).
    pub assigned: Vec<(String, String)>,
    /// Processes with individual affinity/priority settings (independent of groups).
    pub custom_processes: Vec<CustomProcess>,
    pub protected_count: usize,
    pub cpu_stats: CpuStats,
    pub launchers: Vec<String>,
    pub detections: Vec<String>,
    pub scan_interval: u64,
    pub minimize_to_tray: bool,
}

pub mod custom_process_editor;
pub mod draggable_pill;
pub mod drop_zone;
pub mod group_editor;
pub mod priority;
pub mod process_editor;
pub mod tab_auto_mode;
pub mod tab_configure;
pub mod tab_options;
pub mod tab_status;
pub mod topology_diagram;
pub mod widgets;

/// Identifies which tab is currently active.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TabId {
    #[default]
    Status,
    Configure,
    AutoMode,
    Options,
}

// Common message type for the application
#[derive(Debug, Clone)]
pub enum Message {
    // Messages from tabs
    StatusMessage(tab_status::Message),
    ConfigureMessage(tab_configure::Message),
    AutoModeMessage(tab_auto_mode::Message),
    OptionsMessage(tab_options::Message),

    // Global messages
    TabSelected(TabId),
    ImportConfig,
    ExportConfig,
    Exit,

    /// Mouse released with no DropZone under cursor — cancels an active drag.
    DragReleased,

    ShowGroupsHelp,
    HideGroupsHelp,

    /// OS close button was pressed; we decide whether to hide or quit.
    CloseRequested,
    /// Polls tray icon and menu events at ~250 ms intervals.
    PollTrayEvents,


    Tick, // periodic refresh of cached display data

    GroupEditorMessage(group_editor::Message),
    GroupEditorResult(Option<String>),
    GroupEditorDelete(String),
    GroupEditorClosed,
    ProcessEditorMessage(process_editor::Message),
    CustomProcessEditorMessage(custom_process_editor::Message),
}
