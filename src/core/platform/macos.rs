//! macOS topology discovery using hwloc.
//!
//! macOS does not expose sysfs or Win32 APIs, so we rely entirely on hwloc
//! for topology structure and fall back to sysinfo for frequency data.

use super::{CacheLevelInfo, PlatformTopologyProvider, ThreadClassification, ThreadInfo};
use hwlocality::{
    object::{attributes::ObjectAttributes, types::ObjectType},
    topology::Topology,
};
use std::collections::HashMap;

pub struct MacOsProvider;

impl PlatformTopologyProvider for MacOsProvider {
    fn discover_threads(&self) -> Vec<ThreadInfo> {
        let topo = match Topology::new() {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

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

        // hwloc cpu_kinds for Apple Silicon P/E classification.
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

        // sysinfo fallback for frequency.
        let sysinfo_freq = read_sysinfo_freq_mhz(pus.len());

        // Build per-core cache map.
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
                                caches.push(CacheLevelInfo {
                                    level: ca.depth() as u8,
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

            let numa_li = find_ancestor_li(obj, ObjectType::NUMANode);
            let caches = core_caches.get(&core_index).cloned().unwrap_or_default();

            let max_freq_mhz = kind_freq_mhz
                .get(&li)
                .copied()
                .or_else(|| sysinfo_freq.get(li).copied().filter(|&v| v > 0))
                .unwrap_or(0);
            let base_freq_mhz = kind_freq_mhz.get(&li).copied().unwrap_or(0);

            let compute_group = match classification {
                ThreadClassification::Performance => 0,
                ThreadClassification::Efficiency => 1,
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
                ccd_index: -1,
                numa_index: numa_li,
                compute_group,
            });
        }

        threads
    }
}

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
