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

use crate::core::pact_config::{CaseInsensitiveHashSet, PACTConfig};
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
    GetPriorityClass, GetProcessAffinityMask, OpenProcess, PROCESS_ALL_ACCESS, SetPriorityClass,
    SetProcessAffinityMask,
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

    /// Whether auto-mode detection is enabled.
    pub auto_mode: Mutex<bool>,

    /// Auto-mode detections.
    pub auto_mode_detections: Mutex<CaseInsensitiveHashSet>,

    /// Child PID to parent name cache for auto-mode checks.
    pub child_parent_pairs: Mutex<HashMap<u32, String>>,

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
                auto_mode: Mutex::new(true),
                auto_mode_detections: Mutex::new(CaseInsensitiveHashSet::new()),
                child_parent_pairs: Mutex::new(HashMap::new()),
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

    /// Returns whether auto mode is enabled.
    pub fn is_auto_mode(&self) -> bool {
        *self.inner.auto_mode.lock().unwrap()
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

    /// Returns sorted auto-mode detections.
    pub fn auto_mode_detections_list(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .inner
            .auto_mode_detections
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect();

        v.sort();
        v
    }

    /// Locks and returns the user config.
    pub fn user_config_lock(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
    }

    /// Locks and returns the user config for mutation.
    pub fn user_config_lock_mut(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
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

    /// Toggles auto mode and returns the new state.
    pub fn toggle_auto_mode(&self) -> bool {
        let has_auto_mode_group = self
            .inner
            .user_config
            .lock()
            .unwrap()
            .auto_mode_group()
            .is_some();

        if !has_auto_mode_group {
            *self.inner.auto_mode.lock().unwrap() = false;
            return false;
        }

        let mut m = self.inner.auto_mode.lock().unwrap();
        *m = !*m;
        *m
    }

    /// Requests a full rescan on the next scan cycle.
    pub fn request_fresh_scan(&self) {
        *self.inner.fresh_scan_requested.lock().unwrap() = true;
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

    /// Rebuilds parent cache and auto-mode detections from live process data.
    pub fn update_child_parent_pairs(&self, system: &System) {
        let mut pairs: HashMap<u32, String> = HashMap::new();
        let mut detections = CaseInsensitiveHashSet::new();

        for (pid, process) in system.processes() {
            let name_lc = process.name().to_string_lossy().to_lowercase();

            if name_lc == "idle" || name_lc == "system" {
                continue;
            }

            let pid_u32 = pid.as_u32();
            {
                let cached = self.inner.child_parent_pairs.lock().unwrap();
                if let Some(parent) = cached.get(&pid_u32) {
                    pairs.insert(pid_u32, parent.clone());
                    if !parent.is_empty() {
                        let cfg = self.inner.user_config.lock().unwrap();
                        if cfg.auto_mode_launchers.contains(parent.as_str()) {
                            detections.insert(process.name().to_string_lossy().into_owned());
                        }
                    }
                    continue;
                }
            }

            if let Some(ppid) = process.parent() {
                if let Some(parent_proc) = system.process(ppid) {
                    let parent_name = parent_proc.name().to_string_lossy().into_owned();
                    pairs.insert(pid_u32, parent_name.clone());
                    let cfg = self.inner.user_config.lock().unwrap();
                    if cfg.auto_mode_launchers.contains(&parent_name) {
                        detections.insert(process.name().to_string_lossy().into_owned());
                    }
                }
            }
        }

        *self.inner.child_parent_pairs.lock().unwrap() = pairs;
        *self.inner.auto_mode_detections.lock().unwrap() = detections;
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
        all_names.dedup();

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

            let (affinity_mask, priority, is_blacklist) = {
                let cfg = self.inner.user_config.lock().unwrap();

                // Custom per-process rule overrides group-based assignment.
                if let Some(cp) = cfg.custom_process(&process_name) {
                    let mask = cp.affinity.as_ref().map_or(0, |a| a.affinity_mask);
                    (mask, cp.priority.clone(), false)
                } else {
                    let auto_mode = *self.inner.auto_mode.lock().unwrap();
                    let auto_detections = self.inner.auto_mode_detections.lock().unwrap();

                    // Auto mode only applies when process has no explicit assignment.
                    let is_auto_detected = auto_mode
                        && auto_detections.contains(&process_name)
                        && cfg.process_assignments.get(&process_name).is_none();

                    let group = if is_auto_detected {
                        cfg.auto_mode_group()
                    } else {
                        cfg.group_for_process(&process_name)
                    };

                    match group {
                        None => continue,
                        Some(g) if g.is_blacklist => (0u64, None, true),
                        Some(g) => {
                            let mask = g.affinity.as_ref().map(|a| a.affinity_mask);
                            (mask.unwrap_or(0), g.priority.clone(), false)
                        }
                    }
                }
            };

            if is_blacklist {
                continue;
            }

            let want_affinity = affinity_mask != 0;
            let want_priority = priority.is_some();

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

            let ok = Self::apply_to_process(pid_u32, affinity_mask, priority);

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
            use windows::Win32::System::Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            };
            use windows::core::PWSTR;

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
                if cls != 0 { Some(cls) } else { None }
            } else {
                None
            };

            let _ = windows::Win32::Foundation::CloseHandle(handle);

            if affinity_mask.is_some() || priority_class.is_some() {
                Some(OriginalProcessState {
                    affinity_mask,
                    priority_class,
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
        _read_priority: bool,
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

        if affinity_mask.is_some() {
            Some(OriginalProcessState {
                affinity_mask,
                priority_class: None,
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
        use nix::sched::{CpuSet, sched_setaffinity};
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
    }

    /// Unsupported platforms perform no restoration.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn restore_process(_pid: u32, _original: &OriginalProcessState) {}

    /// Applies affinity/priority and reports success.
    fn apply_to_process(pid: u32, affinity_mask: u64, priority: Option<ProcessPriority>) -> bool {
        if affinity_mask == 0 && priority.is_none() {
            return true;
        }
        Self::set_process_affinity_and_priority_impl(pid, affinity_mask, priority)
    }

    /// Applies affinity and priority via Windows APIs.
    #[cfg(target_os = "windows")]
    fn set_process_affinity_and_priority_impl(
        pid: u32,
        affinity_mask: u64,
        priority: Option<ProcessPriority>,
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
        _priority: Option<ProcessPriority>,
    ) -> bool {
        use nix::sched::{CpuSet, sched_setaffinity};
        use nix::unistd::Pid as NixPid;

        if affinity_mask == 0 {
            return true;
        }

        let nix_pid = NixPid::from_raw(pid as i32);
        let mut cpu_set = CpuSet::new();
        for bit in 0..64usize {
            if (affinity_mask >> bit) & 1 == 1 {
                let _ = cpu_set.set(bit);
            }
        }

        sched_setaffinity(nix_pid, &cpu_set).is_ok()
    }

    /// Unsupported platforms cannot apply changes.
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn set_process_affinity_and_priority_impl(
        _pid: u32,
        _affinity_mask: u64,
        _priority: Option<ProcessPriority>,
    ) -> bool {
        false
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
                    if overwatch.is_auto_mode() {
                        overwatch.update_child_parent_pairs(&proc_sys);
                    }

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
