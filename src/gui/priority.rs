/// GUI helpers for converting between:
/// - dropdown index (`usize`)
/// - visible label (`&str`)
/// - core enum (`ProcessPriority`)
///
/// Mapping order is fixed as:
/// `Idle, Below Normal, Normal, Above Normal, High, RealTime`.
///
/// Rust quick notes for C# readers (first encounter):
/// - `usize`: unsigned integer type used for indexing collections.
/// - `&str`: borrowed string slice (read-only view, no allocation).
/// - `::`: namespace/type separator (like `Type.Member` in C#).
use crate::core::process_config::ProcessPriority;

/// Display labels by GUI index.
pub const PRIORITY_LABELS: &[&str] = &[
    "Idle",
    "Below Normal",
    "Normal",
    "Above Normal",
    "High",
    "RealTime",
];

/// Converts GUI index -> label.
///
/// Fallback: any out-of-range index returns `"Normal"`.
///
/// Rust quick notes:
/// - `&'static str`: string literal reference valid for entire program.
/// - `.get(i)`: safe indexing; returns `Option` instead of throwing.
/// - `.copied().unwrap_or("Normal")`: take value when present, else fallback.
#[allow(dead_code)]
pub fn priority_label(i: usize) -> &'static str {
    PRIORITY_LABELS.get(i).copied().unwrap_or("Normal")
}

/// Converts GUI index -> `ProcessPriority`.
///
/// Fallback: any out-of-range index maps to `ProcessPriority::Normal`.
///
/// Rust quick notes:
/// - `match` is Rust's `switch` with exhaustive pattern handling.
/// - `_` means "any other value" (default arm).
pub fn index_to_priority(i: usize) -> ProcessPriority {
    match i {
        0 => ProcessPriority::Idle,
        1 => ProcessPriority::BelowNormal,
        2 => ProcessPriority::Normal,
        3 => ProcessPriority::AboveNormal,
        4 => ProcessPriority::High,
        5 => ProcessPriority::RealTime,
        _ => ProcessPriority::Normal,
    }
}

/// Converts `ProcessPriority` -> GUI index.
///
/// Rust quick notes:
/// - Parameter `p: &ProcessPriority` is a borrowed enum (no ownership move).
/// - Enum variants are matched with `Type::Variant`.
pub fn priority_to_index(p: &ProcessPriority) -> usize {
    match p {
        ProcessPriority::Idle => 0,
        ProcessPriority::BelowNormal => 1,
        ProcessPriority::Normal => 2,
        ProcessPriority::AboveNormal => 3,
        ProcessPriority::High => 4,
        ProcessPriority::RealTime => 5,
    }
}
