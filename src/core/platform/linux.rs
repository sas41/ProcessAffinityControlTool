//! Linux topology discovery using hwloc + sysfs.

use super::{CacheLevelInfo, PlatformTopologyProvider, ThreadClassification, ThreadInfo};
use hwlocality::{
    object::{attributes::ObjectAttributes, types::ObjectType},
    topology::Topology,
};
use std::collections::HashMap;

pub struct LinuxProvider;

impl PlatformTopologyProvider for LinuxProvider {
    fn discover_threads(&self) -> Vec<ThreadInfo> {
        let topo = match Topology::new() {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        // Collect and sort PUs (logical CPUs).
        let mut pus: Vec<_> = topo.objects_with_type(ObjectType::PU).collect();
        pus.sort_by_key(|obj| obj.logical_index());

        // Dense physical core index map.
        let mut core_map: HashMap<usize, usize> = HashMap::new();
        let mut core_counter = 0usize;
        for obj in &pus {
            if let Some(parent) = obj.parent() {
                if parent.object_type() == ObjectType::Core {
                    let core_li = parent.logical_index();
                    core_map.entry(core_li).or_insert_with(|| {
                        let idx = core_counter;
                        core_counter += 1;
                        idx
                    });
                }
            }
        }

        // hwloc cpu_kinds: bulk P/E classification + frequency hints.
        let mut kind_map: HashMap<usize, ThreadClassification> = HashMap::new();
        let mut kind_freq_mhz: HashMap<usize, u64> = HashMap::new();
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
                let freq_mhz = extract_freq_mhz(&kind.infos);
                for bit in kind.cpuset.iter_set() {
                    let li = usize::from(bit);
                    kind_map.insert(li, classification);
                    if freq_mhz > 0 {
                        kind_freq_mhz.insert(li, freq_mhz);
                    }
                }
            }
        }

        // sysfs per-CPU max frequency (more accurate than hwloc hints).
        let sysfs_freq = read_sysfs_max_freq_mhz(pus.len());

