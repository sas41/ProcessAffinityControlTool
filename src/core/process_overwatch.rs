//! Background process scanner and affinity/priority controller.
//!
//! Lifecycle:
//! - `ProcessOverwatch` holds shared state and policy.
//! - `ScanHandler::start` spawns one worker thread.
//! - `ScanHandler::stop` signals shutdown, joins the thread, then restores snapshots.
//!
//! Threading and message flow:
//! - Shared state uses `Arc<Mutex<...>>` so UI/control code and worker code can coordinate safely.
//! - `stop_flag` is an `AtomicBool` polled by the worker loop.
//! - `fresh_scan_requested` is a one-shot flag consumed each loop (`take_fresh_scan`).
//! - Auto-mode detections are rebuilt by the worker, then consumed by scan/apply logic.
//!
//! Rust quick map for C# readers (first-encounter syntax/symbols used here):
//! - `Arc<T>`: atomic ref-counted shared ownership (`System.Threading` + shared reference semantics).
//! - `Mutex<T>` + `.lock().unwrap()`: lock to access `T`; `unwrap()` is fail-fast like letting an unexpected exception terminate.
//! - `Option<T>` with `Some(v)` / `None`: nullable-like value without `null` (`T?`/`Nullable<T>` concept).
//! - `if let Some(x) = ...`: concise "if value exists" pattern.
//! - `match`: exhaustive pattern switch (`switch` expression with compile-time coverage checks).
//! - `&T` / `&mut T`: borrowed reference / mutable borrowed reference (`ref`-like access without ownership transfer).
//! - `*x` (for references/guards): dereference to the inner value.
//! - `#[cfg(...)]`: conditional compilation (similar role to `#if` target checks).
//! - `move || { ... }`: closure that captures and owns values (used for thread entry).

use crate::core::pact_config::PACTConfig;
use crate::core::process_config::ProcessPriority;

use std::collections::HashMap;
use std::collections::HashSet;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use std::thread;
use std::time::Duration;

use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    GetPriorityClass, GetProcessAffinityMask, OpenProcess, SetPriorityClass,
    SetProcessAffinityMask, PROCESS_ALL_ACCESS,
};

/// CPU usage and frequency snapshot.
#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    /// Per-core CPU usage percentage.
    pub per_core: Vec<f32>,

    /// Per-core frequency in MHz.
    pub per_core_mhz: Vec<u64>,

    /// Global average CPU usage percentage.
    pub global: f32,
}

/// Pre-modification process state used for restoration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OriginalProcessState {
    /// Original affinity mask, if affinity was modified.
    pub affinity_mask: Option<u64>,

    /// Original priority class, if priority was modified.
    pub priority_class: Option<u32>,

    /// Original Linux niceness, if priority was modified.
    pub niceness: Option<i32>,
}

/// Shared mutable state used by control code and scan thread.
struct ProcessOverwatchInner {
    /// User-configured settings.
    pub user_config: Mutex<PACTConfig>,

    /// Currently active settings.
    pub active_config: Mutex<PACTConfig>,

    /// Config mirrored while scanner is paused.
    pub paused_config: Mutex<PACTConfig>,

    /// PIDs currently managed.
    pub managed_processes: Mutex<HashSet<u32>>,

    /// PIDs that failed modification.
    pub protected_processes: Mutex<HashSet<u32>>,

    /// Processes currently managed via capture_sub_processes propagation.
    /// Each triple: (child_name_original_case, direct_parent_name_lower, group_name_lower).
    /// Empty string for group_name means a custom-process rule (no group).
    /// Replaced wholesale after every scan pass.
    pub capture_sub_processes_data: Mutex<Vec<(String, String, String)>>,

    /// One-shot full rescan request, consumed by worker loop.
    pub fresh_scan_requested: Mutex<bool>,

    /// Scanner activity flag (pause/resume without stopping thread).
    pub scanner_active: Mutex<bool>,

    /// Latest CPU statistics.
    pub cpu_stats: Mutex<CpuStats>,

    /// Snapshot of running process names.
    pub running_processes: Mutex<Vec<String>>,

    /// Names of processes that failed modification.
    pub protected_process_names: Mutex<Vec<String>>,

    /// Per-PID original state captured before first modification.
    pub original_states: Mutex<HashMap<u32, OriginalProcessState>>,
}

