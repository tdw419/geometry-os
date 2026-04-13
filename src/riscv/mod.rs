// riscv/mod.rs -- RISC-V hypervisor module (Phase 34 stub)
//
// Pure Rust RISC-V interpreter for Geometry OS.
// Will eventually boot Linux inside the existing canvas VM.
// See docs/RISCV_HYPERVISOR.md for full architecture.

pub mod cpu;
pub mod decode;
pub mod memory;

/// Top-level RISC-V virtual machine.
/// Owns the CPU, memory, and (eventually) device bus.
pub struct RiscvVm {
    pub cpu: cpu::RiscvCpu,
    pub mem: memory::GuestMemory,
}

impl RiscvVm {
    /// Create a new VM with the given RAM size in bytes.
    pub fn new(ram_size: usize) -> Self {
        let mem = memory::GuestMemory::new(0x8000_0000, ram_size);
        let cpu = cpu::RiscvCpu::new();
        Self { cpu, mem }
    }
}
