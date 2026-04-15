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
pub mod sbi;
pub mod syscall;
pub mod trace;
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
                StepResult::Ok
                | StepResult::FetchFault
                | StepResult::LoadFault
                | StepResult::StoreFault => {
                    // Page faults are delivered as traps by translate_va
                    // (mepc/mcause/mtval set, PC jumped to trap vector).
                    // The guest OS trap handler will handle them.
                }
                StepResult::Ebreak => break,
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

    /// Parse the first PT_LOAD segment's virtual address from an ELF image.
    /// Returns None if the image is too short or has no LOAD segments.
    /// Parse the first PT_LOAD segment's physical address from an ELF image.
    fn parse_first_load_paddr(image: &[u8]) -> Option<u64> {
        if image.len() < 52 {
            return None;
        }
        if u32::from_le_bytes([image[0], image[1], image[2], image[3]]) != 0x464C457F {
            return None;
        }
        let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;
        let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;
        let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;

        for i in 0..phnum {
            let off = phoff + i * phentsize;
            if off + phentsize > image.len() {
                break;
            }
            let seg = &image[off..off + phentsize];
            let p_type = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
            if p_type == 1 {
                // PT_LOAD
                let p_paddr = u32::from_le_bytes([seg[12], seg[13], seg[14], seg[15]]) as u64;
                return Some(p_paddr);
            }
        }
        None
    }

    /// Parse the highest physical address (paddr + memsz) across all PT_LOAD segments.
    fn parse_elf_highest_paddr(image: &[u8]) -> Option<u64> {
        let class = crate::riscv::loader::validate_elf_header(image).ok()?;
        let hdr = crate::riscv::loader::parse_elf_header(image, class);

        let mut highest: u64 = 0;
        for i in 0..hdr.phnum {
            let off = hdr.phoff + i * hdr.phentsize;
            let phdr = crate::riscv::loader::parse_phdr(image, off, class)?;
            if phdr.p_type == 1 {
                // PT_LOAD
                let seg_end = phdr.p_paddr as u64 + phdr.p_memsz as u64;
                if seg_end > highest {
                    highest = seg_end;
                }
            }
        }
        if highest == 0 { None } else { Some(highest) }
    }

    /// Convert a virtual entry point to physical using ELF segment mappings.
    /// For Linux, the ELF entry is a virtual address; we find which PT_LOAD
    /// segment contains it and compute phys = entry - p_vaddr + p_paddr.
    /// Supports both ELF32 and ELF64 images.
    fn elf_entry_vaddr_to_phys(image: &[u8], entry_vaddr: u32) -> Option<u32> {
        let class = crate::riscv::loader::validate_elf_header(image).ok()?;
        let hdr = crate::riscv::loader::parse_elf_header(image, class);

        for i in 0..hdr.phnum {
            let off = hdr.phoff + i * hdr.phentsize;
            let phdr = crate::riscv::loader::parse_phdr(image, off, class)?;
            if phdr.p_type == 1 {
                if entry_vaddr >= phdr.p_vaddr
                    && entry_vaddr < phdr.p_vaddr.wrapping_add(phdr.p_memsz as u32)
                {
                    let offset = entry_vaddr - phdr.p_vaddr;
                    return Some(phdr.p_paddr.wrapping_add(offset));
                }
            }
        }
        None
    }

    /// Boot a Linux kernel with initramfs support (associated function).
    ///
    /// This is the main Linux boot entry point. Unlike `boot_guest`, it creates
    /// its own VM with the correct RAM layout for the kernel.
    ///
    /// **Key insight:** The kernel is linked with PAGE_OFFSET (e.g., 0xC0000000).
    /// All code references use virtual addresses in this range. With MMU off in
    /// M-mode, the CPU uses addresses as-is (no translation). So we place RAM
    /// at the kernel's first LOAD segment vaddr, making virtual == physical.
    /// This way, the `J _start_kernel` (which encodes virtual address 0xC00010D0)
    /// fetches from physical 0xC00010D0, which IS in RAM.
    ///
    /// MMIO devices (UART, CLINT, PLIC, virtio) remain at their standard addresses
    /// below 0xC0000000. The bus routes these to device handlers before checking RAM.
    ///
    /// Steps:
    /// Set up the VM for Linux boot without running the instruction loop.
    /// Returns (vm, fw_addr, entry, dtb_addr) so callers can run their own loop.

    pub fn boot_linux_setup(
        kernel_image: &[u8],
        initramfs: Option<&[u8]>,
        ram_size_mb: u32,
        bootargs: &str,
    ) -> Result<(Self, u64, u32, u64), loader::LoadError> {
        // 1. Calculate minimum RAM size from kernel's physical address ranges.
        let highest_paddr = Self::parse_elf_highest_paddr(kernel_image)
            .unwrap_or(64 * 1024 * 1024);
        let min_ram_size = highest_paddr as usize + 4 * 1024 * 1024; // extra for initrd/dtb

        let caller_ram_size = (ram_size_mb as u64) * 1024 * 1024;
        let actual_ram_size = std::cmp::max(min_ram_size, caller_ram_size as usize);

        // 2. Create VM with ram_base=0.
        // This is critical: the kernel computes physical addresses as vaddr - PAGE_OFFSET.
        // With ram_base=0, physical addresses 0x00000000..map directly to RAM,
        // so the kernel's page table writes go to the correct physical locations.
        // Previously ram_base was set to the kernel's first LOAD vaddr (0xC0000000),
        // which caused all physical addresses below 0xC0000000 to be silently discarded.
        let mut vm = Self::new_with_base(0, actual_ram_size);
        vm.bus.low_addr_identity_map = true; // Emulate OpenSBI low-address mappings

        // 3. Load kernel ELF at physical addresses (p_paddr).
        // The kernel's ELF has p_paddr = vaddr - PAGE_OFFSET, which are the correct
        // physical addresses for our ram_base=0 setup.
        let load_info = loader::load_elf(&mut vm.bus, kernel_image)?;

        // 4. Convert virtual entry point to physical address.
        // The ELF entry point is a virtual address (e.g., 0xC0000000).
        // We find which PT_LOAD segment contains it and compute:
        // phys = entry_vaddr - p_vaddr + p_paddr
        let entry_vaddr: u32 = load_info.entry;
        let entry_phys: u32 = Self::elf_entry_vaddr_to_phys(kernel_image, entry_vaddr)
            .unwrap_or_else(|| {
                // Fallback: assume identity mapping (entry is already physical)
                entry_vaddr
            });

        // 5. Load initramfs at a page-aligned address after the kernel.
        let (initrd_start, initrd_end) = if let Some(initrd_data) = initramfs {
            let initrd_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
            for (i, &byte) in initrd_data.iter().enumerate() {
                let addr = initrd_addr + i as u64;
                if vm.bus.write_byte(addr, byte).is_err() {
                    break;
                }
            }
            let initrd_end_addr = initrd_addr + initrd_data.len() as u64;
            (Some(initrd_addr), Some(initrd_end_addr))
        } else {
            (None, None)
        };

        // 6. Generate DTB with ram_base=0.
        let ram_size = actual_ram_size as u64;
        let dtb_config = dtb::DtbConfig {
            ram_base: 0,
            ram_size,
            initrd_start,
            initrd_end,
            bootargs: bootargs.to_string(),
            ..Default::default()
        };
        let dtb_blob = dtb::generate_dtb(&dtb_config);

        let after_initrd = initrd_end.unwrap_or(load_info.highest_addr);
        let dtb_addr = ((after_initrd + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in dtb_blob.iter().enumerate() {
            let addr = dtb_addr + i as u64;
            if vm.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }

        // 7. Set CPU state for boot.
        vm.cpu.x[10] = 0; // a0 = hartid (0)
        vm.cpu.x[11] = dtb_addr as u32; // a1 = DTB physical address

        // Stack for the kernel (mimics OpenSBI).
        let stack_top: u32 = (actual_ram_size as u64 - 4096) as u32;
        vm.cpu.x[2] = stack_top;

        vm.cpu.privilege = cpu::Privilege::Machine;

        // M-mode trap handler (single MRET instruction).
        // Place at a physical address above the kernel code to avoid overlap.
        let fw_addr: u64 = ((load_info.highest_addr + 0xFFF) & !0xFFF) + 0x1000;
        vm.bus.write_word(fw_addr, 0x30200073).ok(); // MRET

        // Set mtvec to our trap handler (physical address).
        vm.cpu.csr.write(crate::riscv::csr::MTVEC, fw_addr as u32);

        // Delegate exceptions to S-mode.
        vm.cpu.csr.medeleg = 0xB109;
        vm.cpu.csr.mideleg = 0x222;

        // Enter S-mode via MRET.
        // mepc = physical entry point (the kernel will enable MMU and start
        // using virtual addresses via page tables).
        vm.cpu.csr.mepc = entry_phys;
        vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB;
        vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
        let restored = vm.cpu.csr.trap_return(cpu::Privilege::Machine);
        vm.cpu.pc = vm.cpu.csr.mepc;
        vm.cpu.privilege = restored;

        Ok((vm, fw_addr, entry_phys, dtb_addr))
    }

    /// Boot a RISC-V Linux kernel.
    /// 1. Calculate RAM size from kernel's physical address ranges (p_paddr + memsz)
    /// 2. Create VM with ram_base = 0 (so physical addresses map directly to RAM)
    /// 3. Load kernel ELF at physical addresses (p_paddr)
    /// 4. Convert virtual entry point to physical (entry - p_vaddr + p_paddr)
    /// 5. Load initramfs after the kernel
    /// 6. Generate DTB with ram_base=0, initrd info, bootargs
    /// 7. Enter S-mode via MRET, kernel enables MMU and uses virtual addresses
    /// 8. Execute up to max_instructions steps with trap forwarding
    pub fn boot_linux(
        kernel_image: &[u8],
        initramfs: Option<&[u8]>,
        ram_size_mb: u32,
        max_instructions: u64,
        bootargs: &str,
    ) -> Result<(Self, BootResult), loader::LoadError> {
        let (mut vm, fw_addr, entry, dtb_addr) =
            Self::boot_linux_setup(kernel_image, initramfs, ram_size_mb, bootargs)?;

        // 8. Execute with Rust-level trap forwarding (OpenSBI emulation).
        //
        // The CPU's trap_target_priv() won't delegate M-mode traps to S-mode
        // (medeleg only applies to traps from lower privileges). So when the
        // kernel takes any exception while running in M-mode, the trap goes to
        // our M-mode handler at fw_addr.
        //
        // We intercept this here: after each step, if the CPU landed at our
        // trap handler, we forward ALL exceptions to S-mode (except ECALL_M
        // which is an SBI call). This emulates OpenSBI behavior where most
        // M-mode traps are reflected to S-mode so the kernel's own handlers
        // can process them (page faults, access faults, etc.).
        let fw_addr_u32 = fw_addr as u32;
        let mut count: u64 = 0;
        let mut _trap_counts: [u64; 32] = [0; 32]; // cause code counts
        let mut _mmode_trap_count: u64 = 0;
        let mut _sbi_call_count: u64 = 0;
        let mut _forward_count: u64 = 0;
        let mut _ecall_m_count: u64 = 0;
        let mut _smode_fault_count: u64 = 0;
        let mut _last_unique_pc: u32 = 0;
        let mut _same_pc_count: u64 = 0;
        while count < max_instructions {
            // Check for SBI shutdown request
            if vm.bus.sbi.shutdown_requested {
                break;
            }

            // Detect if we're sitting at the trap handler from a previous step.
            // This happens when a trap was delivered (mepc/mcause/mtval set,
            // PC jumped to mtvec = fw_addr) and we haven't processed it yet.
            if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == cpu::Privilege::Machine {
                let mcause = vm.cpu.csr.mcause;
                let cause_code = mcause & !(1u32 << 31); // strip interrupt bit

                // ECALL from M-mode (cause 11) is an SBI call -- handle by
                // skipping it (the SBI handler runs elsewhere). All other
                // exceptions should be forwarded to S-mode (OpenSBI behavior),
                // BUT ONLY if they originated from S-mode or U-mode.
                //
                // MPP in mstatus records the privilege level when the trap was
                // taken. If MPP=Machine, the trap came from M-mode code and
                // should NOT be forwarded (real OpenSBI handles these in M-mode;
                // our firmware just skips the faulting instruction).
                // If MPP=Supervisor or MPP=User, the trap came from a lower
                // privilege and OpenSBI would reflect it to S-mode.
                if cause_code != csr::CAUSE_ECALL_M {
                    let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK)
                        >> csr::MSTATUS_MPP_LSB;

                    if (cause_code as usize) < 32 {
                        _trap_counts[cause_code as usize] += 1;
                    }
                    if mpp == 3 {
                        _mmode_trap_count += 1;
                    }

                    // ECALL_S from S-mode is an SBI call -- handle it directly.
                    if cause_code == csr::CAUSE_ECALL_S {
                        _sbi_call_count += 1;
                        let result = vm.bus.sbi.handle_ecall(
                            vm.cpu.x[17], vm.cpu.x[16],
                            vm.cpu.x[10], vm.cpu.x[11],
                            vm.cpu.x[12], vm.cpu.x[13],
                            vm.cpu.x[14], vm.cpu.x[15],
                            &mut vm.bus.uart, &mut vm.bus.clint,
                        );
                        if let Some((a0_val, a1_val)) = result {
                            vm.cpu.x[10] = a0_val;
                            vm.cpu.x[11] = a1_val;
                        }
                        // Fall through to mepc+4 / MRET to return to S-mode.
                    } else if mpp != 3 {
                        // Trap came from S-mode or U-mode -- forward to S-mode.
                        let stvec = vm.cpu.csr.stvec & !0x3u32; // direct mode
                        if stvec != 0 {
                            // Copy M-mode trap info to S-mode CSRs.
                            vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                            vm.cpu.csr.scause = mcause;
                            vm.cpu.csr.stval = vm.cpu.csr.mtval;

                            // Set S-mode trap entry state in mstatus.
                            // SPP = previous privilege (1=S, 0=U) from MPP.
                            let spp = if mpp == 1 { 1u32 } else { 0u32 };
                            vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                                | (spp << csr::MSTATUS_SPP);
                            // SPIE = SIE (save current SIE), SIE = 0 (disable S interrupts)
                            let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                            vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                                | (sie << csr::MSTATUS_SPIE);
                            vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);

                            // Jump to S-mode trap vector in Supervisor mode.
                            vm.cpu.pc = stvec;
                            vm.cpu.privilege = cpu::Privilege::Supervisor;

                            // Flush TLB -- address space context changed.
                            vm.cpu.tlb.flush_all();
                            _forward_count += 1;
                            count += 1;
                            continue;
                        }
                        // stvec not set yet -- fall through to skip instruction.
                    }
                    // MPP=3: trap came from M-mode. Fall through to skip.
                    // This handles device probes to unmapped addresses (e.g.,
                    // 0xFFFFFFF0 PLIC/DTB probes) during early M-mode boot.
                }

                // ECALL_M: Handle as SBI call, then skip instruction.
                if cause_code == csr::CAUSE_ECALL_M {
                    _ecall_m_count += 1;
                    // SBI calling convention: a7=extension, a6=function,
                    // a0..a5=args. Return value in a0 (error), a1 (value).
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], // a7
                        vm.cpu.x[16], // a6
                        vm.cpu.x[10], // a0
                        vm.cpu.x[11], // a1
                        vm.cpu.x[12], // a2
                        vm.cpu.x[13], // a3
                        vm.cpu.x[14], // a4
                        vm.cpu.x[15], // a5
                        &mut vm.bus.uart,
                        &mut vm.bus.clint,
                    );
                    if let Some((a0_val, a1_val)) = result {
                        vm.cpu.x[10] = a0_val; // a0 = error code
                        vm.cpu.x[11] = a1_val; // a1 = return value
                    }
                }

                // ECALL_M or exception with no stvec:
                // Skip the faulting instruction and return via MRET.
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                // The MRET instruction at fw_addr will execute on the next step,
                // returning to mepc (now faulting_pc + 4).
                // Fall through to normal step processing.
            }

            let step_result = vm.step();
            match step_result {
                StepResult::Ok => {}
                StepResult::FetchFault
                | StepResult::LoadFault
                | StepResult::StoreFault => {
                    // Log S-mode faults for debugging (first 20).
                    if vm.cpu.privilege == cpu::Privilege::Supervisor && _smode_fault_count < 20 {
                        _smode_fault_count += 1;
                        let fault_type = match step_result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => unreachable!(),
                        };
                        eprintln!("[boot] S-mode {} fault at count={}: PC=0x{:08X} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                            fault_type, count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                    }
                }
                StepResult::Ebreak => break,
                StepResult::Ecall => {} // ECALL is normal during boot
            }

            // Demand-paging is handled at the MMU level via low_addr_identity_map.
            // No need to patch page tables here.

            // Detect spin loops
            if vm.cpu.pc == _last_unique_pc {
                _same_pc_count += 1;
            } else {
                _last_unique_pc = vm.cpu.pc;
                _same_pc_count = 0;
            }
            if _same_pc_count > 0 && count % 500_000 == 0 {
                eprintln!("[boot] count={} PC=0x{:08X} priv={:?} mstatus=0x{:08X} same_pc={}",
                    count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.mstatus, _same_pc_count);
            }
            count += 1;
        }

        eprintln!("[boot] Done: SBI_calls={} ECALL_M={} forwards={} mmode_traps={}",
            _sbi_call_count, _ecall_m_count, _forward_count, _mmode_trap_count);
        for (i, c) in _trap_counts.iter().enumerate() {
            if *c > 0 {
                eprintln!("[boot]   cause {}: {} occurrences", i, c);
            }
        }

        Ok((
            vm,
            BootResult {
                instructions: count,
                entry,
                dtb_addr,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::bridge::UartBridge;

    const CANVAS_COLS: usize = 32;
    const CANVAS_MAX_ROWS: usize = 128;

    fn make_canvas() -> Vec<u32> {
        vec![0u32; CANVAS_MAX_ROWS * CANVAS_COLS]
    }

    /// Helper: encode LUI rd, imm
    fn enc_lui(rd: u32, imm: u32) -> u32 {
        (imm << 12) | (rd << 7) | 0x37
    }

    /// Helper: encode ADDI rd, rs, imm
    fn enc_addi(rd: u32, rs: u32, imm: u32) -> u32 {
        ((imm & 0xFFF) << 20) | (rs << 15) | (0 << 12) | (rd << 7) | 0x13
    }

    /// Helper: encode SW rs2, offset(rs1)
    fn enc_sw(rs2: u32, rs1: u32, offset: u32) -> u32 {
        ((offset >> 5) << 25)
            | (rs2 << 20)
            | (rs1 << 15)
            | (0b010 << 12)
            | ((offset & 0x1F) << 7)
            | 0x23
    }

    /// Helper: encode EBREAK
    fn enc_ebreak() -> u32 {
        0x00100073
    }

    /// Build a tiny RISC-V binary that writes a string to UART at 0x10000000.
    /// The binary is a sequence of: LUI x1, 0x10000; ADDI x2, x0, char; SW x2, 0(x1)
    /// for each character, followed by EBREAK.
    fn build_uart_program(text: &str) -> Vec<u8> {
        let mut code = Vec::new();
        // LUI x1, 0x10000 -> x1 = 0x1000_0000 (UART base)
        let lui = enc_lui(1, 0x10000);
        code.extend_from_slice(&lui.to_le_bytes());
        for &b in text.as_bytes() {
            // ADDI x2, x0, b
            let addi = enc_addi(2, 0, b as u32);
            code.extend_from_slice(&addi.to_le_bytes());
            // SW x2, 0(x1)
            let sw = enc_sw(2, 1, 0);
            code.extend_from_slice(&sw.to_le_bytes());
        }
        // EBREAK
        code.extend_from_slice(&enc_ebreak().to_le_bytes());
        code
    }

    #[test]
    fn fuzzer_lui_direct() {
        // Replicate exactly what the riscv_fuzzer does for a single LUI instruction.
        // LUI x1, 0x87EE5000 = word 0x87EE50B7
        let ram_base: u64 = 0x8000_0000;
        let ram_size: usize = 4096;
        let mut vm = RiscvVm::new_with_base(ram_base, ram_size);
        vm.cpu.pc = ram_base as u32;
        vm.cpu.csr.satp = 0;
        vm.cpu.csr.mie = 0;
        vm.cpu.csr.mstatus = 0;

        let lui_word: u32 = 0x87EE50B7; // LUI x1, 0x87EE5000
        let ebreak_word: u32 = 0x00100073;
        vm.bus.write_word(ram_base, lui_word).unwrap();
        vm.bus.write_word(ram_base + 4, ebreak_word).unwrap();

        // Step 1: LUI
        let r1 = vm.step();
        assert_eq!(r1, cpu::StepResult::Ok, "LUI should return Ok");
        assert_eq!(vm.cpu.x[1], 0x87EE5000, "x1 should be 0x87EE5000 after LUI");
        assert_eq!(vm.cpu.pc, ram_base as u32 + 4, "PC should advance by 4");

        // Step 2: EBREAK
        let r2 = vm.step();
        assert_eq!(r2, cpu::StepResult::Ebreak, "EBREAK should return Ebreak");
    }

    #[test]
    fn verified_boot_synthetic_kernel() {
        // Build a tiny "kernel" that writes "Linux version 6.1.0" to UART.
        let kernel = build_uart_program("Linux version 6.1.0\n");

        // Create VM with 1MB RAM.
        let mut vm = RiscvVm::new(1024 * 1024);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        // Boot the kernel.
        let result = vm.boot_guest(&kernel, 1, 10_000).unwrap();

        // Should have executed some instructions and stopped at EBREAK.
        assert!(result.instructions > 0);
        assert_eq!(result.entry, 0x8000_0000);
        assert!(result.dtb_addr > 0x8000_0000);

        // Drain UART output to canvas.
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // Verify "Linux version" appears on canvas.
        let output = UartBridge::read_canvas_string(&canvas, 0, 0, 32);
        assert!(
            output.contains("Linux version"),
            "Expected 'Linux version' on canvas, got: '{}'",
            output
        );
    }

    #[test]
    fn boot_sets_dtb_in_a1() {
        // Verify that boot_guest sets a1 (x11) to the DTB address.
        let kernel = build_uart_program("A"); // minimal
        let mut vm = RiscvVm::new(64 * 1024);
        let _ = vm.boot_guest(&kernel, 1, 100);

        // x10 should be 0 (hartid), x11 should be DTB address.
        assert_eq!(vm.cpu.x[10], 0, "a0 should be 0 (hartid)");
        assert!(vm.cpu.x[11] > 0, "a1 should be DTB address, got {}", vm.cpu.x[11]);

        // Verify the DTB is actually at that address (starts with FDT magic).
        let dtb_addr = vm.cpu.x[11] as u64;
        let byte0 = vm.bus.read_byte(dtb_addr).unwrap();
        // FDT magic is 0xD00DFEED stored big-endian, first byte is 0xD0.
        assert_eq!(byte0, 0xD0, "DTB should start with FDT magic byte (0xD0)");
    }

    #[test]
    fn boot_raw_binary_at_default_base() {
        // Raw (non-ELF) binary should load at 0x8000_0000.
        let kernel = build_uart_program("OK");
        let mut vm = RiscvVm::new(64 * 1024);
        let result = vm.boot_guest(&kernel, 1, 100).unwrap();

        assert_eq!(result.entry, 0x8000_0000);
    }

    #[test]
    fn boot_elf_kernel() {
        // Build a minimal ELF32 RISC-V kernel with a UART program.
        let code = build_uart_program("HELLO");
        let mut img = Vec::new();

        // ELF header (52 bytes).
        let elf_magic: u32 = 0x464C457F;
        img.extend_from_slice(&elf_magic.to_le_bytes());
        img.push(1); // class: 32-bit
        img.push(1); // endian: little
        img.push(1); // version
        img.extend_from_slice(&[0u8; 9]); // padding (OS/ABI etc)
        img.extend_from_slice(&2u16.to_le_bytes()); // e_type: ET_EXEC
        img.extend_from_slice(&243u16.to_le_bytes()); // e_machine: EM_RISCV
        img.extend_from_slice(&1u32.to_le_bytes()); // version
        let entry = 0x8000_0000u32;
        img.extend_from_slice(&entry.to_le_bytes()); // entry
        img.extend_from_slice(&52u32.to_le_bytes()); // phoff
        img.extend_from_slice(&0u32.to_le_bytes()); // shoff (no section headers)
        img.extend_from_slice(&0u32.to_le_bytes()); // flags
        img.extend_from_slice(&52u16.to_le_bytes()); // ehsize
        img.extend_from_slice(&32u16.to_le_bytes()); // phentsize
        img.extend_from_slice(&1u16.to_le_bytes()); // phnum
        img.extend_from_slice(&0u16.to_le_bytes()); // shentsize
        img.extend_from_slice(&0u16.to_le_bytes()); // shnum
        img.extend_from_slice(&0u16.to_le_bytes()); // shstrndx

        // Program header (32 bytes) for PT_LOAD at 0x8000_0000.
        let data_offset = 52 + 32; // data starts after header + phdr
        img.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
        img.extend_from_slice(&(data_offset as u32).to_le_bytes()); // p_offset
        img.extend_from_slice(&0x8000_0000u32.to_le_bytes()); // p_vaddr
        img.extend_from_slice(&0x8000_0000u32.to_le_bytes()); // p_paddr
        img.extend_from_slice(&(code.len() as u32).to_le_bytes()); // p_filesz
        img.extend_from_slice(&(code.len() as u32).to_le_bytes()); // p_memsz
        img.extend_from_slice(&5u32.to_le_bytes()); // p_flags = RX
        img.extend_from_slice(&4096u32.to_le_bytes()); // p_align

        // Code data.
        img.extend_from_slice(&code);

        let mut vm = RiscvVm::new(64 * 1024);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        let result = vm.boot_guest(&img, 1, 10_000).unwrap();
        assert_eq!(result.entry, 0x8000_0000);

        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
        let output = UartBridge::read_canvas_string(&canvas, 0, 0, 8);
        assert_eq!(output, "HELLO");
    }

    #[test]
    fn boot_dtb_is_valid_fdt() {
        // Boot with any kernel, then verify the DTB in RAM is a valid FDT.
        let kernel = build_uart_program("X");
        let mut vm = RiscvVm::new(64 * 1024);
        let _ = vm.boot_guest(&kernel, 128, 100);

        let dtb_addr = vm.cpu.x[11] as u64;
        let b0 = vm.bus.read_byte(dtb_addr).unwrap();
        let b1 = vm.bus.read_byte(dtb_addr + 1).unwrap();
        let b2 = vm.bus.read_byte(dtb_addr + 2).unwrap();
        let b3 = vm.bus.read_byte(dtb_addr + 3).unwrap();
        let magic = u32::from_be_bytes([b0, b1, b2, b3]);
        assert_eq!(magic, 0xD00D_FEED, "DTB should have FDT magic");

        // Verify totalsize field matches.
        let ts0 = vm.bus.read_byte(dtb_addr + 4).unwrap();
        let ts1 = vm.bus.read_byte(dtb_addr + 5).unwrap();
        let ts2 = vm.bus.read_byte(dtb_addr + 6).unwrap();
        let ts3 = vm.bus.read_byte(dtb_addr + 7).unwrap();
        let totalsize = u32::from_be_bytes([ts0, ts1, ts2, ts3]) as usize;
        assert!(totalsize > 40, "DTB should be > 40 bytes");
    }

    #[test]
    fn boot_keyboard_roundtrip() {
        // Boot a kernel, inject keyboard input via bridge, verify guest can read it.
        let kernel = build_uart_program(">");
        let mut vm = RiscvVm::new(64 * 1024);
        let mut bridge = UartBridge::new();
        let mut canvas = make_canvas();

        let _ = vm.boot_guest(&kernel, 1, 1_000);
        bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

        // Inject keyboard input.
        bridge.forward_key(&mut vm.bus, b'H');
        bridge.forward_key(&mut vm.bus, b'i');

        // Guest reads it back.
        assert_eq!(vm.bus.uart.read_byte(0), b'H');
        assert_eq!(vm.bus.uart.read_byte(0), b'i');
    }

    #[test]
    fn performance_mips_benchmark() {
        // Measure instructions per second of the interpreter.
        // Build a kernel that does pure computation (NOP loop) for measurement.
        let mut code = Vec::new();
        // 1000 NOPs (ADDI x0, x0, 0) followed by EBREAK.
        for _ in 0..1000 {
            let nop = enc_addi(0, 0, 0); // NOP
            code.extend_from_slice(&nop.to_le_bytes());
        }
        code.extend_from_slice(&enc_ebreak().to_le_bytes());

        let mut vm = RiscvVm::new(64 * 1024);

        let start = std::time::Instant::now();
        let result = vm.boot_guest(&code, 1, 100_000).unwrap();
        let elapsed = start.elapsed();

        let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;

        // Log the result (visible in test output with --nocapture).
        eprintln!(
            "Phase 37 MIPS benchmark: {} instructions in {:?} = {:.2} MIPS",
            result.instructions, elapsed, mips
        );

        // Sanity: should have executed exactly 1000 NOPs + 1 EBREAK = 1001.
        // EBREAK stops execution before incrementing count, so we get 1000 NOPs executed.
        assert_eq!(result.instructions, 1000);

        // Sanity: MIPS should be > 0 (trivially true but documents intent).
        assert!(mips > 0.0, "MIPS should be positive, got {}", mips);

        // Performance gate: interpreter should exceed 1 MIPS on any modern CPU.
        // This is a very conservative floor -- real performance should be 10-50+ MIPS.
        // Only enforce in release builds -- debug mode is too slow for this threshold.
        #[cfg(not(debug_assertions))]
        assert!(
            mips > 1.0,
            "Interpreter should exceed 1 MIPS, got {:.2} MIPS",
            mips
        );
        #[cfg(debug_assertions)]
        {
            // In debug mode just log; the release build gate catches real regressions.
            eprintln!(
                "  (debug mode: skipping 1 MIPS gate, got {:.2} MIPS)",
                mips
            );
        }
    }

    #[test]
    fn boot_guest_empty_image_runs_nop_loop() {
        // An empty raw binary loads at 0x8000_0000 with entry=0x8000_0000.
        // All-zero RAM decodes as ADDI x0, x0, 0 (NOP) so the CPU runs all N steps.
        let mut vm = RiscvVm::new(64 * 1024);
        let result = vm.boot_guest(&[], 1, 100).unwrap();
        assert_eq!(result.instructions, 100);
        assert_eq!(result.entry, 0x8000_0000);
    }

    #[test]
    fn test_linux_kernel_early_boot() {
        use std::fs;
        use std::time::Instant;

        let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
        let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

        // Skip if kernel not present (CI, etc.)
        let kernel_data = match fs::read(kernel_path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Skipping: {} not found", kernel_path);
                return;
            }
        };
        let initramfs_data = fs::read(initramfs_path).ok();

        eprintln!("Kernel size: {} bytes", kernel_data.len());
        if let Some(ref ir) = initramfs_data {
            eprintln!("Initramfs size: {} bytes", ir.len());
        }

        let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
        let start = Instant::now();
        let (mut vm, result) = RiscvVm::boot_linux(
            &kernel_data,
            initramfs_data.as_deref(),
            512, // 512MB RAM (kernel needs ~305MB)
            5_000_000, // 5M instructions
            bootargs,
        ).unwrap();

        let elapsed = start.elapsed();
        let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;
        eprintln!(
            "Linux boot: {} instructions in {:?} = {:.2} MIPS",
            result.instructions, elapsed, mips
        );
        eprintln!("Entry: 0x{:08X}, DTB at: 0x{:08X}", result.entry, result.dtb_addr);
        eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
        eprintln!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);

        // Check UART output
        let mut uart_output = Vec::new();
        loop {
            match vm.bus.uart.read_byte(0) {
                0 => break, // no more data
                b => uart_output.push(b),
            }
        }
        if !uart_output.is_empty() {
            let s = String::from_utf8_lossy(&uart_output);
            eprintln!("UART output ({} bytes): {}", uart_output.len(), s);
        } else {
            eprintln!("No UART output");
        }

        // Check CSRs
        eprintln!("mcause: 0x{:08X}, mepc: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc);
        eprintln!("scause: 0x{:08X}, sepc: 0x{:08X}", vm.cpu.csr.scause, vm.cpu.csr.sepc);
        eprintln!("satp: 0x{:08X}", vm.cpu.csr.satp);
        eprintln!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);

        // Read instruction at mepc for diagnostics
        let mepc_pa = vm.cpu.csr.mepc as u64;
        match vm.bus.read_word(mepc_pa) {
            Ok(word) => {
                let hw = (word & 0xFFFF) as u16;
                let is_c = (hw & 0x3) != 0x3;
                eprintln!("Instruction at mepc: word=0x{:08X}, low16=0x{:04X} compressed={}", word, hw, is_c);
                if is_c {
                    eprintln!("  Decoded as: quadrant={}, funct3={}", hw & 0x3, (hw >> 13) & 0x7);
                }
            }
            Err(_) => eprintln!("Could not read instruction at mepc 0x{:08X}", mepc_pa),
        }

        // The test "passes" as long as it doesn't panic -- we're measuring progress.
        assert!(result.instructions > 0, "Should have executed some instructions");
        // With ram_base=0, PC may be a physical address (below 0x02000000)
        // or a virtual address (0xC0xxxxxx) after MMU is enabled.
        eprintln!(
            "Boot result: PC=0x{:08X}, instructions={}",
            vm.cpu.pc, result.instructions
        );
    }

    #[test]
    fn test_parse_first_load_paddr() {
        // Build a minimal ELF with one PT_LOAD segment at paddr=0x100000
        let elf = make_test_elf(0x80000000, 0x100000, 0x1000, 0x1000);
        let result = RiscvVm::parse_first_load_paddr(&elf);
        assert_eq!(result, Some(0x100000));
    }

    #[test]
    fn test_parse_elf_highest_paddr() {
        // Two PT_LOAD segments: paddr 0x0 with memsz 0x1000, paddr 0x100000 with memsz 0x2000
        let elf = make_test_elf_two_segments(
            0x80000000, 0x00000000, 0x1000, 0x1000,
            0x00100000, 0x2000, 0x2000,
        );
        let result = RiscvVm::parse_elf_highest_paddr(&elf);
        assert_eq!(result, Some(0x102000));
    }

    #[test]
    fn test_elf_entry_vaddr_to_phys() {
        // Entry at vaddr 0x80001000, segment vaddr=0x80000000, paddr=0x00000000
        // Physical entry should be 0x00001000
        let elf = make_test_elf(0x80000000, 0x00000000, 0x2000, 0x2000);
        let result = RiscvVm::elf_entry_vaddr_to_phys(&elf, 0x80001000);
        assert_eq!(result, Some(0x00001000));
    }

    #[test]
    fn test_elf_entry_vaddr_to_phys_second_segment() {
        // Entry at vaddr 0x80101000, second segment vaddr=0x80100000, paddr=0x100000
        let elf = make_test_elf_two_segments(
            0x80000000, 0x00000000, 0x1000, 0x1000,
            0x00100000, 0x2000, 0x2000,
        );
        let result = RiscvVm::elf_entry_vaddr_to_phys(&elf, 0x80101000);
        assert_eq!(result, Some(0x00101000));
    }

    /// Build a minimal ELF32 RISC-V image with one PT_LOAD segment.
    fn make_test_elf(entry: u32, paddr: u64, filesz: u32, memsz: u32) -> Vec<u8> {
        let vaddr = entry; // entry is at the start of the segment
        let mut elf = Vec::new();
        // ELF32 header (52 bytes)
        // e_ident (16 bytes)
        elf.extend_from_slice(&[0x7F, 0x45, 0x4C, 0x46]); // magic
        elf.push(1); // EI_CLASS: 32-bit
        elf.push(1); // EI_DATA: little-endian
        elf.extend_from_slice(&[0; 9]); // padding (EI_VERSION through EI_PAD)
        elf.extend_from_slice(&[0]); // EI_NIDENT padding
        // e_type (2), e_machine (2), e_version (4)
        elf.extend_from_slice(&2u16.to_le_bytes()); // e_type = ET_EXEC
        elf.extend_from_slice(&0xF3u16.to_le_bytes()); // e_machine = EM_RISCV
        elf.extend_from_slice(&1u32.to_le_bytes()); // e_version = 1
        // e_entry (4)
        elf.extend_from_slice(&entry.to_le_bytes());
        // e_phoff (4)
        elf.extend_from_slice(&52u32.to_le_bytes());
        // e_shoff (4)
        elf.extend_from_slice(&0u32.to_le_bytes());
        // e_flags (4)
        elf.extend_from_slice(&0u32.to_le_bytes());
        // e_ehsize (2), e_phentsize (2), e_phnum (2), e_shentsize (2), e_shnum (2), e_shstrndx (2)
        elf.extend_from_slice(&52u16.to_le_bytes()); // e_ehsize
        elf.extend_from_slice(&32u16.to_le_bytes()); // e_phentsize
        elf.extend_from_slice(&1u16.to_le_bytes()); // e_phnum
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shentsize
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx
        assert_eq!(elf.len(), 52);
        // Program header (32 bytes)
        elf.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
        elf.extend_from_slice(&0u32.to_le_bytes()); // p_offset
        elf.extend_from_slice(&vaddr.to_le_bytes()); // p_vaddr
        elf.extend_from_slice(&(paddr as u32).to_le_bytes()); // p_paddr
        elf.extend_from_slice(&filesz.to_le_bytes()); // p_filesz
        elf.extend_from_slice(&memsz.to_le_bytes()); // p_memsz
        elf.extend_from_slice(&[5, 0, 0, 0]); // p_flags = R+X
        elf.extend_from_slice(&0x1000u32.to_le_bytes()); // p_align
        // Pad to filesz
        while elf.len() < 52 + 32 + filesz as usize {
            elf.push(0);
        }
        elf
    }

    /// Build a minimal ELF32 RISC-V image with two PT_LOAD segments.
    fn make_test_elf_two_segments(
        entry: u32,
        paddr1: u64, filesz1: u32, memsz1: u32,
        paddr2: u64, filesz2: u32, memsz2: u32,
    ) -> Vec<u8> {
        let vaddr1 = entry;
        let vaddr2 = 0x80100000u32;
        let mut elf = Vec::new();
        // ELF32 header (52 bytes)
        elf.extend_from_slice(&[0x7F, 0x45, 0x4C, 0x46]); // magic
        elf.push(1); // EI_CLASS: 32-bit
        elf.push(1); // EI_DATA: little-endian
        elf.extend_from_slice(&[0; 9]); // padding
        elf.push(0); // EI_NIDENT padding
        elf.extend_from_slice(&2u16.to_le_bytes()); // e_type = ET_EXEC
        elf.extend_from_slice(&0xF3u16.to_le_bytes()); // e_machine = EM_RISCV
        elf.extend_from_slice(&1u32.to_le_bytes()); // e_version
        elf.extend_from_slice(&entry.to_le_bytes()); // e_entry
        elf.extend_from_slice(&52u32.to_le_bytes()); // e_phoff
        elf.extend_from_slice(&0u32.to_le_bytes()); // e_shoff
        elf.extend_from_slice(&0u32.to_le_bytes()); // e_flags
        elf.extend_from_slice(&52u16.to_le_bytes()); // e_ehsize
        elf.extend_from_slice(&32u16.to_le_bytes()); // e_phentsize
        elf.extend_from_slice(&2u16.to_le_bytes()); // e_phnum
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shentsize
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
        elf.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx
        assert_eq!(elf.len(), 52);
        // Segment 1
        elf.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
        elf.extend_from_slice(&0u32.to_le_bytes()); // p_offset
        elf.extend_from_slice(&vaddr1.to_le_bytes()); // p_vaddr
        elf.extend_from_slice(&(paddr1 as u32).to_le_bytes()); // p_paddr
        elf.extend_from_slice(&filesz1.to_le_bytes()); // p_filesz
        elf.extend_from_slice(&memsz1.to_le_bytes()); // p_memsz
        elf.extend_from_slice(&[5, 0, 0, 0]); // p_flags = R+X
        elf.extend_from_slice(&0x1000u32.to_le_bytes()); // p_align
        // Segment 2
        let seg2_offset = (52 + 32 + filesz1 as usize) as u32;
        elf.extend_from_slice(&1u32.to_le_bytes()); // p_type = PT_LOAD
        elf.extend_from_slice(&seg2_offset.to_le_bytes()); // p_offset
        elf.extend_from_slice(&vaddr2.to_le_bytes()); // p_vaddr
        elf.extend_from_slice(&(paddr2 as u32).to_le_bytes()); // p_paddr
        elf.extend_from_slice(&filesz2.to_le_bytes()); // p_filesz
        elf.extend_from_slice(&memsz2.to_le_bytes()); // p_memsz
        elf.extend_from_slice(&[6, 0, 0, 0]); // p_flags = RW
        elf.extend_from_slice(&0x1000u32.to_le_bytes()); // p_align
        elf
    }
}