/// Public controller for policy and shared scan state.
#[derive(Clone)]
pub struct ProcessOverwatch {
    inner: Arc<ProcessOverwatchInner>,
}

impl ProcessOverwatch {
    /// Creates shared state; thread is started separately by `ScanHandler`.
    pub fn new(user_config: PACTConfig) -> Self {
        let paused_config = PACTConfig::default();
        let active_config = user_config.clone();
        Self {
            inner: Arc::new(ProcessOverwatchInner {
                user_config: Mutex::new(user_config),
                active_config: Mutex::new(active_config),
                paused_config: Mutex::new(paused_config),
                managed_processes: Mutex::new(HashSet::new()),
                protected_processes: Mutex::new(HashSet::new()),
                capture_sub_processes_data: Mutex::new(Vec::new()),
                fresh_scan_requested: Mutex::new(false),
                scanner_active: Mutex::new(true),
                cpu_stats: Mutex::new(CpuStats::default()),
                running_processes: Mutex::new(Vec::new()),
                protected_process_names: Mutex::new(Vec::new()),
                original_states: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Returns whether the scanner thread is active.
    pub fn is_scanner_active(&self) -> bool {
        *self.inner.scanner_active.lock().unwrap()
    }

    /// Returns the number of managed processes.
    pub fn managed_process_count(&self) -> usize {
        self.inner.managed_processes.lock().unwrap().len()
    }

    /// Returns the number of protected processes.
    pub fn protected_process_count(&self) -> usize {
        self.inner.protected_processes.lock().unwrap().len()
    }

    /// Returns the scan interval in milliseconds.
    pub fn scan_interval(&self) -> u64 {
        self.inner.user_config.lock().unwrap().scan_interval
    }

    /// Updates the scan interval in milliseconds.
    pub fn set_scan_interval(&self, ms: u64) {
        self.inner.user_config.lock().unwrap().scan_interval = ms;
    }

    /// Returns a copy of the latest CPU statistics.
    pub fn cpu_stats(&self) -> CpuStats {
        self.inner.cpu_stats.lock().unwrap().clone()
    }

    /// Returns a copy of current running process names.
    pub fn running_processes(&self) -> Vec<String> {
        self.inner.running_processes.lock().unwrap().clone()
    }

    /// Returns a copy of process names that failed modification.
    pub fn protected_process_names(&self) -> Vec<String> {
        self.inner.protected_process_names.lock().unwrap().clone()
    }

    /// Locks and returns the user config.
    pub fn user_config_lock(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
    }

    /// Locks and returns the user config for mutation.
    pub fn user_config_lock_mut(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
    }

    /// Returns a snapshot of processes managed via capture_sub_processes propagation.
    /// Each triple: (child_name, direct_parent_name_lower, group_name_lower).
    pub fn capture_sub_processes_data(&self) -> Vec<(String, String, String)> {
        self.inner
            .capture_sub_processes_data
            .lock()
            .unwrap()
            .clone()
    }

    /// Atomically reads and clears the one-shot fresh-scan request.
    pub fn take_fresh_scan(&self) -> bool {
        let mut flag = self.inner.fresh_scan_requested.lock().unwrap();
        let was = *flag;
        *flag = false;
        was
    }

    /// Pauses/resumes applying policy by swapping active config.
    pub fn toggle_process_overwatch(&self) -> bool {
        let mut active = self.inner.scanner_active.lock().unwrap();
        if *active {
            *self.inner.active_config.lock().unwrap() =
                self.inner.paused_config.lock().unwrap().clone();
            *active = false;
        } else {
            *self.inner.active_config.lock().unwrap() =
                self.inner.user_config.lock().unwrap().clone();
            *active = true;
        }
        *active
    }

    /// Requests a full rescan on the next scan cycle.
    /// Also clears stale capture data immediately so the UI doesn't show an
    /// outdated tree between the config change and the next scan completing.
    pub fn request_fresh_scan(&self) {
        *self.inner.fresh_scan_requested.lock().unwrap() = true;
        self.inner
            .capture_sub_processes_data
            .lock()
            .unwrap()
            .clear();
    }

    /// Refreshes and returns CPU usage and frequency stats.
    pub fn refresh_cpu_stats(system: &mut System) -> CpuStats {
        system.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage().with_frequency());

        let cpus = system.cpus();
        let per_core: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();
        let per_core_mhz: Vec<u64> = cpus.iter().map(|c| c.frequency()).collect();
        let global = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f32>() / per_core.len() as f32
        };
        CpuStats {
            per_core,
            per_core_mhz,
            global,
        }
    }

    /// Main scan pass: resolve policy per process, snapshot once, then apply.
    pub fn scan_and_manage(&self, system: &System, forced: bool) {
        if forced {
            self.inner.managed_processes.lock().unwrap().clear();
        }

        self.inner.protected_processes.lock().unwrap().clear();

        let mut current_set = HashSet::new();
        let mut protected_names: Vec<String> = Vec::new();

        let mut all_names: Vec<String> = system
            .processes()
            .values()
            .map(|p| p.name().to_string_lossy().into_owned())
            .collect();

        all_names.sort_unstable();

        // Build PID → parent PID and PID → lowercase name maps for child propagation.
        // These are used below to walk ancestry when a process has no direct match but
        // an ancestor has capture_sub_processes enabled.
        let mut pid_to_parent: HashMap<u32, u32> = HashMap::new();
        let mut pid_to_name_lc: HashMap<u32, String> = HashMap::new();
        for (pid, proc) in system.processes() {
            let pid_u32 = pid.as_u32();
            pid_to_name_lc.insert(pid_u32, proc.name().to_string_lossy().to_lowercase());
            if let Some(ppid) = proc.parent() {
                pid_to_parent.insert(pid_u32, ppid.as_u32());
            }
        }

        // Pre-build a map of lowercase process name → inherited policy for all
        // custom processes and explicitly assigned group processes that have
        // capture_sub_processes = true and at least one effective setting.
        // Tuple: (affinity_mask, priority, niceness, group_name_lower).
        // group_name_lower is empty for custom-process seeds (no group).
        type InheritedPolicy = (u64, Option<ProcessPriority>, Option<i32>, String);
        let child_policies: HashMap<String, InheritedPolicy> = {
            let cfg = self.inner.user_config.lock().unwrap();
            let mut map: HashMap<String, InheritedPolicy> = HashMap::new();

            // Custom processes take precedence (mirror normal resolution order).
            for cp in &cfg.custom_processes {
                if !cp.capture_sub_processes {
                    continue;
                }
                let mask = cp.affinity.as_ref().map_or(0, |a| a.affinity_mask);
                #[cfg(target_os = "linux")]
                let niceness = cp
                    .niceness
                    .or_else(|| cp.priority.as_ref().map(Self::priority_to_niceness));
                #[cfg(not(target_os = "linux"))]
                let niceness: Option<i32> = None;
                // Always seed child_policies even when no settings are applied.
                // Children are still captured and shown in the tree; the scanner's
                // emergent-blacklist check later decides whether to modify them.
                map.insert(
                    cp.name.to_lowercase(),
                    (mask, cp.priority.clone(), niceness, String::new()),
                );
            }

            // Explicit group assignments.
            for (proc_name, group_name) in cfg.process_assignments.iter() {
                if map.contains_key(proc_name) {
                    continue; // custom process entry already present
                }
                if let Some(g) = cfg.group_by_name(group_name) {
                    if !g.capture_sub_processes {
                        continue;
                    }
                    let mask = g.affinity.as_ref().map_or(0, |a| a.affinity_mask);
                    #[cfg(target_os = "linux")]
                    let niceness = g
                        .niceness
                        .or_else(|| g.priority.as_ref().map(Self::priority_to_niceness));
                    #[cfg(not(target_os = "linux"))]
                    let niceness: Option<i32> = None;
                    // Always seed child_policies even when no settings are applied.
                    // Children are still captured and visible in the tree.
                    map.insert(
                        proc_name.clone(),
                        (mask, g.priority.clone(), niceness, group_name.clone()),
                    );
                }
            }

            map
        };

        // Accumulates (child_name, direct_parent_lc, group_lc) triples this scan pass.
        let mut new_capture_sub_processes: Vec<(String, String, String)> = Vec::new();

        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();

            {
                let managed = self.inner.managed_processes.lock().unwrap();
                if managed.contains(&pid_u32) {
                    current_set.insert(pid_u32);
                    continue;
                }
            }

            let process_name = process.name().to_string_lossy().into_owned();

            // Resolution hierarchy (highest to lowest priority):
            // 1. Custom process rule
            // 2. Explicit group assignment
            // 3. Capture sub-processes (ancestor walk)
            // 4. Default group
            // A group/custom-process with no affinity and no priority is an
            // emergent blacklist — the process is tracked but not modified.
            let (affinity_mask, priority, niceness) = {
                let cfg = self.inner.user_config.lock().unwrap();

                // Priority 1: Custom per-process rule.
                if let Some(cp) = cfg.custom_process(&process_name) {
                    let mask = cp.affinity.as_ref().map_or(0, |a| a.affinity_mask);
                    #[cfg(target_os = "linux")]
                    let niceness = cp
                        .niceness
                        .or_else(|| cp.priority.as_ref().map(Self::priority_to_niceness));
                    #[cfg(not(target_os = "linux"))]
                    let niceness = None;
                    (mask, cp.priority.clone(), niceness)
                }
                // Priority 2: Explicit group assignment (no default fallback).
                else if let Some(group_name) = cfg.explicit_group_of(&process_name) {
                    match cfg.group_by_name(group_name) {
                        None => continue, // orphaned assignment
                        Some(g) => {
                            let mask = g.affinity.as_ref().map(|a| a.affinity_mask);
                            #[cfg(target_os = "linux")]
                            let niceness = g
                                .niceness
                                .or_else(|| g.priority.as_ref().map(Self::priority_to_niceness));
                            #[cfg(not(target_os = "linux"))]
                            let niceness = None;
                            (mask.unwrap_or(0), g.priority.clone(), niceness)
                        }
                    }
                } else {
                    // Priority 3: Capture sub-processes — walk ancestry chain.
                    // The while loop handles arbitrary nesting depth.
                    let direct_parent_lc = pid_to_parent
                        .get(&pid_u32)
                        .and_then(|ppid| pid_to_name_lc.get(ppid))
                        .cloned()
                        .unwrap_or_default();

                    let mut ancestor = pid_to_parent.get(&pid_u32).copied();
                    let mut inherited: Option<InheritedPolicy> = None;
                    while let Some(ppid) = ancestor {
                        if let Some(parent_name) = pid_to_name_lc.get(&ppid) {
                            if let Some(policy) = child_policies.get(parent_name) {
                                inherited = Some(policy.clone());
                                break;
                            }
                        }
                        ancestor = pid_to_parent.get(&ppid).copied();
                    }

                    if let Some((mask, pri, nic, group_lc)) = inherited {
                        new_capture_sub_processes.push((
                            process_name.clone(),
                            direct_parent_lc,
                            group_lc,
                        ));
                        (mask, pri, nic)
                    } else {
                        // Priority 4: Default group (lowest).
                        match cfg.default_group() {
                            None => continue,
                            Some(g) => {
                                let mask = g.affinity.as_ref().map(|a| a.affinity_mask);
                                #[cfg(target_os = "linux")]
                                let niceness = g.niceness.or_else(|| {
                                    g.priority.as_ref().map(Self::priority_to_niceness)
                                });
                                #[cfg(not(target_os = "linux"))]
                                let niceness = None;
                                (mask.unwrap_or(0), g.priority.clone(), niceness)
                            }
                        }
                    }
                }
            };

            let want_affinity = affinity_mask != 0;
            let want_priority = priority.is_some() || niceness.is_some();

            // Emergent blacklist: group/rule applies no settings — track but skip.
            if !want_affinity && !want_priority {
                current_set.insert(pid_u32);
                continue;
            }

            if want_affinity || want_priority {
                // Snapshot original state once so we can restore on shutdown.
                let already_snapshotted = {
                    self.inner
                        .original_states
                        .lock()
                        .unwrap()
                        .contains_key(&pid_u32)
                };

                if !already_snapshotted {
                    if let Some(snapshot) =
                        Self::read_original_state(pid_u32, want_affinity, want_priority)
                    {
                        self.inner
                            .original_states
                            .lock()
                            .unwrap()
                            .insert(pid_u32, snapshot);
                    }
                }
            }

            let ok = Self::apply_to_process(pid_u32, affinity_mask, priority, niceness);

            if ok {
                current_set.insert(pid_u32);
            } else {
                self.inner
                    .protected_processes
                    .lock()
                    .unwrap()
                    .insert(pid_u32);
                protected_names.push(process_name);
            }
        }

        *self.inner.managed_processes.lock().unwrap() = current_set;
        *self.inner.running_processes.lock().unwrap() = all_names;
        *self.inner.protected_process_names.lock().unwrap() = protected_names;

        // On a forced scan every process was re-resolved from scratch, so
        // new_capture_sub_processes is the complete and authoritative picture.
        // Replace entirely to avoid stale entries (e.g. a child process whose
        // parent moved to a different capture group).
        //
        // On incremental scans, managed_processes was not cleared, so child
        // processes that were already managed were skipped before they could be
        // re-recorded.  In that case we keep existing entries for processes that
        // are still running and only append freshly discovered children.
        if forced {
            *self.inner.capture_sub_processes_data.lock().unwrap() = new_capture_sub_processes;
        } else {
            let running_lc: HashSet<String> = self
                .inner
                .running_processes
                .lock()
                .unwrap()
                .iter()
                .map(|n| n.to_lowercase())
                .collect();
            let mut acd = self.inner.capture_sub_processes_data.lock().unwrap();
            // Drop entries for processes that are no longer running.
            acd.retain(|(child, _, _)| running_lc.contains(&child.to_lowercase()));
            // Append newly discovered children, skipping duplicates.
            let existing_lc: HashSet<String> =
                acd.iter().map(|(c, _, _)| c.to_lowercase()).collect();
            for entry in new_capture_sub_processes {
                if !existing_lc.contains(&entry.0.to_lowercase()) {
                    acd.push(entry);
                }
            }
        }
    }

    /// Restores all snapshotted processes to pre-modification values.
    pub fn restore_all(&self) {
        let states = self.inner.original_states.lock().unwrap().clone();
        for (pid, original) in &states {
            Self::restore_process(*pid, original);
        }
    }

    /// Restores matching process names and drops their saved snapshots.
    pub fn restore_processes_by_name(&self, names: &std::collections::HashSet<String>) {
        if names.is_empty() {
            return;
        }

        let pids_to_restore: Vec<(u32, OriginalProcessState)> = {
            let states = self.inner.original_states.lock().unwrap();
            states.iter().map(|(&pid, s)| (pid, s.clone())).collect()
        };

        let lower_names: std::collections::HashSet<String> =
            names.iter().map(|n| n.to_lowercase()).collect();

        for (pid, original) in &pids_to_restore {
            if let Some(name) = Self::process_name_for_pid(*pid) {
                if lower_names.contains(&name.to_lowercase()) {
                    Self::restore_process(*pid, original);
                    self.inner.original_states.lock().unwrap().remove(pid);
                }
            }
        }
    }

    /// Returns process name for a PID, if available.
    #[cfg(target_os = "windows")]
    fn process_name_for_pid(pid: u32) -> Option<String> {
        unsafe {
            use windows::core::PWSTR;
            use windows::Win32::System::Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            };

            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = vec![0u16; 260];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
            .is_ok();

            let _ = windows::Win32::Foundation::CloseHandle(handle);
            if !ok {
                return None;
            }

            let full: String = String::from_utf16_lossy(&buf[..len as usize]);
            Some(
                std::path::Path::new(&full)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or(full),
            )
        }
    }

