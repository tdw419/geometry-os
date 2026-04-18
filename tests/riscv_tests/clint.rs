use super::*;
use crate::privilege::{run_vm, MSTATUS_MIE_BIT};

/// Test full CLINT timer pipeline: advance mtime until >= mtimecmp,
/// verify timer interrupt is delivered via RiscvVm::step().
#[test]
fn test_clint_timer_interrupt_via_vm_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Entry: 5 NOPs (timer fires at step 4 when mtime reaches 5)
    for i in 0..5u64 {
        vm.bus
            .write_word(base + i * 4, nop())
            .expect("operation should succeed");
    }
    // Handler at +0x200: write 42 to x1, then EBREAK
    vm.bus
        .write_word(base + 0x200, addi(1, 0, 42))
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 0x204, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Set up CLINT: timer fires at mtime=5
    vm.bus.clint.mtime = 0;
    vm.bus.clint.mtimecmp = 5;

    // Enable machine timer interrupt
    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT; // Global MIE
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Run: after 5 ticks, mtime >= mtimecmp, timer fires
    run_vm(&mut vm, 20);

    assert_eq!(vm.cpu.x[1], 42, "timer handler should have set x1=42");
    assert_eq!(
        vm.cpu.csr.mcause, 0x80000007,
        "mcause should be interrupt | MTI (7)"
    );
    assert_eq!(
        vm.cpu.privilege,
        geometry_os::riscv::cpu::Privilege::Machine
    );
}

/// Test CLINT software interrupt: set msip, verify MSI is delivered.
#[test]
fn test_clint_software_interrupt_via_vm_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus
        .write_word(base, nop())
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 0x200, addi(1, 0, 99))
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 0x204, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Trigger software interrupt via CLINT msip
    vm.bus.clint.msip = 1;

    // Enable machine software interrupt
    vm.cpu.csr.mie = 1 << 3; // MSIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    run_vm(&mut vm, 20);

    assert_eq!(
        vm.cpu.x[1], 99,
        "software interrupt handler should have set x1=99"
    );
    assert_eq!(
        vm.cpu.csr.mcause, 0x80000003,
        "mcause should be interrupt | MSI (3)"
    );
}

/// Test full CLINT MMIO read: guest code reads mtime via LW from CLINT address.
#[test]
fn test_clint_mmio_read_mtime() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Set mtime to a known value (high word >> 32 so no carry from low ticks)
    vm.bus.clint.mtime = 0x0000_0042_0000_0100;

    // Program: load mtime from CLINT MMIO address 0x0200BFF8
    vm.bus
        .write_word(base, lui(5, 0x0200C000))
        .expect("operation should succeed"); // x5 = 0x0200C000
    vm.bus
        .write_word(base + 4, addi(5, 5, -8))
        .expect("operation should succeed"); // x5 = 0x0200BFF8
    vm.bus
        .write_word(base + 8, lw(1, 5, 0))
        .expect("operation should succeed"); // x1 = mtime[31:0]
    vm.bus
        .write_word(base + 12, lw(2, 5, 4))
        .expect("operation should succeed"); // x2 = mtime[63:32]
    vm.bus
        .write_word(base + 16, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    // mtime ticks before each instruction (3 ticks before LW reads it)
    // Low word: 0x100 + 3 = 0x103 (LUI, ADDI, then LW reads on tick 3)
    // High word: 0x42 (no carry from low word incrementing by 3)
    assert_eq!(vm.cpu.x[2], 0x0000_0042, "mtime high word (no carry)");
    assert!(
        vm.cpu.x[1] >= 0x100,
        "mtime low word should be >= initial value"
    );
}

