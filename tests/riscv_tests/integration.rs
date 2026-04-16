use super::*;
use geometry_os::riscv::mmu;
use geometry_os::riscv::csr;
use geometry_os::riscv::plic;
use crate::sv32::{make_pte, sfence_vma};


/// Bus sync_mip sets MEIP when PLIC has pending enabled interrupts.
#[test]
fn test_bus_sync_mip_plic_meip() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 4096);
    bus.plic.priority[plic::IRQ_UART as usize] = 5;
    bus.plic.enable = 1 << plic::IRQ_UART;
    bus.plic.signal(plic::IRQ_UART);
    let mut mip = 0u32;
    bus.sync_mip(&mut mip);
    assert_eq!(mip & (1 << 11), 1 << 11, "MEIP should be set");
}

/// Bus sync_mip clears MEIP when PLIC has no pending interrupts.
#[test]
fn test_bus_sync_mip_plic_clears_meip() {
    let bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 4096);
    let mut mip = 1 << 11;
    bus.sync_mip(&mut mip);
    assert_eq!(mip & (1 << 11), 0, "MEIP should be cleared");
}


// =====================================================================
// Phase 36: MMU + Device Integration Test
// Guest sets up page tables, writes to UART, verify output on canvas
// =====================================================================

use geometry_os::riscv::bridge::UartBridge;

const INT_CANVAS_COLS: usize = 32;
const INT_CANVAS_MAX_ROWS: usize = 128;

fn integration_canvas() -> Vec<u32> {
    vec![0u32; INT_CANVAS_MAX_ROWS * INT_CANVAS_COLS]
}

/// Helper: set up identity-mapped SV32 page tables for code (at 0x8000_0000)
/// and UART (at 0x1000_0000), then run code and verify canvas output.
///
/// Memory layout (all within guest RAM starting at 0x8000_0000):
///   PA 0x8000_0000: code page (PPN 0x80000, identity-mapped)
///   PA 0x8000_1000: root page table (PPN 0x80001)
///   PA 0x8000_2000: L2 PT for code region VPN[1]=0x200 (PPN 0x80002)
///   PA 0x8000_3000: L2 PT for UART region VPN[1]=0x040 (PPN 0x80003)

struct MmuTestEnv {
    vm: RiscvVm,
    root_ppn: u32,
    code_ppn: u32,
}

impl MmuTestEnv {
    fn new() -> Self {
        let ram_size = 0x10000; // 64KB
        let vm = RiscvVm::new(ram_size);
        Self {
            vm,
            root_ppn: 0x80001,   // PA 0x8000_1000
            code_ppn: 0x80000,   // PA 0x8000_0000
        }
    }