    /// Returns process name from `/proc/<pid>/comm`.
    #[cfg(target_os = "linux")]
    fn process_name_for_pid(pid: u32) -> Option<String> {
        std::fs::read_to_string(format!("/proc/{}/comm", pid))
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Unsupported platforms return no process name.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn process_name_for_pid(_pid: u32) -> Option<String> {
        None
    }

    /// Reads current process values before first overwrite (Windows).
    #[cfg(target_os = "windows")]
    fn read_original_state(
        pid: u32,
        read_affinity: bool,
        read_priority: bool,
    ) -> Option<OriginalProcessState> {
        unsafe {
            let handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).ok()?;

            let affinity_mask = if read_affinity {
                let mut proc_mask: usize = 0;
                let mut sys_mask: usize = 0;

                if GetProcessAffinityMask(handle, &mut proc_mask, &mut sys_mask).is_ok() {
                    Some(proc_mask as u64)
                } else {
                    None
                }
            } else {
                None
            };

            let priority_class = if read_priority {
                let cls = GetPriorityClass(handle);
                if cls != 0 {
                    Some(cls)
                } else {
                    None
                }
            } else {
                None
            };

            let _ = windows::Win32::Foundation::CloseHandle(handle);

            if affinity_mask.is_some() || priority_class.is_some() {
                Some(OriginalProcessState {
                    affinity_mask,
                    priority_class,
                    niceness: None,
                })
            } else {
                None
            }
        }
    }

