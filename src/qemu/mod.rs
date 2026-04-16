// qemu/mod.rs -- Phase 33: QEMU Bridge
//
// Spawns QEMU as a subprocess, pipes serial console I/O through
// the Geometry OS canvas text surface. Supports ANSI escape sequences
// for proper terminal rendering.
//
// Usage from .asm: HYPERVISOR r0  (r0 = address of config string in RAM)
// Config: "arch=riscv64 kernel=linux.img ram=256M disk=rootfs.ext4"

pub mod ansi;
pub mod bridge;
pub mod config;

// Re-export primary types for backward compatibility.
// `use crate::qemu::{AnsiHandler, Cursor}` still works.
pub use ansi::{AnsiHandler, Cursor, CANVAS_COLS, CANVAS_MAX_ROWS};
pub use bridge::QemuBridge;
pub use config::{QemuConfig, arch_to_qemu};
