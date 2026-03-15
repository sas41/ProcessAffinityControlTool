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

// ─── CpuStats ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    pub per_core: Vec<f32>,
    /// Per-core frequency in MHz (same length as per_core, 0 if unavailable).
    pub per_core_mhz: Vec<u64>,
    pub global: f32,
}

// ─── OriginalProcessState ────────────────────────────────────────────────────

/// The pre-modification affinity and/or priority of a single process.
///
/// Only attributes that were actually changed are stored — an `Option::None`
/// means "we did not touch this attribute, so there is nothing to restore".
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OriginalProcessState {
    /// The affinity mask the process had **before** we first set it.
    /// `None` if we never touched affinity for this process.
    pub affinity_mask: Option<u64>,

    /// The Windows PROCESS_CREATION_FLAGS priority-class value (or equivalent
    /// on Linux) the process had before we first changed it.
    /// `None` if we never touched priority for this process.
    pub priority_class: Option<u32>,
}

// ─── Inner shared state ───────────────────────────────────────────────────────

struct ProcessOverwatchInner {
    pub user_config: Mutex<PACTConfig>,
    pub active_config: Mutex<PACTConfig>,
    pub paused_config: Mutex<PACTConfig>,

    pub managed_processes: Mutex<HashSet<u32>>,
    pub protected_processes: Mutex<HashSet<u32>>,

    pub auto_mode: Mutex<bool>,
    pub auto_mode_detections: Mutex<CaseInsensitiveHashSet>,
    pub child_parent_pairs: Mutex<HashMap<u32, String>>,

    pub fresh_scan_requested: Mutex<bool>,
    pub scanner_active: Mutex<bool>,

    pub cpu_stats: Mutex<CpuStats>,
    pub running_processes: Mutex<Vec<String>>,
    pub protected_process_names: Mutex<Vec<String>>,

    /// Snapshot of original process state taken just before we first modify
    /// each process.  Keyed by PID.  Populated lazily; never removed while
    /// the program is running so that we can always restore on exit.
    pub original_states: Mutex<HashMap<u32, OriginalProcessState>>,
}

// ─── ProcessOverwatch ────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ProcessOverwatch {
    inner: Arc<ProcessOverwatchInner>,
}

impl ProcessOverwatch {
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

    // ── Read accessors ────────────────────────────────────────────────────

    pub fn is_scanner_active(&self) -> bool {
        *self.inner.scanner_active.lock().unwrap()
    }

    pub fn is_auto_mode(&self) -> bool {
        *self.inner.auto_mode.lock().unwrap()
    }

    pub fn managed_process_count(&self) -> usize {
        self.inner.managed_processes.lock().unwrap().len()
    }

    pub fn protected_process_count(&self) -> usize {
        self.inner.protected_processes.lock().unwrap().len()
    }

    pub fn scan_interval(&self) -> u64 {
        self.inner.user_config.lock().unwrap().scan_interval
    }

    pub fn set_scan_interval(&self, ms: u64) {
        self.inner.user_config.lock().unwrap().scan_interval = ms;
    }

    pub fn cpu_stats(&self) -> CpuStats {
        self.inner.cpu_stats.lock().unwrap().clone()
    }

    pub fn running_processes(&self) -> Vec<String> {
        self.inner.running_processes.lock().unwrap().clone()
    }

    pub fn protected_process_names(&self) -> Vec<String> {
        self.inner.protected_process_names.lock().unwrap().clone()
    }

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

    pub fn user_config_lock(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
    }