    /// Reads affinity mask before first overwrite (Linux).
    #[cfg(target_os = "linux")]
    fn read_original_state(
        pid: u32,
        read_affinity: bool,
        read_priority: bool,
    ) -> Option<OriginalProcessState> {
        use nix::sched::sched_getaffinity;
        use nix::unistd::Pid as NixPid;

        let affinity_mask = if read_affinity {
            let nix_pid = NixPid::from_raw(pid as i32);
            sched_getaffinity(nix_pid).ok().map(|cpu_set| {
                let mut mask = 0u64;
                for bit in 0..64usize {
                    if cpu_set.is_set(bit).unwrap_or(false) {
                        mask |= 1u64 << bit;
                    }
                }
                mask
            })
        } else {
            None
        };

        let niceness = if read_priority {
            use nix::errno::Errno;
            unsafe {
                Errno::clear();
                let v = libc::getpriority(libc::PRIO_PROCESS, pid);
                let err = Errno::last_raw();
                if v == -1 && err != 0 {
                    None
                } else {
                    Some(v)
                }
            }
        } else {
            None
        };

        if affinity_mask.is_some() || niceness.is_some() {
            Some(OriginalProcessState {
                affinity_mask,
                priority_class: None,
                niceness,
            })
        } else {
            None
        }
    }

