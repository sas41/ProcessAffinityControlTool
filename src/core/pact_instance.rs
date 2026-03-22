use crate::core::pact_config::PACTConfig;
use crate::core::process_config::{CustomProcess, ProcessGroup};
use crate::core::process_overwatch::{ProcessOverwatch, ScanHandler};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedProcessSource {
    Explicit,
    Default,
    /// Process is managed because its ancestor was configured with capture_sub_processes.
    ChildInherited,
}

#[derive(Debug, Clone)]
pub struct AssignedProcess {
    pub name: String,
    pub group: String,
    pub source: AssignedProcessSource,
}

pub type ConfigUpdatedCallback = Box<dyn FnMut() + Send>;
// Rust note for C# readers: `type` creates an alias; `Box<...>` is a heap-owned pointer,
// `dyn FnMut()` is a trait object (roughly an interface callback), and `+ Send` adds a trait bound.

// High-level facade used by UI/CLI layers.
// Owns runtime scanning and the persisted user config lifecycle.
pub struct PACTInstance {
    // Shared runtime engine: scans processes and applies affinity policy.
    pub pact_process_overwatch: ProcessOverwatch,
    // Background scanner handle, present only while scanning is active.
    // Rust note for C# readers: `Option<T>` is nullable-like (`Some(value)` or `None`).
    pub scan_handler: Option<ScanHandler>,
    // Local listeners notified after config-affecting operations persist.
    config_updated_callbacks: Vec<Mutex<Option<ConfigUpdatedCallback>>>,
}

impl PACTInstance {
    // Rust note for C# readers: `&self` is an immutable borrowed receiver; `&mut self` is mutable.
    pub fn new() -> Self {
        // Startup boundary: load persisted config, start runtime state from it,
        // request a refresh, then persist normalized defaults/migrations.
        let user_config = Self::read_config();
        let pact_process_overwatch = ProcessOverwatch::new(user_config);
        pact_process_overwatch.request_fresh_scan();
        // Clone creates an owned snapshot so file IO does not hold config locks.
        Self::save_config(&pact_process_overwatch.user_config_lock().clone());
        Self {
            pact_process_overwatch,
            scan_handler: None,
            config_updated_callbacks: Vec::new(),
        }
    }

    // Scanner lifecycle.

    pub fn start_scan_handler(&mut self) {
        let interval = self.pact_process_overwatch.scan_interval();
        // Clone is a cheap shared handle for the background worker thread.
        let mut h = ScanHandler::new(self.pact_process_overwatch.clone(), interval);
        h.start();
        self.scan_handler = Some(h);
    }

    pub fn stop_scan_handler(&mut self) {
        // Rust note for C# readers: `if let Some(x) = ...` pattern-matches only the success shape.
        if let Some(mut h) = self.scan_handler.take() {
            h.stop();
        }
    }

    // Runtime toggles.

    pub fn toggle_process_overwatch(&mut self) -> bool {
        let s = self.pact_process_overwatch.toggle_process_overwatch();
        self.fire_config_updated();
        s
    }

    pub fn request_fresh_scan(&mut self) {
        // Runtime refresh only; does not persist config on its own.
        self.pact_process_overwatch.request_fresh_scan();
    }

    pub fn launch_minimized(&self) -> bool {
        self.pact_process_overwatch
            .user_config_lock()
            .launch_minimized
    }