        // Build per-core cache map by walking hwloc tree from each Core object.
        let mut core_caches: HashMap<usize, Vec<CacheLevelInfo>> = HashMap::new();
        for core_obj in topo.objects_with_type(ObjectType::Core) {
            let core_li = core_obj.logical_index();
            let phys_idx = *core_map.get(&core_li).unwrap_or(&core_li);
            let mut caches = Vec::new();

            let mut cur = core_obj.parent();
            while let Some(anc) = cur {
                match anc.object_type() {
                    ObjectType::Die | ObjectType::Package | ObjectType::Machine => break,
                    t if t.is_cpu_cache() => {
                        if let Some(ObjectAttributes::Cache(ca)) = anc.attributes() {
                            if let Some(sz) = ca.size() {
                                let level = ca.depth() as u8;
                                // group_id uses the cache object's logical index to
                                // distinguish separate physical caches at the same level.
                                caches.push(CacheLevelInfo {
                                    level,
                                    size_bytes: sz.get(),
                                    group_id: anc.logical_index() as isize,
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
            core_caches.insert(phys_idx, caches);
        }

        // Build a dense L3-group-id → CCD-index map so that raw hwloc logical
        // indices (which may be sparse) are normalized to 0..N for display.
        let mut l3_group_to_ccd: HashMap<isize, isize> = HashMap::new();
        {
            let mut raw_ids: Vec<isize> = core_caches
                .values()
                .filter_map(|caches| caches.iter().find(|c| c.level == 3).map(|c| c.group_id))
                .collect();
            raw_ids.sort();
            raw_ids.dedup();
            for (dense_idx, &raw_id) in raw_ids.iter().enumerate() {
                l3_group_to_ccd.insert(raw_id, dense_idx as isize);
            }
        }

        // Build final ThreadInfo records.
        let mut threads = Vec::with_capacity(pus.len());
        for obj in &pus {
            let li = obj.logical_index();

            let core_index = obj
                .parent()
                .filter(|p| p.object_type() == ObjectType::Core)
                .and_then(|p| core_map.get(&p.logical_index()).copied())
                .unwrap_or(li);

            let classification = kind_map
                .get(&li)
                .copied()
                .unwrap_or(ThreadClassification::Generic);

            let die_li = find_ancestor_li(obj, ObjectType::Die);
            let numa_li = find_ancestor_li(obj, ObjectType::NUMANode);

            let caches = core_caches.get(&core_index).cloned().unwrap_or_default();

            // CCD detection: prefer hwloc Die objects, fall back to L3 cache
            // grouping. Each distinct L3 cache instance represents one CCD on
            // AMD Ryzen. Even a single-CCD chip has one L3, yielding ccd_index=0.
            let ccd_index = if die_li >= 0 {
                die_li
            } else {
                caches
                    .iter()
                    .find(|c| c.level == 3)
                    .and_then(|c| l3_group_to_ccd.get(&c.group_id).copied())
                    .unwrap_or(-1)
            };

            // CCX is not directly exposed by hwloc; leave as -1.
            let ccx_index: isize = -1;

            let max_freq_mhz = sysfs_freq
                .get(li)
                .copied()
                .filter(|&v| v > 0)
                .or_else(|| kind_freq_mhz.get(&li).copied())
                .unwrap_or(0);

            // Base frequency: hwloc kinds often report base; sysfs reports max.
            let base_freq_mhz = kind_freq_mhz.get(&li).copied().unwrap_or(0);

            // Compute group: -1 for multi-CCD AMD (use CCD grouping), otherwise 0.
            let compute_group: isize = if classification == ThreadClassification::Performance
                || classification == ThreadClassification::Efficiency
            {
                match classification {
                    ThreadClassification::Performance => 0,
                    ThreadClassification::Efficiency => 1,
                    _ => 0,
                }
            } else if ccd_index >= 0 && l3_group_to_ccd.len() > 1 {
                -1 // AMD multi-CCD: use CCD grouping instead.
            } else {
                0 // Single-CCD or monolithic fallback.
            };

            threads.push(ThreadInfo {
                thread_index: li,
                core_index,
                base_freq_mhz,
                max_freq_mhz,
                caches,
                classification,
                ccx_index,
                ccd_index,
                numa_index: numa_li,
                compute_group,
            });
        }

        threads
    }
}

/// Walk hwloc ancestors to find the first object of the given type.
fn find_ancestor_li(obj: &hwlocality::object::TopologyObject, target: ObjectType) -> isize {
    let mut cur = obj.parent();
    while let Some(anc) = cur {
        if anc.object_type() == target {
            return anc.logical_index() as isize;
        }
        cur = anc.parent();
    }
    -1
}

/// Extract frequency in MHz from hwloc info key-value pairs.
fn extract_freq_mhz(infos: &[hwlocality::info::TextualInfo]) -> u64 {
    const KEYS: &[&str] = &[
        "FrequencyMaxMHz",
        "FrequencyBaseMHz",
        "BaseFrequencyMHz",
        "FrequencyMHz",
        "MaxFrequencyMHz",
    ];
    for &key in KEYS {
        for info in infos {
            if info.name().to_str().map(|n| n == key).unwrap_or(false) {
                if let Ok(v) = info.value().to_str().unwrap_or("").parse::<u64>() {
                    if v > 0 {
                        return v;
                    }
                }
            }
        }
    }
    0
}

/// Read per-CPU max frequency from sysfs, returned in MHz.
fn read_sysfs_max_freq_mhz(num_cpus: usize) -> Vec<u64> {
    fn read_khz(path: &str) -> Option<u64> {
        let raw = std::fs::read_to_string(path).ok()?;
        let val = raw.trim().parse::<u64>().ok()?;
        if val == 0 {
            return None;
        }
        // cpufreq files are usually kHz; ACPI CPPC files may be MHz.
        Some(if val < 20_000 { val * 1000 } else { val })
    }

    let mut result = vec![0u64; num_cpus];
    for (i, out) in result.iter_mut().enumerate() {
        let candidates = [
            format!("/sys/devices/system/cpu/cpu{i}/cpufreq/cpuinfo_max_freq"),
            format!("/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_max_freq"),
            format!("/sys/devices/system/cpu/cpu{i}/acpi_cppc/highest_freq"),
            format!("/sys/devices/system/cpu/cpu{i}/acpi_cppc/nominal_freq"),
        ];
        let khz = candidates
            .iter()
            .filter_map(|p| read_khz(p))
            .max()
            .unwrap_or(0);
        // Convert kHz to MHz.
        *out = khz / 1000;
    }
    result
}
