use crate::core::process_config::ProcessGroup;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

// ─── Case-insensitive collections ────────────────────────────────────────────

/// HashMap whose keys are normalised to lowercase on every operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseInsensitiveHashMap<T> {
    map: HashMap<String, T>,
}

impl<T> CaseInsensitiveHashMap<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: T) -> Option<T> {
        self.map.insert(key.to_lowercase(), value)
    }

    pub fn get<Q: ?Sized + AsRef<str>>(&self, key: &Q) -> Option<&T> {
        self.map.get(key.as_ref().to_lowercase().as_str())
    }

    pub fn get_mut<Q: ?Sized + AsRef<str>>(&mut self, key: &Q) -> Option<&mut T> {
        self.map.get_mut(key.as_ref().to_lowercase().as_str())
    }

    pub fn remove<Q: ?Sized + AsRef<str>>(&mut self, key: &Q) -> Option<T> {
        self.map.remove(key.as_ref().to_lowercase().as_str())
    }

    pub fn contains_key<Q: ?Sized + AsRef<str>>(&self, key: &Q) -> bool {
        self.map.contains_key(key.as_ref().to_lowercase().as_str())
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, T> {
        self.map.iter()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<T> Default for CaseInsensitiveHashMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// HashSet whose values are normalised to lowercase on every operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseInsensitiveHashSet {
    set: HashSet<String>,
}

impl CaseInsensitiveHashSet {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    pub fn insert(&mut self, value: String) -> bool {
        self.set.insert(value.to_lowercase())
    }

    pub fn contains<Q: ?Sized + AsRef<str>>(&self, value: &Q) -> bool {
        self.set.contains(value.as_ref().to_lowercase().as_str())
    }

    pub fn remove<Q: ?Sized + AsRef<str>>(&mut self, value: &Q) -> bool {
        self.set.remove(value.as_ref().to_lowercase().as_str())
    }

    pub fn iter(&self) -> std::collections::hash_set::Iter<'_, String> {
        self.set.iter()
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

impl Default for CaseInsensitiveHashSet {
    fn default() -> Self {
        Self::new()
    }
}

// ─── PACTConfig ───────────────────────────────────────────────────────────────

/// Top-level configuration.
///
/// ### Group model
///
/// `groups` is an ordered list of `ProcessGroup` objects.  Each group defines
/// what should happen to processes assigned to it (optional affinity, optional
/// priority, blacklist flag, default flag).
///
/// `process_assignments` maps a process exe-name (case-insensitive, lowercase)
/// to a group **name** (lowercase).  A process not found in this map is treated
/// by the default group if one exists, otherwise left untouched.
///
/// ### Auto Mode
///
/// `auto_mode_launchers` is a set of launcher exe-names.  When Auto Mode is
/// enabled, child processes of these launchers are dynamically assigned to the
/// default group's affinity/priority settings (same behaviour as before).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PACTConfig {
    /// All configured groups, in display order.
    #[serde(rename = "Groups")]
    pub groups: Vec<ProcessGroup>,

    /// process_name (lowercase) → group_name (lowercase)
    #[serde(rename = "ProcessAssignments")]
    pub process_assignments: CaseInsensitiveHashMap<String>,

    /// Launchers whose child processes get auto-promoted via Auto Mode.
    #[serde(rename = "AutoModeLaunchers")]
    pub auto_mode_launchers: CaseInsensitiveHashSet,

    #[serde(rename = "ScanInterval")]
    pub scan_interval: u64,
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
            groups: Vec::new(), // fresh start — no default groups
            process_assignments: CaseInsensitiveHashMap::new(),
            auto_mode_launchers,
            scan_interval: 3000,
        }
    }
}

impl PACTConfig {
    // ── Group accessors ───────────────────────────────────────────────────

    /// Returns the group whose `is_default` flag is set, if any.
    pub fn default_group(&self) -> Option<&ProcessGroup> {
        self.groups.iter().find(|g| g.is_default)
    }

