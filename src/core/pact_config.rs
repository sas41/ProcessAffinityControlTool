// C# note: `::` is Rust path/namespace access (similar role to `.` in type/module names).
use crate::core::process_config::{CustomProcess, ProcessGroup};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Persistence helper used by `PACTConfig` for case-insensitive map lookups.
///
/// Stored keys are always lowercase so serde writes a normalized shape to disk.
// C# note: `#[derive(...)]` asks the compiler to auto-implement listed traits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseInsensitiveHashMap<T> {
    // C# note: `<T>` declares a generic type parameter.
    map: HashMap<String, T>,
}

// C# note: `impl<T> Type<T> { ... }` is where methods for a generic type are defined.
impl<T> CaseInsensitiveHashMap<T> {
    /// Creates an empty map.
    // C# note: `->` introduces a return type; `Self` means this concrete struct type.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Inserts a value under `key` after lowercasing it.
    // C# note: `&mut self` is a mutable borrowed receiver; `Option<T>` is `Some` or `None`.
    pub fn insert(&mut self, key: String, value: T) -> Option<T> {
        self.map.insert(key.to_lowercase(), value)
    }

    /// Returns the value for `key`, matched case-insensitively.
    // C# note: `Q: ?Sized + AsRef<str>` is a trait-bound generic constraint.
    pub fn get<Q: ?Sized + AsRef<str>>(&self, key: &Q) -> Option<&T> {
        self.map.get(key.as_ref().to_lowercase().as_str())
    }

    /// Returns a mutable value for `key`, matched case-insensitively.
    pub fn get_mut<Q: ?Sized + AsRef<str>>(&mut self, key: &Q) -> Option<&mut T> {
        self.map.get_mut(key.as_ref().to_lowercase().as_str())
    }

    /// Removes `key` and returns the previous value if present.
    pub fn remove<Q: ?Sized + AsRef<str>>(&mut self, key: &Q) -> Option<T> {
        self.map.remove(key.as_ref().to_lowercase().as_str())
    }

    /// Returns whether `key` exists, matched case-insensitively.
    pub fn contains_key<Q: ?Sized + AsRef<str>>(&self, key: &Q) -> bool {
        self.map.contains_key(key.as_ref().to_lowercase().as_str())
    }

    /// Returns an iterator over stored lowercase keys and values.
    // C# note: `'_` is an inferred lifetime tied to this borrow (`&self`).
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, T> {
        self.map.iter()
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<T> Default for CaseInsensitiveHashMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistence helper used by `PACTConfig` for case-insensitive set membership.
///
/// Stored values are always lowercase so serialized config remains normalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseInsensitiveHashSet {
    set: HashSet<String>,
}

impl CaseInsensitiveHashSet {
    /// Creates an empty set.
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    /// Inserts `value` after lowercasing it.
    pub fn insert(&mut self, value: String) -> bool {
        self.set.insert(value.to_lowercase())
    }

    /// Returns whether `value` exists, matched case-insensitively.
    pub fn contains<Q: ?Sized + AsRef<str>>(&self, value: &Q) -> bool {
        self.set.contains(value.as_ref().to_lowercase().as_str())
    }

    /// Removes `value` and returns whether it was present.
    pub fn remove<Q: ?Sized + AsRef<str>>(&mut self, value: &Q) -> bool {
        self.set.remove(value.as_ref().to_lowercase().as_str())
    }

    /// Returns an iterator over stored lowercase values.
    pub fn iter(&self) -> std::collections::hash_set::Iter<'_, String> {
        self.set.iter()
    }

    /// Returns the number of values.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

impl Default for CaseInsensitiveHashSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level persisted Process Affinity Control Tool configuration.
///
/// `serde(rename = ...)` keeps compatibility with the existing on-disk key names,
/// while helper types below keep name matching case-insensitive at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PACTConfig {
    /// Configured groups in display order.
    // C# note: attributes like `#[serde(...)]` attach metadata used by libraries/tools.
    #[serde(rename = "Groups")]
    pub groups: Vec<ProcessGroup>,

    /// Process name to group name mapping, case-insensitive.
    #[serde(rename = "ProcessAssignments")]
    pub process_assignments: CaseInsensitiveHashMap<String>,

    /// Per-process affinity and priority overrides.
    ///
    /// `default` keeps older config files loadable when this field is missing.
    #[serde(rename = "CustomProcesses", default)]
    pub custom_processes: Vec<CustomProcess>,

