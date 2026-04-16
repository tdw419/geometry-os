use super::*;
use geometry_os::riscv::csr;
use geometry_os::riscv::clint;
use geometry_os::riscv::plic;

// ============================================================
// Phase 37: CLINT + PLIC integration tests
// ============================================================

/// CLINT timer fires, CPU traps to mtvec handler, MRET returns.
/// Timing: mtime increments at start of each step, then sync_mip, then instruction.
/// mtimecmp=4 means timer fires when mtime>=4, which is during step 5.
#[test]
fn test_clint_timer_trap_and_mret() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    // Main code (3 instructions)
    vm.bus.write_word(base, addi(5, 0, 42)).expect("operation should succeed");      // 0x00
    vm.bus.write_word(base + 4, addi(6, 0, 99)).expect("operation should succeed");   // 0x04
    vm.bus.write_word(base + 8, nop()).expect("operation should succeed");             // 0x08

    // Trap handler: save mepc, advance by 4, restore, mret
    let handler = 0x8000_0200u64;
    vm.bus.write_word(handler, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(handler + 4, addi(10, 10, 4)).expect("operation should succeed");
    vm.bus.write_word(handler + 8, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(handler + 12, mret()).expect("operation should succeed");

    // Enable MTIE + MIE, set mtvec, timer fires at mtime=4
    vm.cpu.csr.mie = 1 << 7;
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3;
    vm.bus.clint.mtimecmp = 4;

    // Step 1: mtime 0->1, execute addi x5, x0, 42
    vm.step();
    assert_eq!(vm.cpu.x[5], 42);

    // Step 2: mtime 1->2, execute addi x6, x0, 99
    vm.step();
    assert_eq!(vm.cpu.x[6], 99);

    // Step 3: mtime 2->3, execute nop
    vm.step();

    // Step 4: mtime 3->4, timer pending (4>=4), trap fires
    vm.step();
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 7);
    assert_eq!(vm.cpu.pc, 0x8000_0200);

    // Run handler (4 instr)
    vm.step(); // CSRRW mepc -> x10
    vm.step(); // ADDI x10 += 4
    vm.step(); // CSRRW mepc <- x10
    vm.step(); // MRET
    assert_eq!(vm.cpu.pc, 0x8000_0010); // mepc was 0x0C, handler advanced by 4
}

/// CLINT software interrupt (MSIP) triggers a trap.
#[test]
fn test_clint_software_interrupt_trap() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(base + 4, nop()).expect("operation should succeed");
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, mret()).expect("operation should succeed");

    vm.cpu.csr.mie = 1 << 3; // MSIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE

    vm.step(); // normal
    assert_eq!(vm.cpu.pc, 0x8000_0004);

    vm.bus.clint.msip = 1; // trigger
    vm.step(); // MSI trap
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 3);
}

/// PLIC external interrupt sets MEIP and triggers machine trap.
#[test]
fn test_plic_external_interrupt_to_trap() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).expect("operation should succeed");

    // Handler: save mepc, claim from PLIC via MMIO, complete, restore, mret
    // Use direct bus.write_word for PLIC claim/complete instead of CPU instructions
    // to avoid needing LUI+ADDI+LW+SW encoding issues.
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_MEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, mret()).expect("operation should succeed");

    vm.cpu.csr.mie = 1 << 11; // MEIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE

    // Signal UART interrupt via PLIC
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 5;
    vm.bus.plic.enable = 1 << plic::IRQ_UART;
    vm.bus.plic.signal(plic::IRQ_UART);

    vm.step(); // MEI trap
    assert_eq!(vm.cpu.csr.mcause, csr::MCAUSE_INTERRUPT_BIT | 11);
    assert_eq!(vm.cpu.pc, 0x8000_0200);

    // Run handler
    vm.step(); // save mepc
    vm.step(); // restore mepc
    vm.step(); // mret
    assert_eq!(vm.cpu.pc, 0x8000_0000); // MRET returns to mepc (the interrupted NOP)
}

/// MIE=0 blocks interrupt delivery even if everything else is enabled.
#[test]
fn test_interrupt_masked_by_mie() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(base + 4, nop()).expect("operation should succeed");
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mstatus = 0;  // MIE=0!
    vm.bus.clint.mtimecmp = 0;
    vm.step();
    assert_eq!(vm.cpu.pc, 0x8000_0004); // no trap
}

