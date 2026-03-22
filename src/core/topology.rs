//! CPU topology model built from platform-specific discovery.
//!
//! The platform layer provides a flat list of [`ThreadInfo`] records (one per
//! hardware thread). This module assembles them into a hierarchical group
//! model: NUMA > CCD/ComputeGroup > Core > Thread.

use crate::core::platform::{self, ThreadInfo};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::sync::OnceLock;
use sysinfo::System;

// ── Re-exports used by GUI consumers ─────────────────────────────────

pub use crate::core::platform::ThreadClassification;

// ── High-level selection presets exposed to callers ───────────────────

#[allow(dead_code)]
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

// ── Cache entry (view-level, aggregated) ─────────────────────────────

/// Aggregated cache information for display purposes.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub level: u8,
    pub size_bytes: u64,
    /// Individual slice sizes when multiple caches are aggregated.
    pub slice_sizes: Vec<u64>,
}

impl CacheEntry {
    pub fn label(&self) -> String {
        self.detailed_label()
    }

    pub fn detailed_label(&self) -> String {
        let base = format!("L{}: {}", self.level, format_cache_size(self.size_bytes));
        if self.slice_sizes.len() <= 1 {
            return base;
        }

        let mut sorted = self.slice_sizes.clone();
        sorted.sort_unstable();

        let details = if sorted.windows(2).all(|w| w[0] == w[1]) {
            format!("{} x {}", sorted.len(), format_cache_size(sorted[0]))
        } else {
            sorted
                .iter()
                .map(|s| format_cache_size(*s))
                .collect::<Vec<_>>()
                .join(" + ")
        };

        format!("{base} ({details})")
    }
}

// ── View model types: the hierarchical UI structure ──────────────────
//
// Hierarchy: TopologyView > NumaGroupView? > ComputeGroupView > CoreGroupView > ThreadUnitView

/// Smallest unit — one hardware thread.
#[derive(Debug, Clone)]
pub struct ThreadUnitView {
    pub logical_index: usize,
    pub classification: ThreadClassification,
}

/// One physical core containing one or more threads.
#[derive(Debug, Clone)]
pub struct CoreGroupView {
    pub core_index: usize,
    pub threads: Vec<ThreadUnitView>,
    /// Max boost frequency in MHz (0 if unavailable).
    pub max_freq_mhz: u64,
    /// Base frequency in MHz (0 if unavailable).
    pub base_freq_mhz: u64,
    /// Private caches (L1, L2) for this core.
    pub private_caches: Vec<CacheEntry>,
}

/// A compute group: CCD on AMD, P/E cluster on Intel, or a generic group.
#[derive(Debug, Clone)]
pub struct ComputeGroupView {
    pub label: String,
    /// Shared caches (L3+) for this group.
    pub shared_caches: Vec<CacheEntry>,
    pub cores: Vec<CoreGroupView>,
    /// Max frequency across all cores in MHz.
    pub max_freq_mhz: u64,
}

/// Optional NUMA grouping layer.
#[derive(Debug, Clone)]
pub struct NumaGroupView {
    pub numa_index: isize,
    pub label: String,
    pub compute_groups: Vec<ComputeGroupView>,
}

/// Root of the topology view hierarchy.
#[derive(Debug, Clone)]
pub struct TopologyView {
    /// When NUMA grouping is present, groups are nested under NUMA nodes.
    pub numa_groups: Vec<NumaGroupView>,
    /// When no NUMA grouping exists, compute groups sit at the top level.
    pub top_level_groups: Vec<ComputeGroupView>,
    /// True if NUMA grouping is active.
    pub has_numa: bool,
}

impl TopologyView {
    /// Iterate all compute groups regardless of NUMA nesting.
    /// This is the primary iteration method for GUI code.
    pub fn all_compute_groups(&self) -> Vec<&ComputeGroupView> {
        if self.has_numa {
            self.numa_groups
                .iter()
                .flat_map(|n| n.compute_groups.iter())
                .collect()
        } else {
            self.top_level_groups.iter().collect()
        }
    }
}

// ── Backward-compatible type aliases ─────────────────────────────────
// These keep existing GUI code compiling with minimal changes.

