//! Windows topology discovery using Win32 APIs.
//!
//! Uses `GetLogicalProcessorInformationEx` for cache/NUMA/group topology
//! and `CallNtPowerInformation` for per-CPU frequency data.
//! Falls back to hwloc for core kind classification when available.

use super::{CacheLevelInfo, PlatformTopologyProvider, ThreadClassification, ThreadInfo};
use std::collections::HashMap;

pub struct WindowsProvider;

impl PlatformTopologyProvider for WindowsProvider {
    fn discover_threads(&self) -> Vec<ThreadInfo> {
        let num_cpus = num_cpus::get();
        if num_cpus == 0 {
            return Vec::new();
        }

        // Step 1: Gather cache topology from GetLogicalProcessorInformationEx.
        let mut cache_info = query_cache_topology(num_cpus);
        apply_l3_size_overrides(&mut cache_info, num_cpus);
        // Step 2: Gather NUMA topology.
        let numa_info = query_numa_topology(num_cpus);
        // Step 3: Gather processor group/package topology for CCD-like grouping.
        let _group_info = query_processor_groups(num_cpus);
        // Step 4: Gather frequency data.
        let freq_info = query_frequencies(num_cpus);
        // Step 5: Gather core mapping (which threads share a physical core).
        let core_info = query_core_topology(num_cpus);
        // Step 6: hwloc for P/E classification if available.
        let kind_info = query_hwloc_kinds(num_cpus);

        // Normalize L3 group ids to dense CCD indices (0..N-1) for reporting.
        let mut l3_group_ids: Vec<isize> = cache_info
            .entries
            .iter()
            .filter(|e| e.level == 3)
            .map(|e| e.cache_id)
            .collect();
        l3_group_ids.sort_unstable();
        l3_group_ids.dedup();
        let l3_group_to_ccd: HashMap<isize, isize> = l3_group_ids
            .into_iter()
            .enumerate()
            .map(|(dense, raw)| (raw, dense as isize))
            .collect();

        let mut threads = Vec::with_capacity(num_cpus);
        for li in 0..num_cpus {
            let core_index = core_info.thread_to_core.get(&li).copied().unwrap_or(li);

            let mut caches: Vec<CacheLevelInfo> = Vec::new();
            for entry in &cache_info.entries {
                if entry.threads.contains(&li) {
                    caches.push(CacheLevelInfo {
                        level: entry.level,
                        size_bytes: entry.size_bytes,
                        group_id: entry.cache_id,
                    });
                }
            }
            caches.sort_by_key(|c| c.level);
            caches.dedup_by_key(|c| c.level);

            let classification = kind_info
                .get(&li)
                .copied()
                .unwrap_or(ThreadClassification::Generic);

            // CCD: use L3 cache group as proxy for CCD on AMD.
            // If there are multiple distinct L3 groups, each represents a CCD.
            let ccd_index = caches
                .iter()
                .find(|c| c.level == 3)
                .and_then(|c| l3_group_to_ccd.get(&c.group_id).copied())
                .unwrap_or(-1);

            let numa_index = numa_info.thread_to_numa.get(&li).copied().unwrap_or(-1);

            let (base_freq_mhz, max_freq_mhz) = freq_info.get(&li).copied().unwrap_or((0, 0));

            // Compute group logic:
            // - Intel hybrid: P-cores = 0, E-cores = 1.
            // - AMD multi-CCD: -1 (defer to CCD grouping).
            // - Monolithic: 0.
            let compute_group = match classification {
                ThreadClassification::Performance => 0,
                ThreadClassification::Efficiency => 1,
                _ if ccd_index >= 0 && cache_info.num_l3_groups > 1 => -1,
                _ => 0,
            };

            threads.push(ThreadInfo {
                thread_index: li,
                core_index,
                base_freq_mhz,
                max_freq_mhz,
                caches,
                classification,
                ccx_index: -1,
                ccd_index,
                numa_index,
                compute_group,
            });
        }

        // Post-process: reclassify AMD X3D CCDs as HighCache/HighFrequency.
        reclassify_amd_x3d(&mut threads);

        threads
    }
}

// ── Cache topology via GetLogicalProcessorInformationEx ──────────────

