// Rust `::` is namespace access (like C# `.`), and `{...}` groups imports from one path.
use hwlocality::{
    object::{attributes::ObjectAttributes, types::ObjectType},
    topology::Topology,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

// Coarse core-class model used by UI/presets.
// We intentionally keep this small (P/E/Unknown) because hwloc detail varies by CPU/vendor.

// `#[derive(...)]` asks the compiler to auto-implement standard traits (roughly like generated boilerplate interfaces/methods in C#).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Unknown,
    Pcore,
    Ecore,
}

// High-level selection presets exposed to callers.

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TopologyPreset {
    PerformanceCores,
    EfficiencyCores,
    // `CCD(usize)` is a tuple variant: enum case carrying data (similar to a C# discriminated-union case with payload).
    CCD(usize),
    NUMANode(usize),
    AllCores,
    HybridPerformance,
    HybridEfficiency,
}

// Stable per-logical-CPU record built from hwloc plus Linux sysfs.

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LogicalProcessorInfo {
    pub logical_index: usize,
    pub physical_core_index: usize,
    pub kind: CoreKind,
    pub ccd: isize,
    pub numa_node: isize,
    pub is_hyperthread_sibling: bool,
    /// Maximum clock frequency in kHz (0 if unavailable).
    pub max_freq_khz: u64,
}

/// Cache level and size in bytes.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub level: u8,
    pub size_bytes: u64,
}

impl CacheEntry {
    pub fn label(&self) -> String {
        format!("L{}: {}", self.level, format_cache_size(self.size_bytes))
    }
}

