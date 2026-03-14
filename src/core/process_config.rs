use crate::core::topology::{CpuTopology, TopologyPreset};
use serde::{Deserialize, Serialize};

// ─── ProcessPriority ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    RealTime,
}

impl Default for ProcessPriority {
    fn default() -> Self {
        ProcessPriority::Normal
    }
}

// ─── ProcessConfig (affinity + priority, both concrete) ──────────────────────

/// Concrete affinity + priority settings — used where both values are known.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessConfig {
    #[serde(rename = "Priority")]
    pub priority: ProcessPriority,

    #[serde(rename = "AffinityMask")]
    pub affinity_mask: u64,

    #[serde(rename = "CoreList")]
    pub core_list: Vec<usize>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        let mut config = Self {
            priority: ProcessPriority::Normal,
            affinity_mask: 0,
            core_list: (0..num_cpus::get()).collect(),
        };
        config.recalculate_mask();
        config
    }
}

impl ProcessConfig {
    pub fn new(core_list: Vec<usize>, priority: ProcessPriority) -> Self {
        let max_count = num_cpus::get();
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

    pub fn from_topology_preset(
        topology: &CpuTopology,
        preset: TopologyPreset,
        _idx: usize,
    ) -> Self {
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

    pub fn recalculate_mask(&mut self) -> u64 {
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

// ─── ProcessGroup ─────────────────────────────────────────────────────────────

/// A named group that processes can be assigned to.
///
/// Both `affinity` and `priority` are optional:
/// - `None` means "do not touch this attribute for processes in this group".
/// - `Some(_)` means "apply this value".
///
/// `is_default`: at most one group carries this flag.  Processes not explicitly
/// assigned to any other group land here.  If no group is default, unassigned
/// processes are left completely untouched.
///
/// `is_blacklist`: processes in this group are completely skipped — no affinity
/// or priority is applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessGroup {
    /// Display name for the group (unique within a config).
    pub name: String,

    /// If `Some`, set these cores as the CPU affinity.  If `None`, do not
    /// change the affinity of processes in this group.
    pub affinity: Option<AffinityConfig>,

    /// If `Some`, set this priority class.  If `None`, do not change priority.
    pub priority: Option<ProcessPriority>,

    /// This group receives all processes that are not explicitly assigned
    /// elsewhere.  Only one group may have this set to `true`.
    pub is_default: bool,

    /// Processes in this group are skipped entirely (the blacklist semantic).
    pub is_blacklist: bool,
}

/// Affinity is stored as both a core list (for display/editing) and a pre-
/// computed bitmask (for fast application).  Always kept in sync.
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

    /// All logical cores on this machine.
    pub fn all_cores() -> Self {
        Self::new((0..num_cpus::get()).collect())
    }
}

impl ProcessGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            affinity: None,
            priority: None,
            is_default: false,
            is_blacklist: false,
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

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
        assert!(!g.is_default);
        assert!(!g.is_blacklist);
    }
}