    /// Unsupported platforms do not snapshot state.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn read_original_state(
        _pid: u32,
        _read_affinity: bool,
        _read_priority: bool,
    ) -> Option<OriginalProcessState> {
        None
    }

    /// Restores one process from its recorded state (Windows).
    #[cfg(target_os = "windows")]
    fn restore_process(pid: u32, original: &OriginalProcessState) {
        unsafe {
            let handle = match OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
                Ok(h) => h,
                Err(_) => return,
            };

            if let Some(mask) = original.affinity_mask {
                let _ = SetProcessAffinityMask(handle, mask as usize);
            }

            if let Some(cls) = original.priority_class {
                use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;
                let _ = SetPriorityClass(handle, PROCESS_CREATION_FLAGS(cls));
            }

            let _ = windows::Win32::Foundation::CloseHandle(handle);
        }
    }

    /// Restores process affinity from recorded mask (Linux).
    #[cfg(target_os = "linux")]
    fn restore_process(pid: u32, original: &OriginalProcessState) {
        use nix::sched::{sched_setaffinity, CpuSet};
        use nix::unistd::Pid as NixPid;

        if let Some(mask) = original.affinity_mask {
            let nix_pid = NixPid::from_raw(pid as i32);
            let mut cpu_set = CpuSet::new();
            for bit in 0..64usize {
                if (mask >> bit) & 1 == 1 {
                    let _ = cpu_set.set(bit);
                }
            }
            let _ = sched_setaffinity(nix_pid, &cpu_set);
        }

        if let Some(nice) = original.niceness {
            unsafe {
                let _ = libc::setpriority(libc::PRIO_PROCESS, pid, nice);
            }
        }
    }

    /// Unsupported platforms perform no restoration.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn restore_process(_pid: u32, _original: &OriginalProcessState) {}

    /// Applies affinity/priority and reports success.
    fn apply_to_process(
        pid: u32,
        affinity_mask: u64,
        priority: Option<ProcessPriority>,
        niceness: Option<i32>,
    ) -> bool {
        if affinity_mask == 0 && priority.is_none() && niceness.is_none() {
            return true;
        }
        Self::set_process_affinity_and_priority_impl(pid, affinity_mask, priority, niceness)
    }

    /// Applies affinity and priority via Windows APIs.
    #[cfg(target_os = "windows")]
    fn set_process_affinity_and_priority_impl(
        pid: u32,
        affinity_mask: u64,
        priority: Option<ProcessPriority>,
        _niceness: Option<i32>,
    ) -> bool {
        use windows::Win32::System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
            IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, REALTIME_PRIORITY_CLASS,
        };

        unsafe {
            let handle = match OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
                Ok(h) => h,
                Err(_) => return false,
            };

            let mut ok = true;

            if affinity_mask != 0 {
                ok &= SetProcessAffinityMask(handle, affinity_mask as usize).is_ok();
            }

            if let Some(p) = priority {
                let cls = match p {
                    ProcessPriority::Idle => IDLE_PRIORITY_CLASS,
                    ProcessPriority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
                    ProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
                    ProcessPriority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
                    ProcessPriority::High => HIGH_PRIORITY_CLASS,
                    ProcessPriority::RealTime => REALTIME_PRIORITY_CLASS,
                };
                ok &= SetPriorityClass(handle, cls).is_ok();
            }

            let _ = windows::Win32::Foundation::CloseHandle(handle);

            ok
        }
    }

    /// Applies affinity on Linux. Priority is ignored.
    #[cfg(target_os = "linux")]
    fn set_process_affinity_and_priority_impl(
        pid: u32,
        affinity_mask: u64,
        priority: Option<ProcessPriority>,
        niceness: Option<i32>,
    ) -> bool {
        use nix::sched::{sched_setaffinity, CpuSet};
        use nix::unistd::Pid as NixPid;

        let mut ok = true;

        if affinity_mask != 0 {
            let nix_pid = NixPid::from_raw(pid as i32);
            let mut cpu_set = CpuSet::new();
            for bit in 0..64usize {
                if (affinity_mask >> bit) & 1 == 1 {
                    let _ = cpu_set.set(bit);
                }
            }

            ok &= sched_setaffinity(nix_pid, &cpu_set).is_ok();
        }

        let resolved_niceness =
            niceness.or_else(|| priority.as_ref().map(Self::priority_to_niceness));
        if let Some(nice) = resolved_niceness {
            unsafe {
                ok &= libc::setpriority(libc::PRIO_PROCESS, pid, nice.clamp(-20, 19)) == 0;
            }
        }

        ok
    }

    /// Unsupported platforms cannot apply changes.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn set_process_affinity_and_priority_impl(
        _pid: u32,
        _affinity_mask: u64,
        _priority: Option<ProcessPriority>,
        _niceness: Option<i32>,
    ) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    fn priority_to_niceness(priority: &ProcessPriority) -> i32 {
        match priority {
            ProcessPriority::Idle => 19,
            ProcessPriority::BelowNormal => 10,
            ProcessPriority::Normal => 0,
            ProcessPriority::AboveNormal => -5,
            ProcessPriority::High => -10,
            ProcessPriority::RealTime => -20,
        }
    }
}