/// CLINT mtime full 64-bit MMIO through the bus.
#[test]
fn test_clint_mtime_mmio_full() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.write_word(clint::MTIME_ADDR, 0xDEAD_BEEF).expect("operation should succeed");
    vm.bus.write_word(clint::MTIME_ADDR + 4, 0x1234_5678).expect("operation should succeed");
    assert_eq!(vm.bus.clint.mtime, 0x1234_5678_DEAD_BEEF);
    assert_eq!(vm.bus.read_word(clint::MTIME_ADDR).expect("operation should succeed"), 0xDEAD_BEEF);
    assert_eq!(vm.bus.read_word(clint::MTIME_ADDR + 4).expect("operation should succeed"), 0x1234_5678);
}

/// CLINT mtimecmp full 64-bit MMIO through the bus.
#[test]
fn test_clint_mtimecmp_mmio_full() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.write_word(clint::MTIMECMP_BASE, 0x0000_0100).expect("operation should succeed");
    vm.bus.write_word(clint::MTIMECMP_BASE + 4, 0x0000_0002).expect("operation should succeed");
    assert_eq!(vm.bus.clint.mtimecmp, 0x0000_0002_0000_0100);
}

/// PLIC threshold blocks low-priority interrupts.
#[test]
fn test_plic_threshold_blocks_low_priority() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.cpu.csr.mie = 1 << 11; // MEIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3;
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 2;
    vm.bus.plic.enable = 1 << plic::IRQ_UART;
    vm.bus.plic.threshold = 3;
    vm.bus.plic.signal(plic::IRQ_UART);
    vm.step();
    assert_eq!(vm.cpu.pc, 0x8000_0004); // no trap -- below threshold
}

/// PLIC claim returns highest priority among multiple sources.
#[test]
fn test_plic_multiple_sources_priority() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.plic.priority[1] = 2;
    vm.bus.plic.priority[plic::IRQ_UART as usize] = 7;
    vm.bus.plic.enable = (1 << 1) | (1 << plic::IRQ_UART);
    vm.bus.plic.signal(1);
    vm.bus.plic.signal(plic::IRQ_UART);
    assert_eq!(vm.bus.plic.claim(), plic::IRQ_UART);
}

/// PLIC complete clears pending and allows next interrupt.
#[test]
fn test_plic_complete_then_next() {
    let mut vm = RiscvVm::new(4096);
    vm.bus.plic.priority[1] = 5;
    vm.bus.plic.priority[2] = 3;
    vm.bus.plic.enable = (1 << 1) | (1 << 2);
    vm.bus.plic.signal(1);
    vm.bus.plic.signal(2);
    let first = vm.bus.plic.claim();
    assert_eq!(first, 1);
    vm.bus.plic.complete(first);
    assert_eq!(vm.bus.plic.pending & (1 << 1), 0);
    let second = vm.bus.plic.claim();
    assert_eq!(second, 2);
}

/// RiscvVm::step drives full interrupt pipeline: tick -> sync -> trap.
#[test]
fn test_riscvvm_step_drives_timer_interrupt() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    for i in 0..10u64 {
        vm.bus.write_word(base + i * 4, nop()).expect("operation should succeed");
    }
    vm.cpu.pc = base as u32;
    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mtvec = 0x8000_0200;
    vm.cpu.csr.mstatus = 1 << 3; // MIE
    vm.bus.clint.mtimecmp = 4;
    vm.step(); // mtime 0->1
    vm.step(); // mtime 1->2
    vm.step(); // mtime 2->3
    vm.step(); // mtime 3->4, now 4>=4, MTIP set, trap fires
    assert_eq!(vm.cpu.csr.mcause & csr::MCAUSE_INTERRUPT_BIT, csr::MCAUSE_INTERRUPT_BIT);
    assert_eq!(vm.cpu.csr.mcause & !csr::MCAUSE_INTERRUPT_BIT, 7);
}

/// S-mode timer interrupt with mideleg delegation.
#[test]
fn test_supervisor_timer_with_delegation() {
    let mut vm = RiscvVm::new(4096);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, nop()).expect("operation should succeed");
    vm.bus.write_word(0x8000_0200, csrrw(10, 0, CSR_SEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0204, csrrw(0, 10, CSR_SEPC)).expect("operation should succeed");
    vm.bus.write_word(0x8000_0208, sret()).expect("operation should succeed");

    vm.cpu.csr.mideleg = 1 << 5; // Delegate STI
    vm.cpu.csr.mie = 1 << 5; // STIE
    vm.cpu.csr.stvec = 0x8000_0200;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.csr.mstatus = 1 << 1; // SIE
    vm.cpu.csr.mip = 1 << 5; // STIP
    vm.step();
    assert_eq!(vm.cpu.csr.scause, csr::MCAUSE_INTERRUPT_BIT | 5);
    assert_eq!(vm.cpu.pc, 0x8000_0200);
}
