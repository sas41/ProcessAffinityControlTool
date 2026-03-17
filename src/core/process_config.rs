use crate::core::topology::{CpuTopology, TopologyPreset};
use serde::{Deserialize, Serialize};

/// OS process priority class used by this tool.
/// `#[derive(...)]` asks Rust to auto-implement common traits for this type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// `pub` means publicly visible outside this module (like C# `public`).
/// `enum` defines a closed set of named variants.
pub enum ProcessPriority {
    /// Lowest scheduling preference.
    Idle,
    /// Lower-than-normal scheduling preference.
    BelowNormal,
    /// Default scheduling preference.
    Normal,
    /// Higher-than-normal scheduling preference.
    AboveNormal,
    /// High scheduling preference.
    High,
    /// Highest scheduling preference (use sparingly).
    RealTime,
}

impl Default for ProcessPriority {
    /// `impl Trait for Type` implements a trait (similar to interface-like behavior).
    fn default() -> Self {
        // `Self` refers to the current type (`ProcessPriority`) in this impl block.
        ProcessPriority::Normal
    }
}

/// Concrete affinity and priority settings.
///
/// `serde(rename = ...)` keeps compatibility with external PascalCase keys
/// (`Priority`, `AffinityMask`, `CoreList`). Missing keys are not auto-filled
/// because these fields do not use `serde(default)`.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessConfig {
    #[serde(rename = "Priority")]
    pub priority: ProcessPriority,

    #[serde(rename = "AffinityMask")]
    pub affinity_mask: u64,

    #[serde(rename = "CoreList")]
    /// `Vec<usize>` is a growable array of platform-sized unsigned indexes.
    pub core_list: Vec<usize>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        // `let mut` creates a mutable local variable.
        let mut config = Self {
            priority: ProcessPriority::Normal,
            affinity_mask: 0,
            core_list: (0..num_cpus::get()).collect(),
        };
        config.recalculate_mask();
        config
    }
}

#[allow(dead_code)]
impl ProcessConfig {
    /// Builds a concrete config from an explicit core list and priority.
    ///
    /// Panics if any core index is outside `0..num_cpus::get()`.
    pub fn new(core_list: Vec<usize>, priority: ProcessPriority) -> Self {
        let max_count = num_cpus::get();
        // `|...|` starts a closure; `&x` destructures each referenced item from the iterator.
        if core_list.iter().any(|&x| x >= max_count) {
            panic!(
                "Thread numbers are between 0 and {} on this machine!",
                max_count - 1
            );
        }
        let mut config = Self {
            priority,
            affinity_mask: 0,
            core_list,
        };
        config.recalculate_mask();
        config
    }

    /// Converts a topology preset into a concrete core list.
    ///
    /// Presets that resolve to an empty list are normalized by
    /// `recalculate_mask()` to core `0`.
    pub fn from_topology_preset(
        // `&Type` is an immutable borrow/reference (roughly C# `in`/read-only reference semantics).
        topology: &CpuTopology,
        preset: TopologyPreset,
        _idx: usize,
    ) -> Self {
        // `match` is an exhaustive pattern match expression.
        let core_list = match preset {
            TopologyPreset::PerformanceCores => topology.get_performance_cores(),
            TopologyPreset::EfficiencyCores => topology.get_efficiency_cores(),
            TopologyPreset::CCD(i) => {
                let groups = topology.get_ccd_groups();
                groups.get(i).cloned().unwrap_or_default()
            }
            TopologyPreset::NUMANode(i) => {
                let groups = topology.get_numa_groups();
                groups.get(i).cloned().unwrap_or_default()
            }
            TopologyPreset::AllCores => (0..num_cpus::get()).collect(),
            TopologyPreset::HybridPerformance => {
                if topology.is_hybrid() {
                    topology.get_performance_cores()
                } else {
                    (0..num_cpus::get()).collect()
                }
            }
            TopologyPreset::HybridEfficiency => {
                if topology.is_hybrid() {
                    topology.get_efficiency_cores()
                } else {
                    Vec::new()
                }
            }
        };
        Self::new(core_list, ProcessPriority::Normal)
    }

