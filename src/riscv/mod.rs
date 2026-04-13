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
}