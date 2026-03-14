use crate::core::pact_config::PACTConfig;
use crate::core::process_config::ProcessGroup;
use crate::core::process_overwatch::{ProcessOverwatch, ScanHandler};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub type ConfigUpdatedCallback = Box<dyn FnMut() + Send>;

pub struct PACTInstance {
    pub pact_process_overwatch: ProcessOverwatch,
    pub scan_handler: Option<ScanHandler>,
    config_updated_callbacks: Vec<Mutex<Option<ConfigUpdatedCallback>>>,
}

impl PACTInstance {
    pub fn new() -> Self {
        let user_config = Self::read_config();
        let pact_process_overwatch = ProcessOverwatch::new(user_config);
        pact_process_overwatch.request_fresh_scan();
        Self::save_config(&pact_process_overwatch.user_config_lock().clone());
        Self {
            pact_process_overwatch,
            scan_handler: None,
            config_updated_callbacks: Vec::new(),
        }
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────

    pub fn start_scan_handler(&mut self) {
        let interval = self.pact_process_overwatch.scan_interval();
        let mut h = ScanHandler::new(self.pact_process_overwatch.clone(), interval);
        h.start();
        self.scan_handler = Some(h);
    }

    pub fn stop_scan_handler(&mut self) {
        if let Some(mut h) = self.scan_handler.take() {
            h.stop();
        }
    }

    // ── Overwatch control ─────────────────────────────────────────────────

    pub fn toggle_process_overwatch(&mut self) -> bool {
        let s = self.pact_process_overwatch.toggle_process_overwatch();
        self.fire_config_updated();
        s
    }

    pub fn toggle_auto_mode(&mut self) -> bool {
        let s = self.pact_process_overwatch.toggle_auto_mode();
        self.fire_config_updated();
        s
    }

    pub fn request_fresh_scan(&mut self) {
        self.pact_process_overwatch.request_fresh_scan();
    }

    // ── Config persistence ────────────────────────────────────────────────

    pub fn read_config() -> PACTConfig {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(json) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<PACTConfig>(&json) {
                    return cfg;
                }
            }
        }
        PACTConfig::default()
    }

    pub fn save_config(config: &PACTConfig) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(&path, json);
        }
    }

    pub fn import_config(&mut self, fullpath: &str) {
        if let Ok(json) = fs::read_to_string(fullpath) {
            if let Ok(cfg) = serde_json::from_str::<PACTConfig>(&json) {
                *self.pact_process_overwatch.user_config_lock_mut() = cfg;
                self.pact_process_overwatch.request_fresh_scan();
                self.persist_and_notify();
            }
        }
    }

    pub fn export_config(&self, fullpath: &str) {
        let cfg = self.pact_process_overwatch.user_config_lock().clone();
        if let Ok(json) = serde_json::to_string_pretty(&cfg) {
            let _ = fs::write(fullpath, json);
        }
    }

    pub fn reset_config(&mut self) {
        *self.pact_process_overwatch.user_config_lock_mut() = PACTConfig::default();
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    fn config_path() -> PathBuf {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // Linux: ~/.config/pact/config.json       (XDG_CONFIG_HOME honoured)
            // macOS: ~/Library/Application Support/pact/config.json
            let base = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()));
            base.join("pact").join("config.json")
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // <exe directory>/Config/config.json  (Windows, etc.)
            let mut p = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| std::env::current_dir().unwrap());
            p.push("Config");
            p.push("config.json");
            p
        }
    }

    fn persist_and_notify(&self) {
        Self::save_config(&self.pact_process_overwatch.user_config_lock().clone());
        self.fire_config_updated();
    }

    fn fire_config_updated(&self) {
        for m in &self.config_updated_callbacks {
            if let Ok(mut g) = m.lock() {
                if let Some(ref mut cb) = *g {
                    cb();
                }
            }
        }
    }

    pub fn add_config_updated_callback<F: FnMut() + Send + 'static>(&mut self, cb: F) {
        self.config_updated_callbacks
            .push(Mutex::new(Some(Box::new(cb))));
    }

    // ── Group CRUD ────────────────────────────────────────────────────────

    /// Add a new group.  Returns false if a group with that name already exists.
    pub fn add_group(&mut self, group: ProcessGroup) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .add_group(group);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    /// Update an existing group (matched by `old_name`).
    pub fn update_group(&mut self, old_name: &str, new_group: ProcessGroup) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .update_group(old_name, new_group);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    /// Remove a group by name; all its process assignments are also removed.
    pub fn remove_group(&mut self, name: &str) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .remove_group(name);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    /// Delete a group with the correct process-handoff behaviour:
    ///
    /// 1. Collect all process names explicitly assigned to this group.
    /// 2. If a default group exists, reassign them all to it (they will be
    ///    picked up by the next scan with the default group's settings).
    /// 3. If there is no default group, restore the original affinity/priority
    ///    of every currently-running process that belongs to the group, then
    ///    remove their explicit assignments so they are left unmanaged.
    /// 4. Remove the group itself (this also clears any remaining assignments).
    pub fn delete_group(&mut self, name: &str) {
        // 1. Snapshot the group's process list before we mutate anything.
        let procs_in_group: Vec<String> = self.get_processes_in_group(name);

        // 2. Check for a default group (must not be the group being deleted).
        let default_group_name: Option<String> = {
            let cfg = self.pact_process_overwatch.user_config_lock();
            cfg.default_group()
                .filter(|g| g.name.to_lowercase() != name.to_lowercase())
                .map(|g| g.name.clone())
        };

        if let Some(ref default_name) = default_group_name {
            // Reassign all processes to the default group.
            let mut cfg = self.pact_process_overwatch.user_config_lock_mut();
            for proc_name in &procs_in_group {
                cfg.assign_process(proc_name, default_name);
            }
        } else {
            // No default group — restore original state for running processes.
            let names_set: std::collections::HashSet<String> = procs_in_group.into_iter().collect();
            self.pact_process_overwatch
                .restore_processes_by_name(&names_set);
            // Remove their explicit assignments so the next scan ignores them.
            let mut cfg = self.pact_process_overwatch.user_config_lock_mut();
            for proc_name in &names_set {
                cfg.unassign_process(proc_name);
            }
        }

        // 4. Remove the group (also clears any remaining orphaned assignments).
        self.pact_process_overwatch
            .user_config_lock_mut()
            .remove_group(name);
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    /// Return a snapshot of all groups.
    pub fn get_groups(&self) -> Vec<ProcessGroup> {
        self.pact_process_overwatch
            .user_config_lock()
            .groups
            .clone()
    }

    // ── Process assignment CRUD ───────────────────────────────────────────

    /// Assign a process to a named group.
    pub fn assign_process(&mut self, process_name: &str, group_name: &str) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .assign_process(process_name, group_name);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    /// Remove a process's explicit group assignment.
    pub fn unassign_process(&mut self, process_name: &str) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .unassign_process(process_name);
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    /// All process names that have an explicit assignment, sorted, with their group name.
    pub fn get_assigned_processes(&self) -> Vec<(String, String)> {
        let cfg = self.pact_process_overwatch.user_config_lock();
        let mut v: Vec<(String, String)> = cfg
            .process_assignments
            .iter()
            .map(|(p, g)| (p.clone(), g.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Processes assigned to a specific group, sorted.
    pub fn get_processes_in_group(&self, group_name: &str) -> Vec<String> {
        let lower = group_name.to_lowercase();
        let cfg = self.pact_process_overwatch.user_config_lock();
        let mut v: Vec<String> = cfg
            .process_assignments
            .iter()
            .filter(|(_, g)| *g == &lower)
            .map(|(p, _)| p.clone())
            .collect();
        v.sort();
        v
    }

    // ── Running / live process info ───────────────────────────────────────

    pub fn get_all_running_processes(&self) -> Vec<String> {
        self.pact_process_overwatch.running_processes()
    }

    pub fn get_protected_processes(&self) -> Vec<String> {
        self.pact_process_overwatch.protected_process_names()
    }

    /// Running processes NOT explicitly assigned to any group.
    pub fn get_unassigned_running_processes(&self) -> Vec<String> {
        let running = self.get_all_running_processes();
        let cfg = self.pact_process_overwatch.user_config_lock();
        running
            .into_iter()
            .filter(|n| cfg.process_assignments.get(n).is_none())
            .collect()
    }

    // ── Auto mode ─────────────────────────────────────────────────────────

    pub fn get_auto_mode_launchers(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .pact_process_overwatch
            .user_config_lock()
            .auto_mode_launchers
            .iter()
            .cloned()
            .collect();
        v.sort();
        v
    }

    pub fn get_auto_mode_detections(&self) -> Vec<String> {
        self.pact_process_overwatch.auto_mode_detections_list()
    }

    pub fn add_to_auto_mode_launchers(&mut self, name: &str) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .add_to_auto_mode_launchers(name.to_string());
        self.persist_and_notify();
    }

    pub fn remove_from_auto_mode_launchers(&mut self, name: &str) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .remove_from_auto_mode_launchers(name);
        self.persist_and_notify();
    }
}

impl Drop for PACTInstance {
    fn drop(&mut self) {
        self.stop_scan_handler();
    }
}
