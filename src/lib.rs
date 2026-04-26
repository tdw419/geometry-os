// lib.rs -- Geometry OS library
// Exposes VM and assembler for integration tests

pub mod assembler;
pub mod episode_log;
pub mod font;
pub mod glyph_backend;
pub mod inode_fs;
#[cfg(feature = "native")]
pub mod keys;
#[allow(dead_code)]
pub mod pixel;
pub mod preprocessor;
pub mod qemu;
pub mod riscv;
pub mod scrollback;
pub mod vfs;
pub mod viewport;
#[allow(dead_code)]
pub mod vision;
pub mod vm;