struct CacheTopologyEntry {
    level: u8,
    size_bytes: u64,
    cache_id: isize,
    threads: Vec<usize>,
}

struct CacheTopologyResult {
    entries: Vec<CacheTopologyEntry>,
    num_l3_groups: usize,
}

fn query_cache_topology(num_cpus: usize) -> CacheTopologyResult {
    use windows::Win32::System::SystemInformation::{
        RelationCache, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };

    let mut result = CacheTopologyResult {
        entries: Vec::new(),
        num_l3_groups: 0,
    };

    let buffer = match get_processor_info_buffer(RelationCache) {
        Some(b) => b,
        None => return result,
    };

    let mut cache_id_counter: isize = 0;
    let mut l3_groups = std::collections::HashSet::new();

    let mut offset = 0;
    while offset < buffer.len() {
        let entry = unsafe {
            &*(buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX)
        };
        let entry_size = entry.Size as usize;
        if entry_size == 0 || offset + entry_size > buffer.len() {
            break;
        }

        let cache = unsafe { &entry.Anonymous.Cache };
        let level = cache.Level;
        let size = cache.CacheSize as u64;

        if level > 0 && size > 0 {
            let mask = unsafe { cache.Anonymous.GroupMask.Mask as u64 };
            let threads = mask_to_indices(mask, num_cpus);
            let cid = cache_id_counter;
            cache_id_counter += 1;

            if level == 3 {
                l3_groups.insert(cid);
            }

            result.entries.push(CacheTopologyEntry {
                level,
                size_bytes: size,
                cache_id: cid,
                threads,
            });
        }

        offset += entry_size;
    }

    result.num_l3_groups = l3_groups.len();
    result
}

/// On some Windows systems (notably Ryzen X3D), Win32 may report identical L3
/// sizes for all CCDs. Cross-check with per-thread CPUID (preferred) and hwloc
/// and override each Win32 L3 group's size by majority vote.
fn apply_l3_size_overrides(cache_info: &mut CacheTopologyResult, num_cpus: usize) {
    let cpuid_l3_by_thread = query_cpuid_l3_sizes(num_cpus);
    let hwloc_l3_by_thread = query_hwloc_l3_sizes(num_cpus);
    if cpuid_l3_by_thread.is_empty() && hwloc_l3_by_thread.is_empty() {
        return;
    }

    for entry in &mut cache_info.entries {
        if entry.level != 3 || entry.threads.is_empty() {
            continue;
        }

        let mut counts_cpuid: HashMap<u64, usize> = HashMap::new();
        let mut counts_hwloc: HashMap<u64, usize> = HashMap::new();
        for &t in &entry.threads {
            if let Some(&sz) = cpuid_l3_by_thread.get(&t)
                && sz > 0
            {
                *counts_cpuid.entry(sz).or_insert(0) += 1;
            }
            if let Some(&sz) = hwloc_l3_by_thread.get(&t)
                && sz > 0
            {
                *counts_hwloc.entry(sz).or_insert(0) += 1;
            }
        }

        // Prefer CPUID-derived cache sizes for AMD X3D asymmetry.
        let best = counts_cpuid
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(&size, _)| size)
            .or_else(|| {
                counts_hwloc
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(&size, _)| size)
            });

        if let Some(best_size) = best {
            entry.size_bytes = best_size;
        }
    }
}