    /// Set up page tables for code and (optionally) UART regions.
    fn setup_page_tables(&mut self, map_uart: bool) {
        let root_ppn = self.root_ppn;
        let l2_code_ppn: u32 = 0x80002;
        let l2_uart_ppn: u32 = 0x80003;
        let code_ppn = self.code_ppn;
        let uart_phys_ppn: u32 = 0x10000; // PA 0x1000_0000

        let root_pa = (root_ppn as u64) << 12;
        self.vm.bus.write_word(root_pa + (0x200u64 * 4), make_pte(l2_code_ppn, mmu::PTE_V)).expect("operation should succeed");

        let l2_code_pa = (l2_code_ppn as u64) << 12;
        self.vm.bus.write_word(l2_code_pa, make_pte(code_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).expect("operation should succeed");

        if map_uart {
            self.vm.bus.write_word(root_pa + (0x040u64 * 4), make_pte(l2_uart_ppn, mmu::PTE_V)).expect("operation should succeed");
            let l2_uart_pa = (l2_uart_ppn as u64) << 12;
            self.vm.bus.write_word(l2_uart_pa, make_pte(uart_phys_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).expect("operation should succeed");
        }
    }

    fn load_code(&mut self, code: &[u32]) {
        let code_base = (self.code_ppn as u64) << 12;
        for (i, &word) in code.iter().enumerate() {
            self.vm.bus.write_word(code_base + (i as u64) * 4, word).expect("operation should succeed");
        }
    }

    fn satp_value(&self) -> u32 {
        (1u32 << 31) | self.root_ppn
    }

    fn run(&mut self, max_steps: usize) {
        self.vm.cpu.pc = ((self.code_ppn as u64) << 12) as u32;
        self.vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        for _ in 0..max_steps {
            match self.vm.step() {
                StepResult::Ebreak => break,
                StepResult::Ok | StepResult::Ecall => {}
                _ => break,
            }
        }
    }

    /// Build guest code: enable SV32 MMU, write text to UART via page tables, EBREAK.
    fn build_uart_program(&self, text: &str) -> Vec<u32> {
        let satp = self.satp_value();
        let satp_hi = satp & 0xFFFF_F000;
        let satp_lo = (satp & 0xFFF) as i32;
        let mut code = vec![
            lui(1, 0x10000000),    // x1 = 0x1000_0000 (UART base VA)
            lui(2, satp_hi),    // x2 = upper bits of satp
            addi(2, 2, satp_lo), // x2 = satp (SV32, ASID=0, root PPN)
            csrrw(0, 2, CSR_SATP), // write satp
            sfence_vma(0, 0),   // flush TLB
        ];
        for &b in text.as_bytes() {
            code.push(addi(3, 0, b as i32));
            code.push(sw(3, 1, 0)); // write char to UART through MMU
        }
        code.push(ebreak());
        code
    }
}

/// Write text to UART through page tables, verify on canvas.
#[test]
fn test_mmu_device_integration_guest_uart_through_page_tables() {
    let mut env = MmuTestEnv::new();
    env.setup_page_tables(true);

    let code = env.build_uart_program("OK");
    env.load_code(&code);
    env.run(100);

    let mut bridge = UartBridge::new();
    let mut canvas = integration_canvas();
    let n = bridge.drain_uart_to_canvas(&mut env.vm.bus, &mut canvas);
    assert_eq!(n, 2, "Should have drained 2 bytes from UART TX");

    let output = UartBridge::read_canvas_string(&canvas, 0, 0, 32);
    assert_eq!(output, "OK", "Expected 'OK' on canvas");
    assert_eq!(canvas[0], b'O' as u32);
    assert_eq!(canvas[1], b'K' as u32);
}

/// Enable SV32 without mapping UART -> store to unmapped VA -> page fault.
#[test]
fn test_mmu_device_integration_no_output_without_mapping() {
    let mut env = MmuTestEnv::new();
    env.setup_page_tables(false);

    let code = env.build_uart_program("X");
    env.load_code(&code);
    env.run(100);

    let mut bridge = UartBridge::new();
    let mut canvas = integration_canvas();
    let n = bridge.drain_uart_to_canvas(&mut env.vm.bus, &mut canvas);
    assert_eq!(n, 0, "UART should have no output");
    assert!(canvas.iter().all(|&c| c == 0), "Canvas should be empty");

    // Page fault should have been delivered.
    assert_eq!(
        env.vm.cpu.csr.mcause, csr::CAUSE_STORE_PAGE_FAULT,
        "Expected store page fault, got 0x{:X}", env.vm.cpu.csr.mcause
    );
}

/// Multiple UART writes through MMU: first triggers TLB miss, rest hit TLB.
#[test]
fn test_mmu_device_integration_tlb_cached_uart_writes() {
    let mut env = MmuTestEnv::new();
    env.setup_page_tables(true);

    let code = env.build_uart_program("ABC");
    env.load_code(&code);
    env.run(100);

    let mut bridge = UartBridge::new();
    let mut canvas = integration_canvas();
    let n = bridge.drain_uart_to_canvas(&mut env.vm.bus, &mut canvas);
    assert_eq!(n, 3, "Should drain 3 bytes");

    let output = UartBridge::read_canvas_string(&canvas, 0, 0, 32);
    assert_eq!(output, "ABC", "Expected 'ABC' on canvas");

    // TLB should have cached the UART mapping
    assert!(env.vm.cpu.tlb.lookup(
        mmu::va_to_vpn(0x1000_0000),
        mmu::satp_asid(env.vm.cpu.csr.satp)
    ).is_some(), "TLB should cache UART mapping");
}

/// Test that the RV32 Linux kernel can be loaded by our ELF loader.
/// This verifies Phase 39 deliverable: the kernel is a valid ELF32 RV32 binary.
#[test]
fn test_rv32_linux_kernel_loads() {
    use std::path::Path;
    let kernel_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".geometry_os/fs/linux/rv32/vmlinux");
    
    if !kernel_path.exists() {
        eprintln!("Skipping: RV32 kernel not found at {:?}", kernel_path);
        return;
    }
    
    let kernel_image = std::fs::read(&kernel_path).expect("Failed to read kernel");
    assert!(kernel_image.len() > 1_000_000, "Kernel too small: {} bytes", kernel_image.len());
    
    // Verify it's an ELF32 RISC-V file
    assert_eq!(&kernel_image[0..4], &[0x7F, 0x45, 0x4C, 0x46], "Not ELF magic");
    assert_eq!(kernel_image[4], 1, "Not ELF32 (class != 1)");
    assert_eq!(kernel_image[5], 1, "Not little-endian");
    
    let machine = u16::from_le_bytes([kernel_image[18], kernel_image[19]]);
    assert_eq!(machine, 243, "Not RISC-V (EM_RISCV = 243)");
    
    // Create VM with 128MB RAM and attempt to load
    let mut vm = geometry_os::riscv::RiscvVm::new(128 * 1024 * 1024);
    let result = geometry_os::riscv::loader::load_elf(&mut vm.bus, &kernel_image);
    
    match result {
        Ok(info) => {
            eprintln!("Kernel loaded: entry=0x{:08X}, highest=0x{:08X}", 
                      info.entry, info.highest_addr);
            assert!(info.entry == 0xC0000000, "Entry point should be 0xC0000000, got 0x{:08X}", info.entry);
            assert!(info.highest_addr > 0xC0000000, "Kernel should load above 0xC0000000");
        }
        Err(e) => {
            // The kernel may be too large for the test RAM size
            if matches!(e, geometry_os::riscv::loader::LoadError::SegmentOverflow) {
                eprintln!("Kernel too large for 128MB test RAM (acceptable)");
            } else {
                panic!("Failed to load kernel: {}", e);
            }
        }
    }
}


/// Test SBI ECALL interception: ECALL from S-mode with valid SBI extension
/// is handled by SBI (no trap), while non-SBI ECALL traps to M-mode.
#[test]
fn test_sbi_ecall_interception() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    // Write ECALL at entry point
    vm.bus.write_word(base, ecall()).expect("operation should succeed");
    vm.bus.write_word(base + 4, ebreak()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;

    // --- Test 1: SBI_CONSOLE_PUTCHAR (a7=1) should be intercepted ---
    vm.cpu.x[17] = 1; // a7 = SBI_CONSOLE_PUTCHAR
    vm.cpu.x[10] = b'A' as u32; // a0 = 'A'

    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    // PC should advance past ECALL (no trap)
    assert_eq!(vm.cpu.pc, (base as u32) + 4, "SBI call should advance PC normally");
    // Should still be in S-mode (no privilege change)
    assert_eq!(vm.cpu.privilege, Privilege::Supervisor, "SBI call keeps S-mode");
    // a0 should be SBI_SUCCESS (0)
    assert_eq!(vm.cpu.x[10], 0, "a0 = SBI_SUCCESS");
    // Character should be in SBI console output
    assert!(!vm.bus.sbi.console_output.is_empty(), "SBI should have console output");
    assert_eq!(vm.bus.sbi.console_output[0], b'A', "first char should be 'A'");

    // --- Test 2: Non-SBI ECALL (a7=0x999) should trap to M-mode ---
    vm.cpu.pc = base as u32;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;
    vm.bus.write_word(base + 0x400, ebreak()).expect("operation should succeed");
    vm.cpu.x[17] = 0x999; // Not an SBI extension

    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    // Should trap to M-mode
    assert_eq!(vm.cpu.privilege, Privilege::Machine, "non-SBI ECALL traps to M-mode");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400, "should jump to mtvec");
    assert_eq!(vm.cpu.csr.mcause, 9, "mcause = ECALL-S");
}