    pub fn set_launch_minimized(&mut self, enabled: bool) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .launch_minimized = enabled;
        self.persist_and_notify();
    }

    // Config persistence boundaries.

    pub fn read_config() -> PACTConfig {
        // Disk -> memory boundary. Fall back to defaults on any read/parse error.
        let path = Self::config_path();
        if path.exists() {
            // Rust note for C# readers: `if let Ok(v) = ...` unwraps `Result` only when it is `Ok`.
            if let Ok(json) = fs::read_to_string(&path) {
                // Rust note for C# readers: `::<PACTConfig>` is a turbofish explicit generic type.
                if let Ok(cfg) = serde_json::from_str::<PACTConfig>(&json) {
                    return cfg;
                }
            }
        }
        PACTConfig::default()
    }

    pub fn save_config(config: &PACTConfig) {
        // Memory -> disk boundary. Best-effort write; callers keep runtime state.
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(&path, json);
        }
    }

    pub fn import_config(&mut self, fullpath: &str) {
        // External file import: replace in-memory config, rescan, then persist.
        if let Ok(json) = fs::read_to_string(fullpath) {
            if let Ok(cfg) = serde_json::from_str::<PACTConfig>(&json) {
                // Rust note for C# readers: leading `*` dereferences before assignment.
                *self.pact_process_overwatch.user_config_lock_mut() = cfg;
                self.pact_process_overwatch.request_fresh_scan();
                self.persist_and_notify();
            }
        }
    }

    pub fn export_config(&self, fullpath: &str) {
        // Export snapshot only; does not mutate runtime state or canonical config path.
        // Clone detaches serialization from the lock-protected config.
        let cfg = self.pact_process_overwatch.user_config_lock().clone();
        if let Ok(json) = serde_json::to_string_pretty(&cfg) {
            let _ = fs::write(fullpath, json);
        }
    }

    pub fn reset_config(&mut self) {
        // Reset in-memory config first, then rescan, persist, and notify listeners.
        *self.pact_process_overwatch.user_config_lock_mut() = PACTConfig::default();
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    fn config_path() -> PathBuf {
        // Rust note for C# readers: `#[cfg(...)]` includes code at compile time by target platform.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // Linux: ~/.config/pact/config.json (honors XDG_CONFIG_HOME).
            // macOS: ~/Library/Application Support/pact/config.json.
            let base = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()));
            base.join("pact").join("config.json")
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Windows and others: <exe directory>/Config/config.json.
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
        // Standard mutation flow finalizer: persist durable state, then broadcast.
        // Clone keeps callback execution outside config lock ownership.
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
        // Rust note for C# readers: `'static` means no borrowed data shorter than program lifetime.
        self.config_updated_callbacks
            .push(Mutex::new(Some(Box::new(cb))));
    }

    // Group CRUD (mutations follow: config change -> rescan -> persist/notify).

    /// Adds a new group.
    ///
    /// Returns `false` if a group with that name already exists.
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

    /// Updates a group matched by `old_name`.
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

    /// Removes a group by name.
    ///
    /// Also clears its process assignments.
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

    /// Deletes a group and hands off or restores assigned processes.
    ///
    /// - If a different default group exists, reassigns all assigned processes.
    /// - Otherwise restores running processes to original state and unassigns them.
    /// - Removes the group and requests a fresh scan.
    pub fn delete_group(&mut self, name: &str) {
        // Snapshot names first to avoid holding mutable config locks too long.
        let procs_in_group: Vec<String> = self.get_processes_in_group(name);

        // Use default group only when it is not the group being deleted.
        // Clone here keeps only the group name after lock scope ends.
        let default_group_name: Option<String> = {
            let cfg = self.pact_process_overwatch.user_config_lock();
            cfg.default_group()
                .filter(|g| g.name.to_lowercase() != name.to_lowercase())
                .map(|g| g.name.clone())
        };

        if let Some(ref default_name) = default_group_name {
            // Reassign all captured processes in-memory.
            let mut cfg = self.pact_process_overwatch.user_config_lock_mut();
            for proc_name in &procs_in_group {
                cfg.assign_process(proc_name, default_name);
            }
        } else {
            // No fallback group: restore running processes, then clear assignments.
            let names_set: std::collections::HashSet<String> = procs_in_group.into_iter().collect();
            self.pact_process_overwatch
                .restore_processes_by_name(&names_set);
            // Remove explicit assignments so the next scan ignores them.
            let mut cfg = self.pact_process_overwatch.user_config_lock_mut();
            for proc_name in &names_set {
                cfg.unassign_process(proc_name);
            }
        }

        // Final in-memory cleanup, then trigger standard rescan/persist flow.
        self.pact_process_overwatch
            .user_config_lock_mut()
            .remove_group(name);
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    /// Returns a snapshot of all groups.
    pub fn get_groups(&self) -> Vec<ProcessGroup> {
        // Clone returns owned data so callers cannot mutate internal state.
        self.pact_process_overwatch
            .user_config_lock()
            .groups
            .clone()
    }

    // Process assignment CRUD.

    /// Assigns a process to a group.
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

    /// Removes a process's explicit group assignment.
    pub fn unassign_process(&mut self, process_name: &str) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .unassign_process(process_name);
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    /// Returns sorted effective assignments for UI display.
    ///
    /// Includes:
    /// - explicit persisted assignments
    /// - ephemeral default-group assignments for unassigned running processes
    pub fn get_assigned_processes(&self) -> Vec<AssignedProcess> {
        let cfg = self.pact_process_overwatch.user_config_lock();
        let mut by_name: std::collections::HashMap<String, AssignedProcess> =
            std::collections::HashMap::new();

        for (proc_name, group_lower) in cfg.process_assignments.iter() {
            let group_name = cfg
                .group_by_name(group_lower)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| group_lower.clone());

            by_name.insert(
                proc_name.to_lowercase(),
                AssignedProcess {
                    name: proc_name.clone(),
                    group: group_name,
                    source: AssignedProcessSource::Explicit,
                },
            );
        }

        let custom_set: std::collections::HashSet<String> = cfg
            .custom_processes
            .iter()
            .map(|cp| cp.name.to_lowercase())
            .collect();

        // Priority 2: Capture Sub-Processes children.
        // Must be inserted BEFORE the default-group loop so that a child process
        // captured into e.g. "Gaming" is not first claimed by the default group,
        // which would cause it to appear in the wrong group card.
        let child_data = self.pact_process_overwatch.capture_sub_processes_data();
        for (child_name, _parent_lc, group_lc) in child_data {
            let key = child_name.to_lowercase();
            if by_name.contains_key(&key) || custom_set.contains(&key) {
                continue; // explicit assignment or custom process rule takes priority
            }
            // group_lc is empty for children whose seed is a custom process (no group).
            // We still register them with an empty group so they appear in assigned_names
            // and are excluded from the Running Processes panel.  They are rendered
            // in the Custom Processes tree view, not in any group card.
            let group_name = if group_lc.is_empty() {
                String::new()
            } else {
                cfg.group_by_name(&group_lc)
                    .map(|g| g.name.clone())
                    .unwrap_or(group_lc)
            };
            by_name.insert(
                key,
                AssignedProcess {
                    name: child_name,
                    group: group_name,
                    source: AssignedProcessSource::ChildInherited,
                },
            );
        }

        // Priority 3 (lowest): Default group for all other running processes.
        let running = self.pact_process_overwatch.running_processes();
        let default_group = cfg.default_group().map(|g| g.name.clone());

        for process_name in running {
            let key = process_name.to_lowercase();

            if by_name.contains_key(&key) || custom_set.contains(&key) {
                continue;
            }

            if let Some(group_name) = &default_group {
                by_name.insert(
                    key,
                    AssignedProcess {
                        name: process_name,
                        group: group_name.clone(),
                        source: AssignedProcessSource::Default,
                    },
                );
            }
        }

        let mut v: Vec<AssignedProcess> = by_name.into_values().collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        v
    }

    /// Returns sorted process names assigned to `group_name`.
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

    // Custom process CRUD.

    pub fn get_custom_processes(&self) -> Vec<CustomProcess> {
        // Clone returns an immutable snapshot to callers.
        self.pact_process_overwatch
            .user_config_lock()
            .custom_processes
            .clone()
    }

    pub fn add_custom_process(&mut self, cp: CustomProcess) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .add_custom_process(cp);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    pub fn update_custom_process(&mut self, old_name: &str, cp: CustomProcess) -> bool {
        let ok = self
            .pact_process_overwatch
            .user_config_lock_mut()
            .update_custom_process(old_name, cp);
        if ok {
            self.pact_process_overwatch.request_fresh_scan();
            self.persist_and_notify();
        }
        ok
    }

    pub fn remove_custom_process(&mut self, name: &str) {
        self.pact_process_overwatch
            .user_config_lock_mut()
            .remove_custom_process(name);
        self.pact_process_overwatch.request_fresh_scan();
        self.persist_and_notify();
    }

    // Running process info (read-only snapshots).

    pub fn get_all_running_processes(&self) -> Vec<String> {
        self.pact_process_overwatch.running_processes()
    }

    pub fn get_protected_processes(&self) -> Vec<String> {
        self.pact_process_overwatch.protected_process_names()
    }

    /// Returns running processes without explicit group assignments.
    pub fn get_unassigned_running_processes(&self) -> Vec<String> {
        let running = self.get_all_running_processes();
        let cfg = self.pact_process_overwatch.user_config_lock();
        running
            .into_iter()
            .filter(|n| cfg.process_assignments.get(n).is_none())
            .collect()
    }
}

impl Drop for PACTInstance {
    fn drop(&mut self) {
        self.stop_scan_handler();
    }
}