    /// Find a group by name (case-insensitive).
    pub fn group_by_name(&self, name: &str) -> Option<&ProcessGroup> {
        let lower = name.to_lowercase();
        self.groups.iter().find(|g| g.name.to_lowercase() == lower)
    }

    pub fn group_by_name_mut(&mut self, name: &str) -> Option<&mut ProcessGroup> {
        let lower = name.to_lowercase();
        self.groups
            .iter_mut()
            .find(|g| g.name.to_lowercase() == lower)
    }

    /// Find the group a process is assigned to, falling back to the default group.
    pub fn group_for_process(&self, process_name: &str) -> Option<&ProcessGroup> {
        if let Some(group_name) = self.process_assignments.get(process_name) {
            let gn = group_name.clone();
            self.group_by_name(&gn)
        } else {
            self.default_group()
        }
    }

    // ── Group mutations ───────────────────────────────────────────────────

    /// Add a new group.  Returns false if a group with that name already exists.
    pub fn add_group(&mut self, group: ProcessGroup) -> bool {
        let lower = group.name.to_lowercase();
        if self.groups.iter().any(|g| g.name.to_lowercase() == lower) {
            return false;
        }
        // Enforce single-default invariant
        if group.is_default {
            for g in &mut self.groups {
                g.is_default = false;
            }
        }
        self.groups.push(group);
        true
    }

    /// Replace an existing group in-place (matched by current name, case-insensitive).
    /// If `new_group.is_default` is true, clears the flag from all other groups first.
    pub fn update_group(&mut self, old_name: &str, new_group: ProcessGroup) -> bool {
        let old_lower = old_name.to_lowercase();
        let pos = self
            .groups
            .iter()
            .position(|g| g.name.to_lowercase() == old_lower);
        let Some(pos) = pos else { return false };

        // If the new group becomes default, clear others
        if new_group.is_default {
            for (i, g) in self.groups.iter_mut().enumerate() {
                if i != pos {
                    g.is_default = false;
                }
            }
        }

        // If the name changed, re-key all process assignments
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

    /// Remove a group by name.  All process assignments pointing to it are removed.
    pub fn remove_group(&mut self, name: &str) -> bool {
        let lower = name.to_lowercase();
        let len_before = self.groups.len();
        self.groups.retain(|g| g.name.to_lowercase() != lower);
        if self.groups.len() == len_before {
            return false;
        }
        // Remove orphaned assignments
        let orphans: Vec<String> = self
            .process_assignments
            .iter()
            .filter(|(_, v)| *v == &lower)
            .map(|(k, _)| k.clone())
            .collect();
        for k in orphans {
            self.process_assignments.remove(&k as &str);
        }
        true
    }

    /// Set exactly one group as the default (clears flag on all others).
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

    /// Clear the default flag from all groups (no default group).
    pub fn clear_default_group(&mut self) {
        for g in &mut self.groups {
            g.is_default = false;
        }
    }

    // ── Process assignment mutations ──────────────────────────────────────

    /// Assign a process to a group (by group name).  Removes any prior assignment.
    /// Returns false if the target group does not exist.
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

    /// Remove a process's assignment (it will fall through to the default group).
    pub fn unassign_process(&mut self, process_name: &str) {
        self.process_assignments.remove(process_name);
    }

    /// Returns the group name a process is explicitly assigned to, if any.
    pub fn explicit_group_of(&self, process_name: &str) -> Option<&str> {
        self.process_assignments
            .get(process_name)
            .map(|s| s.as_str())
    }

    // ── Auto mode launchers ───────────────────────────────────────────────

    pub fn add_to_auto_mode_launchers(&mut self, name: String) {
        if !name.is_empty() {
            self.auto_mode_launchers.insert(name);
        }
    }

    pub fn remove_from_auto_mode_launchers(&mut self, name: &str) -> bool {
        if !name.is_empty() && self.auto_mode_launchers.contains(name) {
            self.auto_mode_launchers.remove(name);
            true
        } else {
            false
        }
    }
}