/// Test CLINT MMIO write: guest code writes mtimecmp to clear timer interrupt.
#[test]
fn test_clint_mmio_write_mtimecmp() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // x5 = 0x02004000 (mtimecmp address) -- LUI alone can load page-aligned address
    vm.bus
        .write_word(base, lui(5, 0x02004000))
        .expect("operation should succeed"); // x5 = 0x02004000
    vm.bus
        .write_word(base + 4, addi(1, 0, 0x100))
        .expect("operation should succeed"); // x1 = 0x100
    vm.bus
        .write_word(base + 8, sw(1, 5, 0))
        .expect("operation should succeed"); // mtimecmp[31:0] = 0x100
    vm.bus
        .write_word(base + 12, addi(2, 0, 0))
        .expect("operation should succeed"); // x2 = 0
    vm.bus
        .write_word(base + 16, sw(2, 5, 4))
        .expect("operation should succeed"); // mtimecmp[63:32] = 0
    vm.bus
        .write_word(base + 20, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    assert_eq!(vm.bus.clint.mtimecmp, 0x100u64, "mtimecmp should be 0x100");
}

/// Test CLINT msip via MMIO write: guest triggers software interrupt.
#[test]
fn test_clint_mmio_write_msip() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // x5 = 0x02000000 (msip address) -- LUI can load this page-aligned address directly
    vm.bus
        .write_word(base, lui(5, 0x02000000))
        .expect("operation should succeed"); // x5 = 0x02000000
    vm.bus
        .write_word(base + 4, addi(1, 0, 1))
        .expect("operation should succeed"); // x1 = 1
    vm.bus
        .write_word(base + 8, sw(1, 5, 0))
        .expect("operation should succeed"); // msip = 1
    vm.bus
        .write_word(base + 12, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    assert_eq!(vm.bus.clint.msip, 1, "msip should be 1");
    assert!(vm.bus.clint.software_pending());
}

/// Test timer interrupt clears when mtimecmp is set beyond mtime.
#[test]
fn test_clint_timer_clears_after_mtimecmp_update() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus
        .write_word(base, nop())
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 4, addi(1, 0, 42))
        .expect("operation should succeed"); // x1 = 42 after interrupt clears
    vm.bus
        .write_word(base + 8, ebreak())
        .expect("operation should succeed");

    // Handler at +0x200: clear timer by setting mtimecmp far ahead, then MRET
    // x5 = mtimecmp address (0x02004000)
    vm.bus
        .write_word(base + 0x200, lui(5, 0x02004000))
        .expect("operation should succeed"); // x5 = 0x02004000
    vm.bus
        .write_word(base + 0x204, lui(6, 0xFFFFF000))
        .expect("operation should succeed"); // x6 = 0xFFFFF000
    vm.bus
        .write_word(base + 0x208, ori(6, 6, 0xFFF))
        .expect("operation should succeed"); // x6 = 0xFFFFFFFF
    vm.bus
        .write_word(base + 0x20C, sw(6, 5, 0))
        .expect("operation should succeed"); // mtimecmp low = 0xFFFFFFFF
    vm.bus
        .write_word(base + 0x210, sw(6, 5, 4))
        .expect("operation should succeed"); // mtimecmp high = 0xFFFFFFFF
    vm.bus
        .write_word(base + 0x214, mret())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    vm.bus.clint.mtime = 0;
    vm.bus.clint.mtimecmp = 2;

    vm.cpu.csr.mie = 1 << 7; // MTIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Run: timer fires, handler clears it, returns, executes normally
    run_vm(&mut vm, 50);

    // After MRET, mepc points to the NOP (base), so PC continues to base+4 (addi x1, 0, 42)
    // then ebreak. x1 should be 42.
    assert_eq!(
        vm.cpu.x[1], 42,
        "after timer handler returns, should execute normally"
    );
}

/// Test that mtime advances on each RiscvVm::step() call.
#[test]
fn test_clint_mtime_advances_per_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus
        .write_word(base, nop())
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 4, nop())
        .expect("operation should succeed");
    vm.bus
        .write_word(base + 8, ebreak())
        .expect("operation should succeed");

    vm.cpu.pc = base as u32;

    assert_eq!(vm.bus.clint.mtime, 0);
    vm.step(); // tick 1, execute NOP
    assert_eq!(vm.bus.clint.mtime, 1);
    vm.step(); // tick 2, execute NOP
    assert_eq!(vm.bus.clint.mtime, 2);
    vm.step(); // tick 3, execute EBREAK -> stops
    assert_eq!(vm.bus.clint.mtime, 3);
}

// =====================================================================