    /// Launcher names that trigger auto-mode assignment.
    #[serde(rename = "AutoModeLaunchers")]
    pub auto_mode_launchers: CaseInsensitiveHashSet,

    /// Process scan interval in milliseconds.
    #[serde(rename = "ScanInterval")]
    pub scan_interval: u64,

    /// Start UI hidden to tray on launch.
    #[serde(rename = "LaunchMinimized", default)]
    pub launch_minimized: bool,
}

impl Default for PACTConfig {
    fn default() -> Self {
        let mut auto_mode_launchers = CaseInsensitiveHashSet::new();

        for name in [
            "Battle.net",
            "Battle.net Launcher",
            "EALink",
            "EpicGamesLauncher",
            "GalaxyClient",
            "GalaxyClientService",
            "Origin",
            "OriginClientService",
            "steam",
            "steamservice",
            "UbisoftGameLauncher",
            "UbisoftGameLauncher64",
            "UnrealEngineLauncher",
            "Uplay",
            "UplayService",
            "x64launcher",
            "x86launcher",
        ] {
            auto_mode_launchers.insert(name.to_string());
        }

        Self {
            groups: Vec::new(),
            process_assignments: CaseInsensitiveHashMap::new(),
            custom_processes: Vec::new(),
            auto_mode_launchers,
            // 3s is the baseline poll interval used when no config file exists yet.
            scan_interval: 3000,
            launch_minimized: false,
        }
    }
}

impl PACTConfig {
    /// Returns the configured fallback group.
    ///
    /// This is used when a process has no explicit assignment.
    pub fn default_group(&self) -> Option<&ProcessGroup> {
        // C# note: `|g| ...` is a closure (lambda) parameter list.
        self.groups.iter().find(|g| g.is_default)
    }

    /// Returns the configured auto-mode target group.
    pub fn auto_mode_group(&self) -> Option<&ProcessGroup> {
        self.groups.iter().find(|g| g.is_auto_mode_group)
    }

    /// Finds a group by name, case-insensitively.
    pub fn group_by_name(&self, name: &str) -> Option<&ProcessGroup> {
        let lower = name.to_lowercase();
        self.groups.iter().find(|g| g.name.to_lowercase() == lower)
    }

    /// Finds a mutable group by name, case-insensitively.
    pub fn group_by_name_mut(&mut self, name: &str) -> Option<&mut ProcessGroup> {
        let lower = name.to_lowercase();
        self.groups
            .iter_mut()
            .find(|g| g.name.to_lowercase() == lower)
    }

    /// Resolves a process to its effective group.
    ///
    /// Resolution order is explicit assignment first, then default group.
    pub fn group_for_process(&self, process_name: &str) -> Option<&ProcessGroup> {
        // C# note: `if let` matches one pattern and enters only when it matches.
        if let Some(group_name) = self.process_assignments.get(process_name) {
            self.group_by_name(group_name)
        } else {
            self.default_group()
        }
    }

    /// Adds a group if its name is unique.
    /// If the new group is default, clears existing defaults.
    pub fn add_group(&mut self, group: ProcessGroup) -> bool {
        let lower = group.name.to_lowercase();

        if self.groups.iter().any(|g| g.name.to_lowercase() == lower) {
            return false;
        }

        if group.is_default {
            for g in &mut self.groups {
                g.is_default = false;
            }
        }

        if group.is_auto_mode_group {
            for g in &mut self.groups {
                g.is_auto_mode_group = false;
            }
        }

        self.groups.push(group);
        true
    }

    /// Replaces a group matched by `old_name`.
    ///
    /// When renamed, existing assignments are rewired to the new lowercase name
    /// so persisted config and in-memory lookups stay aligned.
    pub fn update_group(&mut self, old_name: &str, new_group: ProcessGroup) -> bool {
        let old_lower = old_name.to_lowercase();

        let pos = self
            .groups
            .iter()
            .position(|g| g.name.to_lowercase() == old_lower);

        // C# note: `let ... else` unwraps a pattern or performs the `else` early-exit path.
        let Some(pos) = pos else { return false };

        if new_group.is_default {
            for (i, g) in self.groups.iter_mut().enumerate() {
                if i != pos {
                    g.is_default = false;
                }
            }
        }

        if new_group.is_auto_mode_group {
            for (i, g) in self.groups.iter_mut().enumerate() {
                if i != pos {
                    g.is_auto_mode_group = false;
                }
            }
        }

        let new_lower = new_group.name.to_lowercase();
        if old_lower != new_lower {
            let keys: Vec<String> = self
                .process_assignments
                .iter()
                .filter(|(_, v)| *v == &old_lower)
                .map(|(k, _)| k.clone())
                .collect();

            for key in keys {
                self.process_assignments.insert(key, new_lower.clone());
            }
        }

        self.groups[pos] = new_group;
        true
    }

