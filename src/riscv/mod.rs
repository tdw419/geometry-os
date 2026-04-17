// riscv/mod.rs -- RISC-V hypervisor module (Phases 34-37)
//
// Pure Rust RISC-V interpreter for Geometry OS.
// Boots guest OS kernels on the canvas text surface.
// See docs/RISCV_HYPERVISOR.md for full architecture.

pub mod boot;
pub mod bridge;
pub mod bus;
pub mod clint;
pub mod cpu;
pub mod csr;
pub mod decode;
pub mod dtb;
pub mod loader;
pub mod memory;
pub mod mmu;
pub mod plic;
pub mod sbi;
pub mod syscall;
pub mod trace;
pub mod uart;
pub mod virtio_blk;

#[cfg(test)]
mod tests;

use cpu::StepResult;

/// Top-level RISC-V virtual machine.
/// Owns the CPU and the bus (memory + devices).
pub struct RiscvVm {
    pub cpu: cpu::RiscvCpu,
    pub bus: bus::Bus,
}

/// Result of a guest boot attempt.
#[derive(Debug)]
pub struct BootResult {
    /// Number of instructions executed.
    pub instructions: u64,
    /// Entry point where CPU started.
    pub entry: u32,
    /// Address where DTB was loaded.
    pub dtb_addr: u64,
}

#[allow(dead_code)]
impl RiscvVm {
    /// Create a new VM with the given RAM size in bytes.
    /// RAM starts at 0x8000_0000 (default for synthetic tests).
    pub fn new(ram_size: usize) -> Self {
        let bus = bus::Bus::new(0x8000_0000, ram_size);
        let cpu = cpu::RiscvCpu::new();
        Self { cpu, bus }
    }

    /// Create a new VM with a custom RAM base address.
    /// Used for Linux boot where RAM starts at 0x0000_0000.
    pub fn new_with_base(ram_base: u64, ram_size: usize) -> Self {
        let bus = bus::Bus::new(ram_base, ram_size);
        let cpu = cpu::RiscvCpu::new();
        Self { cpu, bus }
    }

    /// Execute one step: tick CLINT, sync MIP, run instruction.
    pub fn step(&mut self) -> StepResult {
        // 1. Advance CLINT timer
        self.bus.tick_clint();

        // 2. Sync CLINT hardware state into MIP
        self.bus.sync_mip(&mut self.cpu.csr.mip);

        // 3. Execute one CPU instruction via the bus
        self.cpu.step(&mut self.bus)
    }
}