    /// Recomputes `affinity_mask` from `core_list`.
    ///
    /// Empty `core_list` is normalized to `[0]` so the process always has at
    /// least one runnable core.
    pub fn recalculate_mask(&mut self) -> u64 {
        // `&mut self` is a mutable borrow of the current instance.
        let max_cores = num_cpus::get() as u64;
        if self.core_list.iter().any(|&x| x as u64 >= max_cores) {
            panic!("Invalid core number. Max: {}", max_cores - 1);
        }
        if self.core_list.is_empty() {
            self.core_list = vec![0];
        }
        let mut mask = 0u64;
        for &c in &self.core_list {
            mask |= 1 << c;
        }
        self.affinity_mask = mask;
        mask
    }

    pub fn with_priority(mut self, priority: ProcessPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_core_list(mut self, core_list: Vec<usize>) -> Self {
        self.core_list = core_list;
        self.recalculate_mask();
        self
    }
}

/// Named group for shared process settings.
///
/// `affinity` and `priority` are optional:
/// - `Option<T>` is Rust's nullable/optional container.
/// - `None`: leave that attribute unchanged.
/// - `Some(_)`: apply that value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessGroup {
    /// Display name (unique within a config).
    pub name: String,

    /// CPU affinity to apply, or `None` to leave unchanged.
    pub affinity: Option<AffinityConfig>,

    /// Priority class to apply, or `None` to leave unchanged.
    pub priority: Option<ProcessPriority>,

    /// Linux niceness override (`-20..19`), or `None` to leave unchanged.
    #[serde(rename = "Niceness", default)]
    pub niceness: Option<i32>,

    /// Fallback group for unassigned processes. At most one group should use this.
    pub is_default: bool,

    /// Skip all changes for processes in this group.
    pub is_blacklist: bool,

    /// Target group for auto-mode launcher detections.
    #[serde(default)]
    pub is_auto_mode_group: bool,
}

/// CPU affinity as editable core list plus precomputed bitmask.
///
/// `core_list` is the user-facing value; `affinity_mask` is the derived bitset
/// used when applying affinity. Unlike `ProcessConfig`, an empty `core_list`
/// stays empty here and produces a zero mask.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AffinityConfig {
    pub core_list: Vec<usize>,
    pub affinity_mask: u64,
}

impl AffinityConfig {
    pub fn new(core_list: Vec<usize>) -> Self {
        let max = num_cpus::get();
        let mut mask = 0u64;
        for &c in &core_list {
            if c < max {
                mask |= 1u64 << c;
            }
        }
        Self {
            core_list,
            affinity_mask: mask,
        }
    }

    /// Affinity spanning all logical cores.
    pub fn all_cores() -> Self {
        Self::new((0..num_cpus::get()).collect())
    }
}

/// Per-process settings independent of group membership.
///
/// `None` means "do not override" that attribute for this process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomProcess {
    pub name: String,
    pub affinity: Option<AffinityConfig>,
    pub priority: Option<ProcessPriority>,
    #[serde(rename = "Niceness", default)]
    pub niceness: Option<i32>,
}

impl CustomProcess {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            affinity: None,
            priority: None,
            niceness: None,
        }
    }
}

impl ProcessGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            affinity: None,
            priority: None,
            niceness: None,
            is_default: false,
            is_blacklist: false,
            is_auto_mode_group: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_config_default() {
        let config = ProcessConfig::default();
        assert_eq!(config.priority, ProcessPriority::Normal);
        assert!(!config.core_list.is_empty());
    }

    #[test]
    fn test_process_config_new() {
        let config = ProcessConfig::new(vec![0, 2], ProcessPriority::High);
        assert_eq!(config.priority, ProcessPriority::High);
        assert_eq!(config.core_list, vec![0, 2]);
        assert_eq!(config.affinity_mask, 0b101);
    }

    #[test]
    fn test_affinity_config_new() {
        let a = AffinityConfig::new(vec![0, 2]);
        assert_eq!(a.affinity_mask, 0b101);
    }

    #[test]
    fn test_process_group_defaults() {
        let g = ProcessGroup::new("Test");
        assert_eq!(g.name, "Test");
        assert!(g.affinity.is_none());
        assert!(g.priority.is_none());
        assert!(g.niceness.is_none());
        assert!(!g.is_default);
        assert!(!g.is_blacklist);
        assert!(!g.is_auto_mode_group);
    }
}