    /// Removes a group and any assignments that point to it.
    ///
    /// Assignment cleanup prevents stale names from being written back to disk.
    pub fn remove_group(&mut self, name: &str) -> bool {
        let lower = name.to_lowercase();
        let len_before = self.groups.len();

        self.groups.retain(|g| g.name.to_lowercase() != lower);

        if self.groups.len() == len_before {
            return false;
        }

        let orphans: Vec<String> = self
            .process_assignments
            .iter()
            .filter(|(_, v)| *v == &lower)
            .map(|(k, _)| k.clone())
            .collect();

        for k in orphans {
            self.process_assignments.remove(&k);
        }
        true
    }

    /// Marks one group as default and clears the flag on others.
    pub fn set_default_group(&mut self, name: &str) -> bool {
        let lower = name.to_lowercase();

        let exists = self.groups.iter().any(|g| g.name.to_lowercase() == lower);
        if !exists {
            return false;
        }

        for g in &mut self.groups {
            g.is_default = g.name.to_lowercase() == lower;
        }
        true
    }

    /// Clears the default flag from all groups.
    pub fn clear_default_group(&mut self) {
        for g in &mut self.groups {
            g.is_default = false;
        }
    }

    /// Assigns a process to an existing group.
    ///
    /// Stored assignment values use lowercase group names for stable persistence.
    /// Returns `false` for an empty process name or unknown group.
    pub fn assign_process(&mut self, process_name: &str, group_name: &str) -> bool {
        if process_name.is_empty() {
            return false;
        }

        let lower_group = group_name.to_lowercase();
        if !self
            .groups
            .iter()
            .any(|g| g.name.to_lowercase() == lower_group)
        {
            return false;
        }

        self.process_assignments
            .insert(process_name.to_string(), lower_group);
        true
    }

    /// Removes a process's explicit group assignment.
    pub fn unassign_process(&mut self, process_name: &str) {
        self.process_assignments.remove(process_name);
    }

    /// Returns only the explicit assignment for a process, if any.
    ///
    /// Unlike `group_for_process`, this helper does not apply default fallback.
    pub fn explicit_group_of(&self, process_name: &str) -> Option<&str> {
        self.process_assignments
            .get(process_name)
            .map(String::as_str)
    }

    /// Adds a non-empty launcher name to auto-mode.
    pub fn add_to_auto_mode_launchers(&mut self, name: String) {
        if !name.is_empty() {
            self.auto_mode_launchers.insert(name);
        }
    }

    /// Removes a launcher from auto-mode.
    /// Returns `false` for an empty name or missing entry.
    pub fn remove_from_auto_mode_launchers(&mut self, name: &str) -> bool {
        if !name.is_empty() && self.auto_mode_launchers.contains(name) {
            self.auto_mode_launchers.remove(name);
            true
        } else {
            false
        }
    }

    /// Finds a custom process by name, case-insensitively.
    pub fn custom_process(&self, name: &str) -> Option<&CustomProcess> {
        let lower = name.to_lowercase();
        self.custom_processes
            .iter()
            .find(|cp| cp.name.to_lowercase() == lower)
    }

    /// Adds a custom process if its name is not already present.
    pub fn add_custom_process(&mut self, cp: CustomProcess) -> bool {
        if self.custom_process(&cp.name).is_some() {
            return false;
        }

        self.custom_processes.push(cp);
        true
    }

    /// Replaces a custom process matched by `old_name`.
    pub fn update_custom_process(&mut self, old_name: &str, cp: CustomProcess) -> bool {
        let lower = old_name.to_lowercase();

        if let Some(pos) = self
            .custom_processes
            .iter()
            .position(|p| p.name.to_lowercase() == lower)
        {
            self.custom_processes[pos] = cp;
            true
        } else {
            false
        }
    }

    /// Removes a custom process by name.
    pub fn remove_custom_process(&mut self, name: &str) -> bool {
        let lower = name.to_lowercase();
        let before = self.custom_processes.len();

        self.custom_processes
            .retain(|cp| cp.name.to_lowercase() != lower);

        self.custom_processes.len() < before
    }
}