// Read-only view models consumed by presentation code.

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ThreadView {
    pub logical_index: usize,
    pub kind: CoreKind,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PhysicalCoreView {
    pub physical_index: usize,
    pub threads: Vec<ThreadView>,
    /// Max boost frequency in kHz (0 if unavailable).
    pub max_freq_khz: u64,
    /// Per-core private caches (L1d, L1i, L2). Sorted by level.
    pub private_caches: Vec<CacheEntry>,
}

#[derive(Debug, Clone)]
pub struct TopLevelGroup {
    pub label: String,
    /// Shared caches (L3, L4…) for this group. Sorted by level.
    pub shared_caches: Vec<CacheEntry>,
    pub physical_cores: Vec<PhysicalCoreView>,
}

#[derive(Debug, Clone)]
pub struct TopologyView {
    pub groups: Vec<TopLevelGroup>,
}

// Normalized topology snapshot used across the app.

#[derive(Debug, Default)]
pub struct CpuTopology {
    processors: Vec<LogicalProcessorInfo>,
    /// Private caches keyed by dense physical-core index used in this module.
    core_private_caches: HashMap<usize, Vec<CacheEntry>>,
    /// Shared caches keyed by die logical index (`-1` means package-level fallback).
    ccd_shared_caches: HashMap<isize, Vec<CacheEntry>>,
}

impl CpuTopology {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::discover()
    }

    pub fn discover() -> Self {
        let mut topology = Self::default();

        // `match` is expression-based branching; each arm must return a compatible type.
        let topo = match Topology::new() {
            Ok(t) => t,
            Err(_) => return topology,
        };

        // hwloc PUs are logical CPUs (hardware threads). Sort for deterministic output.
        let mut logical_processors: Vec<_> = topo.objects_with_type(ObjectType::PU).collect();
        // `|obj| ...` is a closure/lambda parameter list.
        logical_processors.sort_by_key(|obj| obj.logical_index());

        // Map hwloc core logical IDs to dense 0..N indices for stable grouping/UI labels.
        let mut physical_core_map: HashMap<usize, usize> = HashMap::new();
        let mut physical_core_counter = 0usize;
        for obj in &logical_processors {
            // `if let` is pattern matching for one desired shape (here: only `Some(...)`).
            if let Some(parent) = obj.parent() {
                if parent.object_type() == ObjectType::Core {
                    let core_li = parent.logical_index();
                    physical_core_map.entry(core_li).or_insert_with(|| {
                        let idx = physical_core_counter;
                        physical_core_counter += 1;
                        idx
                    });
                }
            }
        }

        // Build per-logical-CPU kind/frequency hints from hwloc cpu_kinds metadata.
        let mut kind_map: HashMap<usize, CoreKind> = HashMap::new();
        let mut kind_max_freq_mhz: HashMap<usize, u64> = HashMap::new();
        if let Ok(kinds) = topo.cpu_kinds() {
            let kinds_vec: Vec<_> = kinds.collect();
            let num_kinds = kinds_vec.len();
            for kind in &kinds_vec {
                let core_kind = match kind.efficiency {
                    None => CoreKind::Unknown,
                    Some(_) if num_kinds == 1 => CoreKind::Unknown,
                    Some(0) => CoreKind::Ecore,
                    Some(_) => CoreKind::Pcore,
                };
                let freq_mhz: u64 = kind
                    .infos
                    .iter()
                    .find(|i| {
                        i.name()
                            .to_str()
                            .map(|n| n == "FrequencyMaxMHz")
                            .unwrap_or(false)
                    })
                    .and_then(|i| i.value().to_str().ok().and_then(|v| v.parse().ok()))
                    .unwrap_or(0);
                for bit in kind.cpuset.iter_set() {
                    let li = usize::from(bit);
                    kind_map.insert(li, core_kind);
                    if freq_mhz > 0 {
                        kind_max_freq_mhz.insert(li, freq_mhz);
                    }
                }
            }
        }

        // Linux may expose more accurate per-CPU max clocks in sysfs; use it first when present.
        let sysfs_max_freq = read_sysfs_max_freq_khz(logical_processors.len());

        // Walk upward from each core and collect private caches (L1/L2) before shared levels.
        for core_obj in topo.objects_with_type(ObjectType::Core) {
            let core_li = core_obj.logical_index();
            let phys_idx = *physical_core_map.get(&core_li).unwrap_or(&core_li);
            let mut caches: Vec<CacheEntry> = Vec::new();

            let mut cur = core_obj.parent();
            while let Some(anc) = cur {
                match anc.object_type() {
                    ObjectType::Die | ObjectType::Package | ObjectType::Machine => break,
                    t if t.is_cpu_cache() => {
                        if let Some(ObjectAttributes::Cache(ca)) = anc.attributes() {
                            let level = ca.depth() as u8;
                            if level >= 3 {
                                break;
                            }
                            if let Some(sz) = ca.size() {
                                caches.push(CacheEntry {
                                    level,
                                    size_bytes: sz.get(),
                                });
                            }
                        }
                    }
                    _ => {}
                }
                cur = anc.parent();
            }

            caches.sort_by_key(|c| c.level);
            caches.dedup_by_key(|c| c.level);
            if !caches.is_empty() {
                topology.core_private_caches.insert(phys_idx, caches);
            }
        }

        // Collect shared caches (L3+) and bucket by die/CCD.
        for cache_type in [
            ObjectType::L3Cache,
            ObjectType::L4Cache,
            ObjectType::L5Cache,
        ] {
            for cache_obj in topo.objects_with_type(cache_type) {
                // `let ... else` destructures-or-early-continues; useful for linear happy-path code.
                let Some(ObjectAttributes::Cache(ca)) = cache_obj.attributes() else {
                    continue;
                };
                let Some(sz) = ca.size() else { continue };
                let level = ca.depth() as u8;
                let size_bytes = sz.get();

                let die_li = Self::find_ancestor_type_static(cache_obj, ObjectType::Die);

                topology
                    .ccd_shared_caches
                    .entry(die_li)
                    .or_default()
                    .push(CacheEntry { level, size_bytes });
            }
        }
        // Normalize each shared-cache list for deterministic display.
        for caches in topology.ccd_shared_caches.values_mut() {
            caches.sort_by_key(|c| c.level);
            caches.dedup_by_key(|c| c.level);
        }

        // Some topologies have no Die objects. Keep L3 data under sentinel key `-1`.
        if topology.ccd_shared_caches.is_empty() {
            for cache_obj in topo.objects_with_type(ObjectType::L3Cache) {
                let Some(ObjectAttributes::Cache(ca)) = cache_obj.attributes() else {
                    continue;
                };
                let Some(sz) = ca.size() else { continue };
                topology
                    .ccd_shared_caches
                    .entry(-1)
                    .or_default()
                    .push(CacheEntry {
                        level: ca.depth() as u8,
                        size_bytes: sz.get(),
                    });
            }
            if let Some(caches) = topology.ccd_shared_caches.get_mut(&-1) {
                caches.sort_by_key(|c| c.level);
                caches.dedup_by_key(|c| c.level);
            }
        }

        // Build final logical-CPU records used by selection/grouping helpers.
        for obj in &logical_processors {
            let li = obj.logical_index();

            let physical_core_index = obj
                .parent()
                .filter(|p| p.object_type() == ObjectType::Core)
                .and_then(|p| physical_core_map.get(&p.logical_index()).copied())
                .unwrap_or(li);

            let is_hyperthread_sibling = obj
                .parent()
                .filter(|p| p.object_type() == ObjectType::Core)
                .map(|p| p.normal_children().count() > 1)
                .unwrap_or(false);

            let kind = kind_map.get(&li).copied().unwrap_or(CoreKind::Unknown);
            let ccd = Self::find_ancestor_type_static_li(obj, ObjectType::Die);
            let numa_node = Self::find_ancestor_type_static_li(obj, ObjectType::NUMANode);

            let max_freq_khz = sysfs_max_freq
                .get(li)
                .copied()
                .filter(|&v| v > 0)
                .or_else(|| kind_max_freq_mhz.get(&li).map(|&mhz| mhz * 1000))
                .unwrap_or(0);

            topology.processors.push(LogicalProcessorInfo {
                logical_index: li,
                physical_core_index,
                kind,
                ccd,
                numa_node,
                is_hyperthread_sibling,
                max_freq_khz,
            });
        }

        topology
    }

    /// Returns first ancestor logical index of the target type, or -1.
    fn find_ancestor_type_static_li(
        obj: &hwlocality::object::TopologyObject,
        target_type: ObjectType,
    ) -> isize {
        // Keep traversal in a free/static helper shape so callers can finish borrowing `self`
        // before walking hwloc parent links (simplifies lifetime/borrow interactions).
        let mut cur = obj.parent();
        // `while let` keeps looping while pattern match succeeds.
        while let Some(anc) = cur {
            if anc.object_type() == target_type {
                return anc.logical_index() as isize;
            }
            cur = anc.parent();
        }
        -1
    }

    /// Same traversal helper kept separate for call-site readability in cache code paths.
    fn find_ancestor_type_static(
        obj: &hwlocality::object::TopologyObject,
        target_type: ObjectType,
    ) -> isize {
        let mut cur = obj.parent();
        while let Some(anc) = cur {
            if anc.object_type() == target_type {
                return anc.logical_index() as isize;
            }
            cur = anc.parent();
        }
        -1
    }

    // Query helpers used by presets and filtering.

    pub fn processors(&self) -> &[LogicalProcessorInfo] {
        // `&T` is a shared reference; `&[T]` is a slice view (read-only window over contiguous items).
        &self.processors
    }

    pub fn get_performance_cores(&self) -> Vec<usize> {
        self.processors
            .iter()
            .filter(|p| p.kind == CoreKind::Pcore)
            .map(|p| p.logical_index)
            .collect()
    }

    pub fn get_efficiency_cores(&self) -> Vec<usize> {
        self.processors
            .iter()
            .filter(|p| p.kind == CoreKind::Ecore)
            .map(|p| p.logical_index)
            .collect()
    }

    pub fn get_ccd_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: HashMap<isize, Vec<usize>> = HashMap::new();
        for proc in &self.processors {
            if proc.ccd >= 0 {
                groups.entry(proc.ccd).or_default().push(proc.logical_index);
            }
        }
        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by_key(|(k, _)| *k);
        result.into_iter().map(|(_, v)| v).collect()
    }

    #[allow(dead_code)]
    pub fn get_numa_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: HashMap<isize, Vec<usize>> = HashMap::new();
        for proc in &self.processors {
            if proc.numa_node >= 0 {
                groups
                    .entry(proc.numa_node)
                    .or_default()
                    .push(proc.logical_index);
            }
        }
        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by_key(|(k, _)| *k);
        result.into_iter().map(|(_, v)| v).collect()
    }

    pub fn is_hybrid(&self) -> bool {
        self.processors.iter().any(|p| p.kind == CoreKind::Pcore)
            && self.processors.iter().any(|p| p.kind == CoreKind::Ecore)
    }

    #[allow(dead_code)]
    pub fn total_logical_processors(&self) -> usize {
        self.processors.len()
    }

    // Structured presentation model with one top-level grouping strategy:
    // die/CCD first, otherwise hybrid split, otherwise one flat group.

    pub fn topology_view(&self) -> TopologyView {
        let has_ccd = self.processors.iter().any(|p| p.ccd >= 0);
        let groups = if has_ccd {
            self.build_ccd_groups()
        } else if self.is_hybrid() {
            self.build_hybrid_groups()
        } else {
            self.build_flat_group()
        };
        TopologyView { groups }
    }

    fn shared_caches_for_die(&self, die_li: isize) -> Vec<CacheEntry> {
        // Prefer exact die key, then package-level fallback (`-1`).
        self.ccd_shared_caches
            .get(&die_li)
            .or_else(|| self.ccd_shared_caches.get(&-1))
            .cloned()
            .unwrap_or_default()
    }

    fn build_ccd_groups(&self) -> Vec<TopLevelGroup> {
        let mut ccd_ids: Vec<isize> = self
            .processors
            .iter()
            .filter(|p| p.ccd >= 0)
            .map(|p| p.ccd)
            .collect();
        ccd_ids.sort();
        ccd_ids.dedup();

        ccd_ids
            .iter()
            .map(|&ccd_id| {
                let procs: Vec<&LogicalProcessorInfo> =
                    self.processors.iter().filter(|p| p.ccd == ccd_id).collect();
                TopLevelGroup {
                    label: format!("Complex {ccd_id}"),
                    shared_caches: self.shared_caches_for_die(ccd_id),
                    physical_cores: self.build_physical_cores_view(&procs),
                }
            })
            .collect()
    }

    fn build_hybrid_groups(&self) -> Vec<TopLevelGroup> {
        let p_procs: Vec<&LogicalProcessorInfo> = self
            .processors
            .iter()
            .filter(|p| p.kind == CoreKind::Pcore)
            .collect();
        let e_procs: Vec<&LogicalProcessorInfo> = self
            .processors
            .iter()
            .filter(|p| p.kind == CoreKind::Ecore)
            .collect();

        let mut groups = Vec::new();
        if !p_procs.is_empty() {
            groups.push(TopLevelGroup {
                label: "Performance Cores".into(),
                shared_caches: self.shared_caches_for_die(-1),
                physical_cores: self.build_physical_cores_view(&p_procs),
            });
        }
        if !e_procs.is_empty() {
            groups.push(TopLevelGroup {
                label: "Efficiency Cores".into(),
                shared_caches: self.shared_caches_for_die(-1),
                physical_cores: self.build_physical_cores_view(&e_procs),
            });
        }
        groups
    }

    fn build_flat_group(&self) -> Vec<TopLevelGroup> {
        let all: Vec<&LogicalProcessorInfo> = self.processors.iter().collect();
        // Flat view uses first shared-cache entry when available.
        let shared = self
            .ccd_shared_caches
            .values()
            .next()
            .cloned()
            .unwrap_or_default();
        vec![TopLevelGroup {
            label: "All Cores".into(),
            shared_caches: shared,
            physical_cores: self.build_physical_cores_view(&all),
        }]
    }

    fn build_physical_cores_view(&self, procs: &[&LogicalProcessorInfo]) -> Vec<PhysicalCoreView> {
        // Input uses references to avoid copying processor records while regrouping.
        let mut by_core: HashMap<usize, Vec<(ThreadView, u64)>> = HashMap::new();
        for p in procs {
            by_core.entry(p.physical_core_index).or_default().push((
                ThreadView {
                    logical_index: p.logical_index,
                    kind: p.kind,
                },
                p.max_freq_khz,
            ));
        }
        for threads in by_core.values_mut() {
            threads.sort_by_key(|(t, _)| t.logical_index);
        }
        let mut cores: Vec<PhysicalCoreView> = by_core
            .into_iter()
            .map(|(phys_idx, threads)| {
                let max_freq_khz = threads.iter().map(|(_, f)| *f).max().unwrap_or(0);
                let private_caches = self
                    .core_private_caches
                    .get(&phys_idx)
                    .cloned()
                    .unwrap_or_default();
                PhysicalCoreView {
                    physical_index: phys_idx,
                    threads: threads.into_iter().map(|(t, _)| t).collect(),
                    max_freq_khz,
                    private_caches,
                }
            })
            .collect();
        cores.sort_by_key(|c| c.threads.first().map_or(0, |t| t.logical_index));
        cores
    }
}

