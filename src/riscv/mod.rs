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
    fn parse_first_load_vaddr(image: &[u8]) -> Option<u64> {
        if image.len() < 52 {
            return None;
        }
        // Check ELF magic.
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
                let p_vaddr = u32::from_le_bytes([seg[8], seg[9], seg[10], seg[11]]) as u64;
                return Some(p_vaddr);
            }
        }
        None
    }

    /// Parse the highest address (vaddr + memsz) across all PT_LOAD segments.
    fn parse_elf_highest_addr(image: &[u8]) -> Option<u64> {
        if image.len() < 52 {
            return None;
        }
        if u32::from_le_bytes([image[0], image[1], image[2], image[3]]) != 0x464C457F {
            return None;
        }
        let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;
        let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;
        let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;

        let mut highest: u64 = 0;
        for i in 0..phnum {
            let off = phoff + i * phentsize;
            if off + phentsize > image.len() {
                break;
            }
            let seg = &image[off..off + phentsize];
            let p_type = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
            if p_type == 1 {
                // PT_LOAD
                let p_vaddr = u32::from_le_bytes([seg[8], seg[9], seg[10], seg[11]]) as u64;
                let p_memsz = u32::from_le_bytes([seg[20], seg[21], seg[22], seg[23]]) as u64;
                let seg_end = p_vaddr + p_memsz;
                if seg_end > highest {
                    highest = seg_end;
                }
            }
        }
        if highest == 0 { None } else { Some(highest) }
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
    /// 1. Parse ELF to find first LOAD segment vaddr (becomes ram_base)
    /// 2. Calculate RAM size to fit all segments + initramfs + DTB
    /// 3. Create VM with ram_base = first vaddr
    /// 4. Load kernel ELF at virtual addresses (which are now physical)
    /// 5. Load initramfs after the kernel
    /// 6. Generate DTB with correct ram_base, initrd info, bootargs
    /// 7. Set PC to ELF entry (vaddr, now a valid physical address)
    /// 8. Execute up to max_instructions steps
    pub fn boot_linux(
        kernel_image: &[u8],
        initramfs: Option<&[u8]>,
        ram_size_mb: u32,
        max_instructions: u64,
        bootargs: &str,
    ) -> Result<(Self, BootResult), loader::LoadError> {
        // 1. Parse ELF header to find the first LOAD segment's vaddr.
        // This determines where RAM should start.
        let first_vaddr = Self::parse_first_load_vaddr(kernel_image)
            .unwrap_or(0x8000_0000); // fallback for non-standard kernels

        // 2. Calculate minimum RAM size from kernel segments.
        let min_ram = Self::parse_elf_highest_addr(kernel_image)
            .unwrap_or(first_vaddr + 64 * 1024 * 1024);
        let min_ram_size = (min_ram - first_vaddr) as usize;

        // Use the larger of: caller-specified size, or minimum needed for kernel.
        let caller_ram_size = (ram_size_mb as u64) * 1024 * 1024;
        let actual_ram_size = std::cmp::max(min_ram_size, caller_ram_size as usize);

        // 3. Create VM with RAM starting at the kernel's first LOAD vaddr.
        let mut vm = Self::new_with_base(first_vaddr, actual_ram_size);

        // 4. Load kernel ELF at virtual addresses (which are physical in our VM).
        let load_info = loader::load_elf_vaddr(&mut vm.bus, kernel_image)?;

        // 5. Load initramfs at a page-aligned address after the kernel.
        let (initrd_start, initrd_end) = if let Some(initrd_data) = initramfs {
            let initrd_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
            for (i, &byte) in initrd_data.iter().enumerate() {
                let addr = initrd_addr + i as u64;
                if vm.bus.write_byte(addr, byte).is_err() {
                    break; // initrd doesn't fit, skip it
                }
            }
            let initrd_end_addr = initrd_addr + initrd_data.len() as u64;
            (Some(initrd_addr), Some(initrd_end_addr))
        } else {
            (None, None)
        };

        // 6. Generate DTB with correct ram_base.
        let ram_size = actual_ram_size as u64;
        let dtb_config = dtb::DtbConfig {
            ram_base: first_vaddr,
            ram_size,
            initrd_start,
            initrd_end,
            bootargs: bootargs.to_string(),
            ..Default::default()
        };
        let dtb_blob = dtb::generate_dtb(&dtb_config);

        // Place DTB after kernel (and after initramfs if present).
        let after_initrd = initrd_end.unwrap_or(load_info.highest_addr);
        let dtb_addr = ((after_initrd + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in dtb_blob.iter().enumerate() {
            let addr = dtb_addr + i as u64;
            if vm.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }

        // 7. Set CPU state for boot.
        // Entry point is the ELF entry (virtual address), which IS a valid
        // physical address in our VM since ram_base = first_vaddr.
        let entry: u32 = load_info.entry;
        vm.cpu.pc = entry;
        vm.cpu.x[10] = 0; // a0 = hartid (0)
        vm.cpu.x[11] = dtb_addr as u32; // a1 = DTB address
        vm.cpu.privilege = cpu::Privilege::Machine;

        // Install a minimal M-mode trap handler (firmware stub).
        // On real hardware, OpenSBI/firmware provides this. Our handler:
        //   1. Skips the faulting instruction (mepc += 4)
        //   2. Returns via mret
        // This allows the kernel to proceed past unexpected M-mode traps
        // (e.g., page faults before S-mode trap vectors are set up).
        //
        // Place handler at 0xC0940000 -- in the gap between the kernel's
        // first LOAD segment (ends ~0xC0940000) and second LOAD segment
        // (starts at 0xC0C00000). This address is within the kernel's
        // identity-mapped region and won't be overwritten by the ELF loader.
        let fw_addr: u64 = first_vaddr + 0x940_000;
        // csrr t0, mepc      (0x34202373)
        // addi t0, t0, 4     (0x00428293)
        // csrw mepc, t0      (0x34129073)
        // mret                (0x30200073)
        vm.bus.write_word(fw_addr, 0x34202373).ok();
        vm.bus.write_word(fw_addr + 4, 0x00428293).ok();
        vm.bus.write_word(fw_addr + 8, 0x34129073).ok();
        vm.bus.write_word(fw_addr + 12, 0x30200073).ok();
        // Set mtvec to our trap handler (direct mode, bit[0]=0).
        vm.cpu.csr.write(crate::riscv::csr::MTVEC, fw_addr as u32);

        // Delegate exceptions to S-mode (standard OpenSBI delegation).
        // Bits: 0=instr_misaligned, 1=instr_access, 2=illegal_insn, 3=breakpoint,
        //        8=ecall_U, 9=ecall_S, 12=instr_page_fault, 13=load_page_fault,
        //        15=store_page_fault
        vm.cpu.csr.medeleg = 0xB309;
        // Delegate interrupts to S-mode: bit 1=SSIP, 5=STI, 9=SEI
        vm.cpu.csr.mideleg = 0x222;

        // 8. Execute.
        let mut count: u64 = 0;
        while count < max_instructions {
            // Check for SBI shutdown request
            if vm.bus.sbi.shutdown_requested {
                break;
            }
            match vm.step() {
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

        // The test "passes" as long as it doesn't panic -- we're measuring progress.
        assert!(result.instructions > 0, "Should have executed some instructions");
        // PC should ideally be in kernel code (0xC0xxx range).
        // Log the actual state for diagnostics even if it's not.
        if vm.cpu.pc < 0xC0000000 {
            eprintln!(
                "WARNING: PC outside kernel range: 0x{:08X}. \
                 Kernel may have faulted or jumped to invalid address.",
                vm.cpu.pc
            );
        }
    }
}