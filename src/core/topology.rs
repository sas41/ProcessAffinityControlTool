use hwlocality::{
    object::{attributes::ObjectAttributes, types::ObjectType},
    topology::Topology,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

// ─── CoreKind ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Unknown,
    Pcore,
    Ecore,
}

// ─── TopologyPreset ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TopologyPreset {
    PerformanceCores,
    EfficiencyCores,
    CCD(usize),
    NUMANode(usize),
    AllCores,
    HybridPerformance,
    HybridEfficiency,
}

// ─── LogicalProcessorInfo ─────────────────────────────────────────────────────

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

// ─── Cache entry ──────────────────────────────────────────────────────────────

/// One cache level's size (bytes) and level number.
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

// ─── TopologyView structs ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThreadView {
    pub logical_index: usize,
    pub kind: CoreKind,
}

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

// ─── CpuTopology ─────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct CpuTopology {
    processors: Vec<LogicalProcessorInfo>,
    /// Per-core private caches keyed by physical_core_index.
    core_private_caches: HashMap<usize, Vec<CacheEntry>>,
    /// Shared caches keyed by CCD (Die) logical index.
    ccd_shared_caches: HashMap<isize, Vec<CacheEntry>>,
}

impl CpuTopology {
    pub fn new() -> Self {
        Self::discover()
    }

    pub fn discover() -> Self {
        let mut topology = Self::default();

        let topo = match Topology::new() {
            Ok(t) => t,
            Err(_) => return topology,
        };

        // ── PU list ───────────────────────────────────────────────────────
        let mut logical_processors: Vec<_> = topo.objects_with_type(ObjectType::PU).collect();
        logical_processors.sort_by_key(|obj| obj.logical_index());

        // ── Physical core index map ───────────────────────────────────────
        let mut physical_core_map: HashMap<usize, usize> = HashMap::new();
        let mut physical_core_counter = 0usize;
        for obj in &logical_processors {
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

        // ── CPU kind map (Intel P/E) + FrequencyMaxMHz from cpu_kinds ────
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

        // ── Max frequency per LP from sysfs ───────────────────────────────
        let sysfs_max_freq = read_sysfs_max_freq_khz(logical_processors.len());

        // ── Per-core private caches (L1/L2) ──────────────────────────────
        // For every Core object, walk its ancestors collecting cache objects
        // until we hit a Die or Package (those are the shared levels).
        for core_obj in topo.objects_with_type(ObjectType::Core) {
            let core_li = core_obj.logical_index();
            let phys_idx = *physical_core_map.get(&core_li).unwrap_or(&core_li);
            let mut caches: Vec<CacheEntry> = Vec::new();

            let mut cur = core_obj.parent();
            while let Some(anc) = cur {
                match anc.object_type() {
                    // Stop when we leave the per-core hierarchy
                    ObjectType::Die | ObjectType::Package | ObjectType::Machine => break,
                    t if t.is_cpu_cache() => {
                        if let Some(ObjectAttributes::Cache(ca)) = anc.attributes() {
                            let level = ca.depth() as u8;
                            // L1 and L2 are private; L3+ are shared — stop at L3
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

        // ── Shared caches (L3+) per Die/CCD ──────────────────────────────
        // Walk all L3/L4/L5 cache objects; attribute each to its Die ancestor.
        for cache_type in [
            ObjectType::L3Cache,
            ObjectType::L4Cache,
            ObjectType::L5Cache,
        ] {
            for cache_obj in topo.objects_with_type(cache_type) {
                let Some(ObjectAttributes::Cache(ca)) = cache_obj.attributes() else {
                    continue;
                };
                let Some(sz) = ca.size() else { continue };
                let level = ca.depth() as u8;
                let size_bytes = sz.get();

                // Find the Die ancestor (-1 = no Die, put under key -1 for whole-package)
                let die_li = Self::find_ancestor_type_static(cache_obj, ObjectType::Die);

                topology
                    .ccd_shared_caches
                    .entry(die_li)
                    .or_default()
                    .push(CacheEntry { level, size_bytes });
            }
        }
        // Sort and deduplicate each Die's shared cache list
        for caches in topology.ccd_shared_caches.values_mut() {
            caches.sort_by_key(|c| c.level);
            caches.dedup_by_key(|c| c.level);
        }

        // If there were no Die objects, also check for L3 cache under Package
        // (single-CCD AMD, most Intel) and store under key -1.
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

        // ── Build LogicalProcessorInfo ────────────────────────────────────
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
            let ccd = Self::find_ancestor_type_static_li(&topo, obj, ObjectType::Die);
            let numa_node = Self::find_ancestor_type_static_li(&topo, obj, ObjectType::NUMANode);

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

    /// Walk obj's ancestors to find the first one of `target_type`, return its logical_index or -1.
    fn find_ancestor_type_static_li(
        _topo: &Topology,
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

    /// Same but takes just the object (no topology reference needed).
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

    // ── Public accessors ──────────────────────────────────────────────────

    pub fn processors(&self) -> &[LogicalProcessorInfo] {
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

    pub fn total_logical_processors(&self) -> usize {
        self.processors.len()
    }

    // ── Structured topology view ──────────────────────────────────────────

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
        // Try exact Die key first, then the package-level fallback (-1).
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
        // For a flat group use the first (and usually only) shared-cache entry
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

// ─── Sysfs max frequency (Linux) ─────────────────────────────────────────────

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

// ─── Formatting helpers ───────────────────────────────────────────────────────

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

pub fn format_freq_ghz(khz: u64) -> String {
    if khz == 0 {
        return String::new();
    }
    format!("{:.2} GHz", khz as f64 / 1_000_000.0)
}

// ─── Global singleton ─────────────────────────────────────────────────────────

static TOPOLOGY: OnceLock<CpuTopology> = OnceLock::new();

pub fn get_topology() -> &'static CpuTopology {
    TOPOLOGY.get_or_init(CpuTopology::discover)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

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