#[allow(dead_code)]
/// Alias: `TopLevelGroup` maps to `ComputeGroupView`.
pub type TopLevelGroup = ComputeGroupView;
#[allow(dead_code)]
/// Alias: `PhysicalCoreView` maps to `CoreGroupView`.
pub type PhysicalCoreView = CoreGroupView;
#[allow(dead_code)]
/// Alias: `ThreadView` maps to `ThreadUnitView`.
pub type ThreadView = ThreadUnitView;

// Backward-compatible field accessors for ThreadView.
impl ThreadUnitView {
    /// Backward compat: old code accessed `kind` as a `CoreKind`.
    pub fn kind_label(&self) -> &'static str {
        self.classification.label()
    }
}

// Backward-compatible field accessors for PhysicalCoreView.
impl CoreGroupView {
    /// Max frequency in kHz for backward compatibility.
    pub fn max_freq_khz(&self) -> u64 {
        self.max_freq_mhz * 1000
    }
}

// Backward-compatible field accessors for ComputeGroupView.
impl ComputeGroupView {
    /// Alias for backward compat: `physical_cores` field.
    pub fn physical_cores(&self) -> &[CoreGroupView] {
        &self.cores
    }
}

// ── Legacy CoreKind (kept for process_config.rs compatibility) ───────

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Unknown,
    Pcore,
    Ecore,
}

impl From<ThreadClassification> for CoreKind {
    fn from(tc: ThreadClassification) -> Self {
        match tc {
            ThreadClassification::Performance | ThreadClassification::HighFrequency => {
                CoreKind::Pcore
            }
            ThreadClassification::Efficiency | ThreadClassification::HighCache => CoreKind::Ecore,
            ThreadClassification::Generic => CoreKind::Unknown,
        }
    }
}

// ── Topology classification ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyClassification {
    NumaMultiCcd,
    MultiCcd,
    HybridPE,
    Monolithic,
}

impl Default for TopologyClassification {
    fn default() -> Self {
        Self::Monolithic
    }
}

impl TopologyClassification {
    pub fn label(self) -> &'static str {
        match self {
            Self::NumaMultiCcd => "NUMA + Multi-CCD",
            Self::MultiCcd => "Multi-CCD",
            Self::HybridPE => "Hybrid (P/E)",
            Self::Monolithic => "Monolithic",
        }
    }
}

// ── Main CpuTopology struct ──────────────────────────────────────────

#[derive(Debug)]
pub struct CpuTopology {
    threads: Vec<ThreadInfo>,
    classification: TopologyClassification,
}

impl Default for CpuTopology {
    fn default() -> Self {
        Self {
            threads: Vec::new(),
            classification: TopologyClassification::Monolithic,
        }
    }
}