/// Read L3 cache size per logical thread via CPUID by temporarily pinning the
/// current thread to each logical processor.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn query_cpuid_l3_sizes(num_cpus: usize) -> HashMap<usize, u64> {
    use windows::Win32::System::Threading::{
        GetCurrentProcessorNumber, GetCurrentThread, SetThreadAffinityMask, Sleep, SwitchToThread,
    };

    let mut map = HashMap::new();
    let bits = std::mem::size_of::<usize>() * 8;
    let max = num_cpus.min(bits);

    for li in 0..max {
        let mask = 1usize << li;
        let old_mask = unsafe { SetThreadAffinityMask(GetCurrentThread(), mask) };
        if old_mask == 0 {
            continue;
        }

        // Ensure we actually execute CPUID on the targeted logical processor.
        // Affinity changes can be observed lazily, so spin/yield briefly.
        for _ in 0..64 {
            let cur = unsafe { GetCurrentProcessorNumber() as usize };
            if cur == li {
                break;
            }
            unsafe {
                let _ = SwitchToThread();
                Sleep(0);
            }
        }

        let l3_ext = query_l3_size_for_current_cpu_leaf(0x8000_001D);
        let l3_det = query_l3_size_for_current_cpu_leaf(0x0000_0004);
        let l3_size = l3_ext.max(l3_det);

        unsafe {
            let _ = SetThreadAffinityMask(GetCurrentThread(), old_mask);
        }

        if l3_size > 0 {
            map.insert(li, l3_size);
        }
    }

    map
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn query_l3_size_for_current_cpu_leaf(leaf: u32) -> u64 {
    let mut l3_size = 0u64;

    for subleaf in 0u32..32u32 {
        #[cfg(target_arch = "x86")]
        let regs = core::arch::x86::__cpuid_count(leaf, subleaf);
        #[cfg(target_arch = "x86_64")]
        let regs = core::arch::x86_64::__cpuid_count(leaf, subleaf);

        let cache_type = regs.eax & 0x1f;
        if cache_type == 0 {
            break;
        }

        let level = (regs.eax >> 5) & 0x7;
        if level == 3 {
            let line_size = u64::from((regs.ebx & 0x0fff) + 1);
            let partitions = u64::from(((regs.ebx >> 12) & 0x03ff) + 1);
            let ways = u64::from(((regs.ebx >> 22) & 0x03ff) + 1);
            let sets = u64::from(regs.ecx) + 1;
            let size = line_size
                .saturating_mul(partitions)
                .saturating_mul(ways)
                .saturating_mul(sets);
            l3_size = l3_size.max(size);
        }
    }

    l3_size
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn query_cpuid_l3_sizes(_num_cpus: usize) -> HashMap<usize, u64> {
    HashMap::new()
}

/// Read L3 cache size for each logical thread from hwloc ancestry.
fn query_hwloc_l3_sizes(num_cpus: usize) -> HashMap<usize, u64> {
    use hwlocality::{
        object::{attributes::ObjectAttributes, types::ObjectType},
        topology::Topology,
    };

    let mut map = HashMap::new();

    let topo = match Topology::new() {
        Ok(t) => t,
        Err(_) => return map,
    };

    for pu in topo.objects_with_type(ObjectType::PU) {
        let li = pu.logical_index();
        if li >= num_cpus {
            continue;
        }

        let mut cur = pu.parent();
        while let Some(anc) = cur {
            match anc.object_type() {
                t if t.is_cpu_cache() => {
                    if let Some(ObjectAttributes::Cache(ca)) = anc.attributes()
                        && ca.depth() == 3
                    {
                        if let Some(sz) = ca.size() {
                            map.insert(li, sz.get());
                        }
                        break;
                    }
                }
                ObjectType::Die | ObjectType::Package | ObjectType::Machine => break,
                _ => {}
            }
            cur = anc.parent();
        }
    }

    map
}

// ── NUMA topology ────────────────────────────────────────────────────

struct NumaTopologyResult {
    thread_to_numa: HashMap<usize, isize>,
}

fn query_numa_topology(num_cpus: usize) -> NumaTopologyResult {
    use windows::Win32::System::SystemInformation::{
        RelationNumaNode, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };

    let mut result = NumaTopologyResult {
        thread_to_numa: HashMap::new(),
    };

    let buffer = match get_processor_info_buffer(RelationNumaNode) {
        Some(b) => b,
        None => return result,
    };

    let mut offset = 0;
    while offset < buffer.len() {
        let entry = unsafe {
            &*(buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX)
        };
        let entry_size = entry.Size as usize;
        if entry_size == 0 || offset + entry_size > buffer.len() {
            break;
        }

        let numa = unsafe { &entry.Anonymous.NumaNode };
        let node_number = numa.NodeNumber as isize;
        let mask = unsafe { numa.Anonymous.GroupMask.Mask as u64 };
        let threads = mask_to_indices(mask, num_cpus);
        for t in threads {
            result.thread_to_numa.insert(t, node_number);
        }

        offset += entry_size;
    }

    result
}

// ── Processor group / package topology ───────────────────────────────

struct ProcessorGroupResult {
    // Reserved for future use. Currently we use L3 cache groups as CCD proxy.
    _num_groups: usize,
}

fn query_processor_groups(_num_cpus: usize) -> ProcessorGroupResult {
    // Windows processor groups are typically only relevant for >64 core systems.
    // For CCD detection we rely on L3 cache grouping instead, which is more
    // accurate across AMD and Intel architectures.
    ProcessorGroupResult { _num_groups: 1 }
}

// ── Core topology (thread-to-core mapping) ───────────────────────────

struct CoreTopologyResult {
    thread_to_core: HashMap<usize, usize>,
}

fn query_core_topology(num_cpus: usize) -> CoreTopologyResult {
    use windows::Win32::System::SystemInformation::{
        RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    };

    let mut result = CoreTopologyResult {
        thread_to_core: HashMap::new(),
    };

    let buffer = match get_processor_info_buffer(RelationProcessorCore) {
        Some(b) => b,
        None => return result,
    };

    let mut core_index = 0usize;
    let mut offset = 0;
    while offset < buffer.len() {
        let entry = unsafe {
            &*(buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX)
        };
        let entry_size = entry.Size as usize;
        if entry_size == 0 || offset + entry_size > buffer.len() {
            break;
        }

        let processor = unsafe { &entry.Anonymous.Processor };
        // GroupCount is at most 1 for core relationships.
        if processor.GroupCount > 0 {
            let mask = processor.GroupMask[0].Mask as u64;
            let threads = mask_to_indices(mask, num_cpus);
            for t in threads {
                result.thread_to_core.insert(t, core_index);
            }
        }
        core_index += 1;

        offset += entry_size;
    }

    result
}

// ── Frequency data ───────────────────────────────────────────────────

fn query_frequencies(num_cpus: usize) -> HashMap<usize, (u64, u64)> {
    use windows::Win32::System::Power::{
        CallNtPowerInformation, ProcessorInformation, POWER_INFORMATION_LEVEL,
        PROCESSOR_POWER_INFORMATION,
    };

    let mut result = HashMap::new();
    let mut info = vec![PROCESSOR_POWER_INFORMATION::default(); num_cpus];
    let out_size = (std::mem::size_of::<PROCESSOR_POWER_INFORMATION>() * info.len()) as u32;

    let status = unsafe {
        CallNtPowerInformation(
            POWER_INFORMATION_LEVEL(ProcessorInformation.0),
            None,
            0,
            Some(info.as_mut_ptr() as *mut core::ffi::c_void),
            out_size,
        )
    };

    if status.0 == 0 {
        for (i, p) in info.iter().enumerate() {
            let base_mhz = u64::from(p.MaxMhz); // MaxMhz is actually base clock.
            let max_mhz = u64::from(p.MhzLimit).max(base_mhz); // MhzLimit may be boost.
            result.insert(i, (base_mhz, max_mhz));
        }
    }

    // Supplement with sysinfo for better boost detection.
    let sysinfo_freq = read_sysinfo_freq_mhz(num_cpus);
    for (i, &mhz) in sysinfo_freq.iter().enumerate() {
        if mhz > 0 {
            let entry = result.entry(i).or_insert((0, 0));
            // If sysinfo reports higher than what we have, use it as max.
            if mhz > entry.1 {
                entry.1 = mhz;
            }
            if entry.0 == 0 {
                entry.0 = mhz;
            }
        }
    }

    result
}

// ── hwloc P/E classification ─────────────────────────────────────────

fn query_hwloc_kinds(num_cpus: usize) -> HashMap<usize, ThreadClassification> {
    use hwlocality::topology::Topology;

    let mut map = HashMap::new();

    let topo = match Topology::new() {
        Ok(t) => t,
        Err(_) => return map,
    };

    if let Ok(kinds) = topo.cpu_kinds() {
        let kinds_vec: Vec<_> = kinds.collect();
        let num_kinds = kinds_vec.len();
        for kind in &kinds_vec {
            let classification = match kind.efficiency {
                None => ThreadClassification::Generic,
                Some(_) if num_kinds == 1 => ThreadClassification::Generic,
                Some(0) => ThreadClassification::Efficiency,
                Some(_) => ThreadClassification::Performance,
            };
            for bit in kind.cpuset.iter_set() {
                let li = usize::from(bit);
                if li < num_cpus {
                    map.insert(li, classification);
                }
            }
        }
    }

    map
}

// ── AMD X3D reclassification ─────────────────────────────────────────

/// Detect AMD X3D (V-Cache) configurations and reclassify CCDs.
///
/// Heuristic: if there are exactly 2 L3 cache groups with different sizes,
/// the larger one is HighCache (V-Cache CCD) and the smaller is HighFrequency.
fn reclassify_amd_x3d(threads: &mut [ThreadInfo]) {
    // Only reclassify if all threads are currently Generic (not Intel hybrid).
    if threads.iter().any(|t| {
        t.classification == ThreadClassification::Performance
            || t.classification == ThreadClassification::Efficiency
    }) {
        return;
    }

    // Collect distinct L3 groups and their sizes.
    let mut l3_groups: HashMap<isize, u64> = HashMap::new();
    for t in threads.iter() {
        if let Some(l3) = t.caches.iter().find(|c| c.level == 3) {
            l3_groups.entry(l3.group_id).or_insert(l3.size_bytes);
        }
    }

    // Need at least 2 distinct L3 groups with different sizes for X3D detection.
    if l3_groups.len() < 2 {
        return;
    }

    let sizes: Vec<u64> = l3_groups.values().copied().collect();
    let all_same = sizes.windows(2).all(|w| w[0] == w[1]);
    if all_same {
        return; // Symmetric CCDs, not X3D.
    }

    let max_size = *sizes.iter().max().unwrap();
    let high_cache_groups: Vec<isize> = l3_groups
        .iter()
        .filter(|(_, sz)| **sz == max_size)
        .map(|(id, _)| *id)
        .collect();

    for t in threads.iter_mut() {
        if let Some(l3) = t.caches.iter().find(|c| c.level == 3) {
            if high_cache_groups.contains(&l3.group_id) {
                t.classification = ThreadClassification::HighCache;
            } else {
                t.classification = ThreadClassification::HighFrequency;
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Call GetLogicalProcessorInformationEx and return the raw buffer.
fn get_processor_info_buffer(
    relation: windows::Win32::System::SystemInformation::LOGICAL_PROCESSOR_RELATIONSHIP,
) -> Option<Vec<u8>> {
    use windows::Win32::System::SystemInformation::GetLogicalProcessorInformationEx;

    let mut buffer_size: u32 = 0;

    // First call to get required buffer size.
    unsafe {
        let _ = GetLogicalProcessorInformationEx(relation, None, &mut buffer_size);
    }

    if buffer_size == 0 {
        return None;
    }

    let mut buffer = vec![0u8; buffer_size as usize];
    let success = unsafe {
        GetLogicalProcessorInformationEx(
            relation,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut buffer_size,
        )
    };

    if success.is_ok() {
        Some(buffer)
    } else {
        None
    }
}

/// Convert a processor affinity mask to a list of logical CPU indices.
fn mask_to_indices(mask: u64, max_cpus: usize) -> Vec<usize> {
    let mut indices = Vec::new();
    for bit in 0..64usize.min(max_cpus) {
        if mask & (1u64 << bit) != 0 {
            indices.push(bit);
        }
    }
    indices
}

/// Read current CPU frequencies via sysinfo crate (MHz).
fn read_sysinfo_freq_mhz(num_cpus: usize) -> Vec<u64> {
    use sysinfo::{CpuRefreshKind, RefreshKind, System};

    let cpu_refresh = CpuRefreshKind::nothing().with_cpu_usage().with_frequency();
    let mut system = System::new_with_specifics(RefreshKind::nothing().with_cpu(cpu_refresh));
    system.refresh_cpu_specifics(cpu_refresh);
    system.refresh_cpu_specifics(cpu_refresh);

    let cpus = system.cpus();
    let mut result = vec![0u64; num_cpus];
    for (i, out) in result.iter_mut().enumerate() {
        *out = cpus.get(i).map(|c| c.frequency()).unwrap_or(0);
    }
    result
}