    pub fn user_config_lock_mut(&self) -> std::sync::MutexGuard<'_, PACTConfig> {
        self.inner.user_config.lock().unwrap()
    }

    pub fn take_fresh_scan(&self) -> bool {
        let mut flag = self.inner.fresh_scan_requested.lock().unwrap();
        let was = *flag;
        *flag = false;
        was
    }

    // ── Control ───────────────────────────────────────────────────────────

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

    pub fn toggle_auto_mode(&self) -> bool {
        let mut m = self.inner.auto_mode.lock().unwrap();
        *m = !*m;
        *m
    }

    pub fn request_fresh_scan(&self) {
        *self.inner.fresh_scan_requested.lock().unwrap() = true;
    }

    // ── Scan helpers ──────────────────────────────────────────────────────

    pub fn refresh_cpu_stats(system: &mut System) -> CpuStats {
        system.refresh_cpu_specifics(CpuRefreshKind::new().with_cpu_usage().with_frequency());
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

    pub fn update_child_parent_pairs(&self, system: &System) {
        let mut pairs: HashMap<u32, String> = HashMap::new();
        let mut detections = CaseInsensitiveHashSet::new();

        for (pid, process) in system.processes() {
            let name_lc = process.name().to_lowercase();
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
                            detections.insert(process.name().to_string());
                        }
                    }
                    continue;
                }
            }

            if let Some(ppid) = process.parent() {
                if let Some(parent_proc) = system.process(ppid) {
                    let parent_name = parent_proc.name().to_string();
                    pairs.insert(pid_u32, parent_name.clone());
                    let cfg = self.inner.user_config.lock().unwrap();
                    if cfg.auto_mode_launchers.contains(&parent_name) {
                        detections.insert(process.name().to_string());
                    }
                }
            }
        }

        *self.inner.child_parent_pairs.lock().unwrap() = pairs;
        *self.inner.auto_mode_detections.lock().unwrap() = detections;
    }

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
            .map(|p| p.name().to_string())
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

            let process_name = process.name().to_string();

            let (affinity_mask, priority, is_blacklist) = {
                let cfg = self.inner.user_config.lock().unwrap();

                // Custom processes take precedence over group assignments.
                if let Some(cp) = cfg.custom_process(&process_name) {
                    let mask = cp.affinity.as_ref().map_or(0, |a| a.affinity_mask);
                    (mask, cp.priority.clone(), false)
                } else {
                    let auto_mode = *self.inner.auto_mode.lock().unwrap();
                    let auto_detections = self.inner.auto_mode_detections.lock().unwrap();
                    let is_auto_detected = auto_mode
                        && auto_detections.contains(&process_name)
                        && cfg.process_assignments.get(&process_name).is_none();

                    let group = if is_auto_detected {
                        cfg.default_group()
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

            // ── Snapshot original state before first modification ──────────
            // We only read back an attribute if we are about to change it.
            // This is done inside the apply call below, but we need to know
            // what we *intend* to change first so we can pass the flags.
            let want_affinity = affinity_mask != 0;
            let want_priority = priority.is_some();

            if want_affinity || want_priority {
                // Only snapshot if not already recorded (first touch only)
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

    /// Restore every process we ever modified back to its original state.
    ///
    /// Called on program exit.  Processes that have exited since we modified
    /// them are silently skipped.
    pub fn restore_all(&self) {
        let states = self.inner.original_states.lock().unwrap().clone();
        for (pid, original) in &states {
            Self::restore_process(*pid, original);
        }
    }

    /// Restore all currently-running processes whose name matches one of the
    /// provided names, then drop their snapshots so they won't be re-touched
    /// until the next group assignment changes them.
    ///
    /// Called when a group is deleted and has no default group to fall back to.
    pub fn restore_processes_by_name(&self, names: &std::collections::HashSet<String>) {
        if names.is_empty() {
            return;
        }
        // Build a name→[pids] map from the snapshot table using the running
        // process list.  We only know PIDs, so we have to cross-reference with
        // sysinfo data that was last refreshed by the scan thread.  As a
        // pragmatic fallback we iterate over all snapshotted PIDs and restore
        // any that the OS still reports as belonging to a matching name.
        // Because sysinfo data isn't accessible here, we restore *all*
        // snapshotted PIDs for processes whose name appears in the set — the
        // OS will simply return an error for dead PIDs.
        let pids_to_restore: Vec<(u32, OriginalProcessState)> = {
            let states = self.inner.original_states.lock().unwrap();
            // We can't know which PID maps to which name without a live
            // process snapshot, so we restore everything and let the next scan
            // re-apply only the groups that are still configured.
            states.iter().map(|(&pid, s)| (pid, s.clone())).collect()
        };

        let lower_names: std::collections::HashSet<String> =
            names.iter().map(|n| n.to_lowercase()).collect();

        // Use the running-processes snapshot (names only) to find which PIDs
        // to restore.  We need PID→name info; the only source available here
        // without spawning sysinfo is the managed-processes set paired with
        // what we can obtain from /proc (Linux) or a lightweight OS call.
        // For simplicity we restore all snapshotted PIDs whose process name
        // can be confirmed via the OS, and skip any that don't match.
        for (pid, original) in &pids_to_restore {
            if let Some(name) = Self::process_name_for_pid(*pid) {
                if lower_names.contains(&name.to_lowercase()) {
                    Self::restore_process(*pid, original);
                    self.inner.original_states.lock().unwrap().remove(pid);
                }
            }
        }
    }

    /// Attempt to read the executable name of a process by PID using OS APIs.
    /// Returns `None` if the process no longer exists or cannot be queried.
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
            // Return just the file name part
            Some(
                std::path::Path::new(&full)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or(full),
            )
        }
    }

    #[cfg(target_os = "linux")]
    fn process_name_for_pid(pid: u32) -> Option<String> {
        // /proc/<pid>/comm contains just the executable name (up to 15 chars)
        std::fs::read_to_string(format!("/proc/{}/comm", pid))
            .ok()
            .map(|s| s.trim().to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn process_name_for_pid(_pid: u32) -> Option<String> {
        None
    }

    // ── Platform implementations ──────────────────────────────────────────

    /// Read the current affinity and/or priority of a process before we
    /// change them.  Returns `None` if the process cannot be opened.
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

            // Only return a snapshot if we successfully read at least one value
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

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn read_original_state(
        _pid: u32,
        _read_affinity: bool,
        _read_priority: bool,
    ) -> Option<OriginalProcessState> {
        None
    }

    // ─────────────────────────────────────────────────────────────────────

    /// Restore a single process to its recorded original state.
    #[cfg(target_os = "windows")]
    fn restore_process(pid: u32, original: &OriginalProcessState) {
        unsafe {
            let handle = match OpenProcess(PROCESS_ALL_ACCESS, false, pid) {
                Ok(h) => h,
                Err(_) => return, // process already gone — nothing to do
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
        // priority (nice) restoration omitted — requires root and is reversible
        // by the process itself
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn restore_process(_pid: u32, _original: &OriginalProcessState) {}

    // ─────────────────────────────────────────────────────────────────────

    fn apply_to_process(pid: u32, affinity_mask: u64, priority: Option<ProcessPriority>) -> bool {
        if affinity_mask == 0 && priority.is_none() {
            return true;
        }
        Self::set_process_affinity_and_priority_impl(pid, affinity_mask, priority)
    }

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

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn set_process_affinity_and_priority_impl(
        _pid: u32,
        _affinity_mask: u64,
        _priority: Option<ProcessPriority>,
    ) -> bool {
        false
    }
}

impl Drop for ProcessOverwatch {
    fn drop(&mut self) {}
}

// ─── ScanHandler ─────────────────────────────────────────────────────────────

pub struct ScanHandler {
    process_overwatch: ProcessOverwatch,
    scan_interval: u64,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ScanHandler {
    pub fn new(process_overwatch: ProcessOverwatch, scan_interval: u64) -> Self {
        Self {
            process_overwatch,
            scan_interval,
            stop_flag: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    pub fn start(&mut self) {
        let overwatch = self.process_overwatch.clone();
        let scan_interval = self.scan_interval;
        let stop_flag = Arc::clone(&self.stop_flag);

        self.handle = Some(thread::spawn(move || {
            // Keep CPU and process refresh in separate System instances.
            // On Linux (and some other platforms) refresh_processes_specifics
            // resets the kernel CPU-time counters that refresh_cpu_specifics
            // relies on to compute the usage delta, producing zeros.  Two
            // isolated instances avoid this interference entirely.
            let mut cpu_sys = System::new_with_specifics(
                RefreshKind::new()
                    .with_cpu(CpuRefreshKind::new().with_cpu_usage().with_frequency()),
            );
            let mut proc_sys = System::new_with_specifics(
                RefreshKind::new().with_processes(sysinfo::ProcessRefreshKind::everything()),
            );

            // Prime CPU sampler — must have one prior sample before the loop
            // so the first delta is meaningful.
            cpu_sys.refresh_cpu_specifics(CpuRefreshKind::new().with_cpu_usage().with_frequency());
            thread::sleep(Duration::from_millis(500));

            while !stop_flag.load(Ordering::Relaxed) {
                // Refresh each system independently
                proc_sys.refresh_processes_specifics(sysinfo::ProcessRefreshKind::everything());
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

    /// Stop the background scan thread, then restore all modified processes.
    pub fn stop(&mut self) {
        // Signal the thread to exit and wait for it to finish so that no new
        // modifications can race with our restore pass.
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }

        // Now restore every process we ever touched.
        self.process_overwatch.restore_all();
    }
}