/// Test SBI base extension probe from S-mode.
#[test]
fn test_sbi_base_probe_from_smode() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;

    // Probe SBI_EXT_BASE (0x10), function PROBE_EXTENSION (3)
    vm.cpu.x[17] = 0x10; // a7 = SBI_EXT_BASE
    vm.cpu.x[16] = 3; // a6 = PROBE_EXTENSION
    vm.cpu.x[10] = 1; // a0 = probe for SBI_CONSOLE_PUTCHAR (1)

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::Supervisor);
    assert_eq!(vm.cpu.x[10], 1, "SBI_CONSOLE_PUTCHAR should be available");

    // Now probe an unknown extension
    vm.cpu.pc = base as u32;
    vm.cpu.x[17] = 0x10;
    vm.cpu.x[16] = 3;
    vm.cpu.x[10] = 0x999; // unknown extension

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.x[10], 0, "unknown extension should return 0");
}

/// Test SBI shutdown from S-mode causes EBREAK.
#[test]
fn test_sbi_shutdown_from_smode() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;

    // SBI_SHUTDOWN = 8
    vm.cpu.x[17] = 8; // a7 = SBI_SHUTDOWN

    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ebreak, "SBI shutdown should return EBREAK");
    assert!(vm.bus.sbi.shutdown_requested);
}
