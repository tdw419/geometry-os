// riscv/mod.rs -- RISC-V hypervisor module (Phase 34-36)
//
// Pure Rust RISC-V interpreter for Geometry OS.
// Will eventually boot Linux inside the existing canvas VM.
// See docs/RISCV_HYPERVISOR.md for full architecture.

pub mod bus;
pub mod clint;
pub mod cpu;
pub mod csr;
pub mod decode;
pub mod dtb;
pub mod memory;
pub mod mmu;
pub mod plic;
pub mod uart;
pub mod virtio_blk;
pub mod mmu;

use cpu::StepResult;

/// Top-level RISC-V virtual machine.
/// Owns the CPU and the bus (memory + devices).
pub struct RiscvVm {
    pub cpu: cpu::RiscvCpu,
    pub bus: bus::Bus,
}

impl RiscvVm {
    /// Create a new VM with the given RAM size in bytes.
    pub fn new(ram_size: usize) -> Self {
        let bus = bus::Bus::new(0x8000_0000, ram_size);
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
