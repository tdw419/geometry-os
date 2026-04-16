// qemu/mod.rs -- Phase 33: QEMU Bridge
//
// Re-exports from submodules. External code uses:
//   use crate::qemu::{AnsiHandler, Cursor, QemuBridge, QemuConfig};

pub mod ansi;
pub mod bridge;
pub mod config;

// Re-export public types for convenience.
pub use ansi::{AnsiHandler, Cursor};
pub use bridge::QemuBridge;
pub use config::QemuConfig;
