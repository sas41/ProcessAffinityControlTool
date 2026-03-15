//! Core domain modules for process-affinity control orchestration.
//! Includes configuration, process lifecycle, and topology coordination.
//! `//!` is a module-level doc comment (similar to XML docs on a C# namespace/file scope).

// `pub mod name;` declares a child module (loaded from `name.rs` or `name/mod.rs`)
// and exposes it publicly (roughly like `public` visibility in C#).
pub mod pact_config;
pub mod pact_instance;
pub mod process_config;
pub mod process_overwatch;
pub mod topology;
