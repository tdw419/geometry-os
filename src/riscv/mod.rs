// riscv/mod.rs -- RISC-V hypervisor module (Phases 34-37)
//
// Pure Rust RISC-V interpreter for Geometry OS.
// Boots guest OS kernels on the canvas text surface.
// See docs/RISCV_HYPERVISOR.md for full architecture.

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
pub mod uart;
pub mod virtio_blk;

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

    /// Boot a guest OS kernel image.
    ///
    /// 1. Load kernel image (ELF32 or raw binary) into guest RAM
    /// 2. Generate and load a DTB (device tree blob) into guest RAM
    /// 3. Set PC to entry point, a0=0 (hartid), a1=dtb_addr
    /// 4. Run for `max_instructions` steps or until EBREAK/halt
    ///
    /// Returns the number of instructions executed and boot metadata.
    pub fn boot_guest(
        &mut self,
        kernel_image: &[u8],
        ram_size_mb: u32,
        max_instructions: u64,
    ) -> Result<BootResult, loader::LoadError> {
        // 1. Load kernel image.
        let load_info = loader::load_auto(&mut self.bus, kernel_image, 0x8000_0000)?;

        // 2. Generate DTB and load it into guest RAM just after the kernel.
        let dtb_config = dtb::DtbConfig {
            ram_size: ram_size_mb as u64 * 1024 * 1024,
            ..Default::default()
        };
        let dtb_blob = dtb::generate_dtb(&dtb_config);

        // Place DTB at a page-aligned address after the kernel image.
        let dtb_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in dtb_blob.iter().enumerate() {
            let addr = dtb_addr + i as u64;
            if self.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }

        // 3. Set CPU state for boot.
        self.cpu.pc = load_info.entry;
        self.cpu.x[10] = 0; // a0 = hartid (0)
        self.cpu.x[11] = dtb_addr as u32; // a1 = DTB address
        self.cpu.privilege = cpu::Privilege::Machine;

        // 4. Execute.
        let mut count: u64 = 0;
        while count < max_instructions {
            match self.step() {
                StepResult::Ok => {}
                StepResult::Ebreak => break,
                StepResult::FetchFault
                | StepResult::LoadFault
                | StepResult::StoreFault => break,
                StepResult::Ecall => {} // ECALL is normal during boot
            }
            count += 1;
        }

        Ok(BootResult {
            instructions: count,
            entry: load_info.entry,
            dtb_addr,
        })
    }
}
