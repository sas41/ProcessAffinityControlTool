use crate::core::process_config::ProcessPriority;

pub const PRIORITY_LABELS: &[&str] = &[
    "Idle",
    "Below Normal",
    "Normal",
    "Above Normal",
    "High",
    "RealTime",
];

#[allow(dead_code)]
pub fn priority_label(i: usize) -> &'static str {
    PRIORITY_LABELS.get(i).copied().unwrap_or("Normal")
}

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
