//! Platform-abstracted CPU topology discovery.
//!
//! Each OS provides a [`PlatformTopologyProvider`] implementation that returns
//! per-thread [`ThreadInfo`] records. The topology module consumes these to
//! build the hierarchical group model without knowing OS-specific details.

// Conditionally compiled platform backends.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Classification of a logical thread's core type.
///
/// Intel hybrid: Performance / Efficiency.
/// AMD X3D:      HighCache (V-Cache CCD) / HighFrequency (standard CCD).
/// Everything else or undetermined: Generic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadClassification {
    /// Performance core (Intel big core).
    Performance,
    /// Efficiency core (Intel small core).
    Efficiency,
    /// AMD CCD with extra cache (V-Cache / X3D).
    HighCache,
    /// AMD CCD with higher boost clocks (standard CCD).
    HighFrequency,
    /// Monolithic / cannot determine / fallback.
    Generic,
}

impl ThreadClassification {
    /// Short label for UI display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Performance => "P",
            Self::Efficiency => "E",
            Self::HighCache => "HC",
            Self::HighFrequency => "HF",
            Self::Generic => "?",
        }
    }
}

/// One level of cache associated with a thread.
///
/// `group_id` identifies which set of threads share this particular cache
/// instance. Threads with the same `(level, group_id)` share the same
/// physical cache.
#[derive(Debug, Clone)]
pub struct CacheLevelInfo {
    /// Cache level: 1, 2, 3, …
    pub level: u8,
    /// Size of this cache instance in bytes.
    pub size_bytes: u64,
    /// Opaque group identifier. Threads sharing the same physical cache
    /// at this level will have the same `group_id`.
    pub group_id: isize,
}

/// Complete per-thread hardware information returned by the platform provider.
///
/// Every logical thread (hardware thread / PU) gets one of these.
/// The topology module iterates these to build the group hierarchy.
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    /// OS logical processor index (0-based).
    pub thread_index: usize,
    /// Physical core index this thread belongs to.
    pub core_index: usize,
    /// Base clock frequency in MHz (0 if unavailable).
    pub base_freq_mhz: u64,
    /// Maximum / boost clock frequency in MHz (0 if unavailable).
    pub max_freq_mhz: u64,
    /// Cache levels associated with this thread, from L1 upward.
    /// Index 0 = L1, index 1 = L2, etc. May be shorter if data is unavailable.
    pub caches: Vec<CacheLevelInfo>,
    /// Thread classification (P/E for Intel, HC/HF for AMD X3D, Generic otherwise).
    pub classification: ThreadClassification,
    /// Core Complex (CCX) index. -1 if not applicable (non-AMD or undiscoverable).
    pub ccx_index: isize,
    /// Core Complex Die (CCD) index. -1 if not applicable.
    pub ccd_index: isize,
    /// NUMA node index. -1 if not applicable or single-node.
    pub numa_index: isize,
    /// Compute group index.
    ///
    /// For Intel hybrid: groups P-cores and E-cores separately (typically 0 and 1).
    /// For AMD multi-CCD: -1 (use CCD grouping instead).
    /// For monolithic / fallback: 0 (single group containing all cores).
    pub compute_group: isize,
}

/// Trait for OS-specific topology discovery.
///
/// Each platform implements this to return a flat list of [`ThreadInfo`]
/// records. The topology module handles all grouping/hierarchy logic.
pub trait PlatformTopologyProvider {
    /// Discover all logical threads and their hardware properties.
    fn discover_threads(&self) -> Vec<ThreadInfo>;
}

/// Instantiate the correct platform provider and discover threads.
///
/// This is the single entry point used by the topology module.
pub fn discover_platform_threads() -> Vec<ThreadInfo> {
    let provider = create_provider();
    provider.discover_threads()
}

fn create_provider() -> Box<dyn PlatformTopologyProvider> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxProvider)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsProvider)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsProvider)
    }
    // Fallback for unsupported platforms: return empty.
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        struct EmptyProvider;
        impl PlatformTopologyProvider for EmptyProvider {
            fn discover_threads(&self) -> Vec<ThreadInfo> {
                Vec::new()
            }
        }
        Box::new(EmptyProvider)
    }
}