// Linux-only max-frequency probe. Non-Linux builds keep zeros and rely on hwloc hints.

fn read_sysfs_max_freq_khz(num_cpus: usize) -> Vec<u64> {
    let mut result = vec![0u64; num_cpus];
    #[cfg(target_os = "linux")]
    for i in 0..num_cpus {
        let path = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq");
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(v) = s.trim().parse::<u64>() {
                result[i] = v;
            }
        }
    }
    result
}

// Small formatting helpers for human-readable UI labels.

pub fn format_cache_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.0} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[allow(dead_code)]
pub fn format_freq_ghz(khz: u64) -> String {
    if khz == 0 {
        return String::new();
    }
    format!("{:.2} GHz", khz as f64 / 1_000_000.0)
}

// Process-wide lazy singleton so topology discovery runs once.

static TOPOLOGY: OnceLock<CpuTopology> = OnceLock::new();

pub fn get_topology() -> &'static CpuTopology {
    TOPOLOGY.get_or_init(CpuTopology::discover)
}

// Basic invariants for discovery and formatting.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_creation() {
        let t = CpuTopology::discover();
        assert!(!t.processors.is_empty());
    }

    #[test]
    fn test_topology_view_non_empty() {
        let t = CpuTopology::discover();
        let view = t.topology_view();
        assert!(!view.groups.is_empty());
        for g in &view.groups {
            assert!(!g.physical_cores.is_empty());
            for c in &g.physical_cores {
                assert!(!c.threads.is_empty());
            }
        }
    }

    #[test]
    fn test_format_cache_size() {
        assert_eq!(format_cache_size(32 * 1024 * 1024), "32 MB");
        assert_eq!(format_cache_size(512 * 1024), "512 KB");
    }

    #[test]
    fn test_format_freq_ghz() {
        assert_eq!(format_freq_ghz(5271622), "5.27 GHz");
        assert_eq!(format_freq_ghz(0), "");
    }
}
