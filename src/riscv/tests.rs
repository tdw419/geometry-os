// riscv/tests.rs -- Tests for RiscvVm (extracted from mod.rs)

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
    vm.bus.write_word(ram_base, lui_word).expect("operation should succeed");
    vm.bus.write_word(ram_base + 4, ebreak_word).expect("operation should succeed");

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
    let result = vm.boot_guest(&kernel, 1, 10_000).expect("operation should succeed");

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
    let byte0 = vm.bus.read_byte(dtb_addr).expect("operation should succeed");
    // FDT magic is 0xD00DFEED stored big-endian, first byte is 0xD0.
    assert_eq!(byte0, 0xD0, "DTB should start with FDT magic byte (0xD0)");
}

#[test]
fn boot_raw_binary_at_default_base() {
    // Raw (non-ELF) binary should load at 0x8000_0000.
    let kernel = build_uart_program("OK");
    let mut vm = RiscvVm::new(64 * 1024);
    let result = vm.boot_guest(&kernel, 1, 100).expect("operation should succeed");

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

    let result = vm.boot_guest(&img, 1, 10_000).expect("operation should succeed");
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
    let b0 = vm.bus.read_byte(dtb_addr).expect("operation should succeed");
    let b1 = vm.bus.read_byte(dtb_addr + 1).expect("operation should succeed");
    let b2 = vm.bus.read_byte(dtb_addr + 2).expect("operation should succeed");
    let b3 = vm.bus.read_byte(dtb_addr + 3).expect("operation should succeed");
    let magic = u32::from_be_bytes([b0, b1, b2, b3]);
    assert_eq!(magic, 0xD00D_FEED, "DTB should have FDT magic");

    // Verify totalsize field matches.
    let ts0 = vm.bus.read_byte(dtb_addr + 4).expect("operation should succeed");
    let ts1 = vm.bus.read_byte(dtb_addr + 5).expect("operation should succeed");
    let ts2 = vm.bus.read_byte(dtb_addr + 6).expect("operation should succeed");
    let ts3 = vm.bus.read_byte(dtb_addr + 7).expect("operation should succeed");
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
    let result = vm.boot_guest(&code, 1, 100_000).expect("operation should succeed");
    let elapsed = start.elapsed();

    let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;

    // Log the result (visible in test output with --nocapture).
    eprintln!(
        "Phase 37 MIPS benchmark: {} instructions in {:?} = {:.2} MIPS",
        result.instructions, elapsed, mips
    );

    // Sanity: should have executed exactly 1000 NOPs + 1 EBREAK = 1000.
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
    let result = vm.boot_guest(&[], 1, 100).expect("operation should succeed");
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
    ).expect("operation should succeed");

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