impl CpuTopology {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::discover()
    }

    /// Main discovery entry point. Delegates to platform provider then builds groups.
    pub fn discover() -> Self {
        let threads = platform::discover_platform_threads();
        if threads.is_empty() {
            return Self::default();
        }

        let classification = classify_topology(&threads);

        Self {
            threads,
            classification,
        }
    }

    #[allow(dead_code)]
    pub fn processors(&self) -> &[ThreadInfo] {
        &self.threads
    }

    #[allow(dead_code)]
    pub fn total_logical_processors(&self) -> usize {
        self.threads.len()
    }

    pub fn classification_label(&self) -> &'static str {
        self.classification.label()
    }

    // ── Preset query helpers ─────────────────────────────────────────

    pub fn get_performance_cores(&self) -> Vec<usize> {
        self.threads
            .iter()
            .filter(|t| {
                t.classification == ThreadClassification::Performance
                    || t.classification == ThreadClassification::HighFrequency
            })
            .map(|t| t.thread_index)
            .collect()
    }

    pub fn get_efficiency_cores(&self) -> Vec<usize> {
        self.threads
            .iter()
            .filter(|t| {
                t.classification == ThreadClassification::Efficiency
                    || t.classification == ThreadClassification::HighCache
            })
            .map(|t| t.thread_index)
            .collect()
    }

    pub fn get_ccd_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: HashMap<isize, Vec<usize>> = HashMap::new();
        for t in &self.threads {
            if t.ccd_index >= 0 {
                groups.entry(t.ccd_index).or_default().push(t.thread_index);
            }
        }
        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by_key(|(k, _)| *k);
        result.into_iter().map(|(_, v)| v).collect()
    }

    #[allow(dead_code)]
    pub fn get_numa_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: HashMap<isize, Vec<usize>> = HashMap::new();
        for t in &self.threads {
            if t.numa_index >= 0 {
                groups.entry(t.numa_index).or_default().push(t.thread_index);
            }
        }
        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by_key(|(k, _)| *k);
        result.into_iter().map(|(_, v)| v).collect()
    }

    pub fn is_hybrid(&self) -> bool {
        let has_perf = self.threads.iter().any(|t| {
            t.classification == ThreadClassification::Performance
                || t.classification == ThreadClassification::HighFrequency
        });
        let has_eff = self.threads.iter().any(|t| {
            t.classification == ThreadClassification::Efficiency
                || t.classification == ThreadClassification::HighCache
        });
        has_perf && has_eff
    }

    // ── View construction ────────────────────────────────────────────

    pub fn topology_view(&self) -> TopologyView {
        // Step 1: Check for multiple NUMA nodes.
        let numa_ids = distinct_positive_values(self.threads.iter().map(|t| t.numa_index));
        let has_numa = numa_ids.len() > 1;

        if has_numa {
            // Build NUMA > CCD/Compute > Core > Thread hierarchy.
            let mut numa_groups = Vec::new();
            for &numa_id in &numa_ids {
                let numa_threads: Vec<&ThreadInfo> = self
                    .threads
                    .iter()
                    .filter(|t| t.numa_index == numa_id)
                    .collect();
                let compute_groups = self.build_compute_groups(&numa_threads);
                numa_groups.push(NumaGroupView {
                    numa_index: numa_id,
                    label: format!("NUMA Node {numa_id}"),
                    compute_groups,
                });
            }
            TopologyView {
                numa_groups,
                top_level_groups: Vec::new(),
                has_numa: true,
            }
        } else {
            // No NUMA: CCD/Compute > Core > Thread.
            let all_threads: Vec<&ThreadInfo> = self.threads.iter().collect();
            let top_level_groups = self.build_compute_groups(&all_threads);
            TopologyView {
                numa_groups: Vec::new(),
                top_level_groups,
                has_numa: false,
            }
        }
    }

    /// Step 2 & 3: Build compute groups from a set of threads.
    fn build_compute_groups(&self, threads: &[&ThreadInfo]) -> Vec<ComputeGroupView> {
        // Check for CCD grouping.
        let ccd_ids = distinct_positive_values(threads.iter().map(|t| t.ccd_index));
        let has_ccds = ccd_ids.len() > 1;

        if has_ccds {
            // AMD multi-CCD: group by CCD.
            ccd_ids
                .iter()
                .map(|&ccd_id| {
                    let ccd_threads: Vec<&ThreadInfo> = threads
                        .iter()
                        .filter(|t| t.ccd_index == ccd_id)
                        .copied()
                        .collect();

                    let label = self.compute_group_label_for_ccd(ccd_id, &ccd_threads);
                    let cores = self.build_core_groups(&ccd_threads);
                    let shared_caches = compute_shared_caches(&ccd_threads);
                    let max_freq_mhz = cores.iter().map(|c| c.max_freq_mhz).max().unwrap_or(0);

                    ComputeGroupView {
                        label,
                        shared_caches,
                        cores,
                        max_freq_mhz,
                    }
                })
                .collect()
        } else {
            // Check for compute group (Intel P/E or generic).
            let cg_ids = distinct_positive_values(threads.iter().map(|t| t.compute_group));
            let has_compute_groups = cg_ids.len() > 1;

            if has_compute_groups {
                // Intel hybrid or similar: group by compute_group.
                cg_ids
                    .iter()
                    .map(|&cg_id| {
                        let cg_threads: Vec<&ThreadInfo> = threads
                            .iter()
                            .filter(|t| t.compute_group == cg_id)
                            .copied()
                            .collect();

                        let label = self.compute_group_label_for_pe(&cg_threads);
                        let cores = self.build_core_groups(&cg_threads);
                        let shared_caches = compute_shared_caches(&cg_threads);
                        let max_freq_mhz = cores.iter().map(|c| c.max_freq_mhz).max().unwrap_or(0);

                        ComputeGroupView {
                            label,
                            shared_caches,
                            cores,
                            max_freq_mhz,
                        }
                    })
                    .collect()
            } else {
                // Monolithic / single group.
                let cores = self.build_core_groups(threads);
                let shared_caches = compute_shared_caches(threads);
                let max_freq_mhz = cores.iter().map(|c| c.max_freq_mhz).max().unwrap_or(0);

                vec![ComputeGroupView {
                    label: "All Cores".into(),
                    shared_caches,
                    cores,
                    max_freq_mhz,
                }]
            }
        }
    }

    /// Step 3 & 4: Build core groups containing thread units.
    fn build_core_groups(&self, threads: &[&ThreadInfo]) -> Vec<CoreGroupView> {
        let mut by_core: BTreeMap<usize, Vec<&ThreadInfo>> = BTreeMap::new();
        for t in threads {
            by_core.entry(t.core_index).or_default().push(t);
        }

        by_core
            .into_iter()
            .map(|(core_index, core_threads)| {
                let mut thread_views: Vec<ThreadUnitView> = core_threads
                    .iter()
                    .map(|t| ThreadUnitView {
                        logical_index: t.thread_index,
                        classification: t.classification,
                    })
                    .collect();
                thread_views.sort_by_key(|t| t.logical_index);

                let max_freq_mhz = core_threads
                    .iter()
                    .map(|t| t.max_freq_mhz)
                    .max()
                    .unwrap_or(0);
                let base_freq_mhz = core_threads
                    .iter()
                    .map(|t| t.base_freq_mhz)
                    .max()
                    .unwrap_or(0);

                // Private caches: L1, L2 (levels < 3).
                let private_caches = compute_private_caches(&core_threads);

                CoreGroupView {
                    core_index,
                    threads: thread_views,
                    max_freq_mhz,
                    base_freq_mhz,
                    private_caches,
                }
            })
            .collect()
    }

    /// Generate a label for a CCD-based compute group.
    fn compute_group_label_for_ccd(&self, ccd_id: isize, threads: &[&ThreadInfo]) -> String {
        let first = threads.first();
        match first.map(|t| t.classification) {
            Some(ThreadClassification::HighCache) => format!("CCD {ccd_id} (V-Cache)"),
            Some(ThreadClassification::HighFrequency) => format!("CCD {ccd_id} (High Freq)"),
            _ => format!("CCD {ccd_id}"),
        }
    }

    /// Generate a label for a P/E compute group.
    fn compute_group_label_for_pe(&self, threads: &[&ThreadInfo]) -> String {
        let first = threads.first();
        match first.map(|t| t.classification) {
            Some(ThreadClassification::Performance) => "Performance Cores".into(),
            Some(ThreadClassification::Efficiency) => "Efficiency Cores".into(),
            _ => "Compute Group".into(),
        }
    }

    // ── Diagnostic report ────────────────────────────────────────────

    pub fn detailed_report(&self) -> String {
        let view = self.topology_view();
        let mut out = String::new();

        let _ = writeln!(&mut out, "=== Topology Report ===");
        let _ = writeln!(
            &mut out,
            "classification      : {}",
            self.classification_label()
        );
        let runtime = topology_runtime_info();
        let _ = writeln!(&mut out, "os                  : {}", runtime.os_label);
        let _ = writeln!(
            &mut out,
            "hypervisor          : {}",
            if runtime.hypervisor_on { "On" } else { "Off" }
        );
        let _ = writeln!(&mut out, "logical processors  : {}", self.threads.len());

        let all_groups = view.all_compute_groups();
        let _ = writeln!(&mut out, "top-level groups   : {}", all_groups.len());
        let _ = writeln!(
            &mut out,
            "ccd groups         : {}",
            self.get_ccd_groups().len()
        );
        let _ = writeln!(
            &mut out,
            "numa groups        : {}",
            self.get_numa_groups().len()
        );
        let _ = writeln!(&mut out);

        let _ = writeln!(&mut out, "-- Logical Processors --");
        for t in &self.threads {
            let cache_str = if t.caches.is_empty() {
                "<none>".to_string()
            } else {
                t.caches
                    .iter()
                    .map(|c| {
                        format!(
                            "L{}={} grp#{}",
                            c.level,
                            format_cache_size(c.size_bytes),
                            c.group_id
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(
                &mut out,
                "T{}: C{} class={} ccd={} ccx={} numa={} cg={} base={} MHz max={} MHz caches=[{}]",
                t.thread_index,
                t.core_index,
                t.classification.label(),
                t.ccd_index,
                t.ccx_index,
                t.numa_index,
                t.compute_group,
                t.base_freq_mhz,
                t.max_freq_mhz,
                cache_str,
            );
        }
        let _ = writeln!(&mut out);

        if view.has_numa {
            for numa in &view.numa_groups {
                let _ = writeln!(&mut out, "=== {} ===", numa.label);
                for group in &numa.compute_groups {
                    write_group_report(&mut out, group);
                }
            }
        } else {
            for (gi, group) in view.top_level_groups.iter().enumerate() {
                let _ = writeln!(&mut out, "Group #{gi}: {}", group.label);
                write_group_report(&mut out, group);
            }
        }

        out
    }
}

// ── Free functions ───────────────────────────────────────────────────

fn classify_topology(threads: &[ThreadInfo]) -> TopologyClassification {
    let numa_count = distinct_positive_values(threads.iter().map(|t| t.numa_index)).len();
    let ccd_count = distinct_positive_values(threads.iter().map(|t| t.ccd_index)).len();

    let has_pe = threads.iter().any(|t| {
        t.classification == ThreadClassification::Performance
            || t.classification == ThreadClassification::Efficiency
    });

    if numa_count > 1 && ccd_count > 1 {
        TopologyClassification::NumaMultiCcd
    } else if ccd_count > 1 {
        TopologyClassification::MultiCcd
    } else if has_pe {
        TopologyClassification::HybridPE
    } else {
        TopologyClassification::Monolithic
    }
}

/// Collect distinct non-negative values, sorted.
fn distinct_positive_values<I: Iterator<Item = isize>>(iter: I) -> Vec<isize> {
    let mut vals: Vec<isize> = iter.filter(|&v| v >= 0).collect();
    vals.sort();
    vals.dedup();
    vals
}

/// Step 5: Compute shared caches (L3+) for a set of threads.
///
/// Each unique `(level, group_id)` pair represents one physical cache instance.
/// Multiple instances at the same level (e.g. two L3 caches on a dual-CCD chip)
/// are kept as separate slices so the label can show "64 MB (2 x 32 MB)".
fn compute_shared_caches(threads: &[&ThreadInfo]) -> Vec<CacheEntry> {
    // Deduplicate by (level, group_id) so each physical cache instance
    // appears exactly once, even though many threads reference it.
    let mut by_level_group: BTreeMap<(u8, isize), u64> = BTreeMap::new();

    for t in threads {
        for c in &t.caches {
            if c.level >= 3 {
                by_level_group
                    .entry((c.level, c.group_id))
                    .or_insert(c.size_bytes);
            }
        }
    }

    // Group the distinct cache instances by level. Do NOT dedup by size —
    // two 32 MB L3 caches are two separate slices, not one.
    let mut level_slices: BTreeMap<u8, Vec<u64>> = BTreeMap::new();
    for (&(level, _), &size) in &by_level_group {
        level_slices.entry(level).or_default().push(size);
    }

    for slices in level_slices.values_mut() {
        slices.sort_unstable();
    }

    level_slices
        .into_iter()
        .map(|(level, slices)| {
            let total: u64 = slices.iter().sum();
            CacheEntry {
                level,
                size_bytes: total,
                slice_sizes: slices,
            }
        })
        .collect()
}

/// Compute private caches (L1, L2) for a set of threads in the same core.
fn compute_private_caches(threads: &[&ThreadInfo]) -> Vec<CacheEntry> {
    let mut by_level: BTreeMap<u8, u64> = BTreeMap::new();

    // All threads in a core share the same private caches, so just take
    // the first thread's cache data for levels < 3.
    if let Some(first) = threads.first() {
        for c in &first.caches {
            if c.level < 3 {
                by_level.entry(c.level).or_insert(c.size_bytes);
            }
        }
    }

    by_level
        .into_iter()
        .map(|(level, size_bytes)| CacheEntry {
            level,
            size_bytes,
            slice_sizes: vec![size_bytes],
        })
        .collect()
}

fn write_group_report(out: &mut String, group: &ComputeGroupView) {
    let _ = writeln!(out, "  {}", group.label);
    let _ = writeln!(out, "  max freq: {} MHz", group.max_freq_mhz);

    if group.shared_caches.is_empty() {
        let _ = writeln!(out, "  shared cache: <none>");
    } else {
        let shared = group
            .shared_caches
            .iter()
            .map(|c| c.detailed_label())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  shared cache: {shared}");
    }

    for core in &group.cores {
        let threads = core
            .threads
            .iter()
            .map(|t| format!("T{}({})", t.logical_index, t.classification.label()))
            .collect::<Vec<_>>()
            .join(", ");
        let private = if core.private_caches.is_empty() {
            "<none>".to_string()
        } else {
            core.private_caches
                .iter()
                .map(|c| c.detailed_label())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            out,
            "    C{}: base={} MHz max={} MHz threads=[{threads}] private=[{private}]",
            core.core_index, core.base_freq_mhz, core.max_freq_mhz,
        );
    }

    let _ = writeln!(out);
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn is_hypervisor_present() -> bool {
    #[cfg(target_arch = "x86")]
    let info = core::arch::x86::__cpuid(1);
    #[cfg(target_arch = "x86_64")]
    let info = core::arch::x86_64::__cpuid(1);
    (info.ecx & (1 << 31)) != 0
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn is_hypervisor_present() -> bool {
    false
}

#[derive(Debug, Clone)]
pub struct TopologyRuntimeInfo {
    pub os_label: String,
    pub hypervisor_on: bool,
    pub accuracy_warnings: Vec<String>,
}

pub fn topology_runtime_info() -> TopologyRuntimeInfo {
    let os_label = detect_os_label();
    let hypervisor_on = is_hypervisor_present();
    let mut accuracy_warnings = Vec::new();

    if hypervisor_on {
        accuracy_warnings.push(
            "Hypervisor is active: cache/topology data may be virtualized or flattened."
                .to_string(),
        );
    }

    #[cfg(target_os = "windows")]
    if os_label.starts_with("Windows 10") {
        accuracy_warnings.push(
            "Windows 10 may expose less accurate asymmetric CCD/cache details on newer CPUs."
                .to_string(),
        );
    }

    TopologyRuntimeInfo {
        os_label,
        hypervisor_on,
        accuracy_warnings,
    }
}

fn detect_os_label() -> String {
    let name = System::name().unwrap_or_default();
    let long = System::long_os_version().unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        if long.contains("Windows 11") {
            return "Windows 11".to_string();
        }
        if long.contains("Windows 10") {
            return "Windows 10".to_string();
        }
        if !long.is_empty() {
            return long;
        }
        if name.is_empty() {
            "Windows".to_string()
        } else {
            name
        }
    }

    #[cfg(target_os = "linux")]
    {
        if !long.is_empty() {
            return long;
        }
        return if name.is_empty() {
            "Linux".to_string()
        } else {
            name
        };
    }

    #[cfg(target_os = "macos")]
    {
        if !long.is_empty() {
            return long;
        }
        return "macOS".to_string();
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        if !long.is_empty() {
            return long;
        }
        if !name.is_empty() {
            return name;
        }
        "Unknown OS".to_string()
    }
}

// ── Formatting helpers ───────────────────────────────────────────────

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

/// Format frequency from MHz to a human-readable string.
pub fn format_freq_mhz(mhz: u64) -> String {
    if mhz == 0 {
        return String::new();
    }
    if mhz >= 1000 {
        format!("{:.2} GHz", mhz as f64 / 1000.0)
    } else {
        format!("{mhz} MHz")
    }
}

// ── Process-wide lazy singleton ──────────────────────────────────────

static TOPOLOGY: OnceLock<CpuTopology> = OnceLock::new();

pub fn get_topology() -> &'static CpuTopology {
    TOPOLOGY.get_or_init(CpuTopology::discover)
}

/// Builds a human-readable topology report for diagnostics.
pub fn topology_report() -> String {
    get_topology().detailed_report()
}

pub fn topology_classification_label() -> &'static str {
    get_topology().classification_label()
}

pub fn topology_details_report() -> String {
    get_topology().detailed_report()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_creation() {
        let t = CpuTopology::discover();
        assert!(!t.threads.is_empty());
    }

    #[test]
    fn test_topology_view_non_empty() {
        let t = CpuTopology::discover();
        let view = t.topology_view();
        let groups = view.all_compute_groups();
        assert!(!groups.is_empty());
        for g in groups {
            assert!(!g.cores.is_empty());
            for c in &g.cores {
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
    fn test_format_freq_mhz() {
        assert_eq!(format_freq_mhz(5271), "5.27 GHz");
        assert_eq!(format_freq_mhz(0), "");
        assert_eq!(format_freq_mhz(800), "800 MHz");
    }

    #[test]
    fn test_format_freq_ghz() {
        assert_eq!(format_freq_ghz(5271622), "5.27 GHz");
        assert_eq!(format_freq_ghz(0), "");
    }
}