/// Drop is intentionally a no-op.
impl Drop for ProcessOverwatch {
    fn drop(&mut self) {}
}

/// Owns the worker thread lifecycle.
pub struct ScanHandler {
    /// Shared controller cloned into the worker thread.
    process_overwatch: ProcessOverwatch,

    /// Scan interval in milliseconds.
    scan_interval: u64,

    /// Stop signal polled by the worker loop.
    stop_flag: Arc<AtomicBool>,

    /// Join handle for the worker thread.
    handle: Option<thread::JoinHandle<()>>,
}

impl ScanHandler {
    /// Creates a new scan handler.
    pub fn new(process_overwatch: ProcessOverwatch, scan_interval: u64) -> Self {
        Self {
            process_overwatch,
            scan_interval,
            stop_flag: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Starts the worker loop.
    ///
    /// Rust note: `move` transfers captured values into the new thread closure.
    pub fn start(&mut self) {
        let overwatch = self.process_overwatch.clone();
        let scan_interval = self.scan_interval;
        let stop_flag = Arc::clone(&self.stop_flag);

        self.handle = Some(thread::spawn(move || {
            // Keep CPU and process refreshers separate to avoid stale CPU deltas on Linux.
            let mut cpu_sys = System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency()),
            );

            let mut proc_sys = System::new_with_specifics(
                RefreshKind::nothing().with_processes(sysinfo::ProcessRefreshKind::everything()),
            );

            cpu_sys
                .refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage().with_frequency());
            thread::sleep(Duration::from_millis(500));

            while !stop_flag.load(Ordering::Relaxed) {
                proc_sys.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::All,
                    true,
                    sysinfo::ProcessRefreshKind::everything(),
                );

                let stats = ProcessOverwatch::refresh_cpu_stats(&mut cpu_sys);
                *overwatch.inner.cpu_stats.lock().unwrap() = stats;

                if overwatch.is_scanner_active() {
                    let fresh = overwatch.take_fresh_scan();
                    overwatch.scan_and_manage(&proc_sys, fresh);
                }

                thread::sleep(Duration::from_millis(scan_interval));
            }
        }));
    }

    /// Requests shutdown, waits for worker exit, then restores snapshots.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);

        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }

        self.process_overwatch.restore_all();
    }
}
