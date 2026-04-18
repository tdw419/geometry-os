use super::sv32::{make_pte, make_satp};
use super::*;
use geometry_os::riscv::csr;
use geometry_os::riscv::mmu;

// ============================================================
// Phase 36: Page Fault Traps
// ============================================================

#[test]
fn test_page_fault_load_sets_mcause_mtval() {
    // Fetch from address outside bus range -> fetch access fault trap.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 4096);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    cpu.pc = 0xDEAD_0000;
    cpu.csr.mtvec = 0x8000_0000;

    let result = cpu.step(&mut bus);
    assert_eq!(
        result,
        StepResult::Ok,
        "fetch fault should return Ok after trap delivery"
    );
    assert_eq!(cpu.pc, 0x8000_0000, "should jump to mtvec");
    assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_ACCESS);
    assert_eq!(cpu.csr.mepc, 0xDEAD_0000);
    assert_eq!(cpu.csr.mtval, 0xDEAD_0000);
}

#[test]
fn test_page_fault_fetch_no_exec_permission() {
    // Fetch from page without X permission -> instruction page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    // Root[0] -> L2 table
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    // L2[0] -> code page (RX)
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    // L2[1] -> data page (RW, no X)
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W),
    )
    .expect("operation should succeed");

    // Code at VA 0x0: LUI x1, 0x1 -> x1 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (1u32 << 7) | 0x37)
        .expect("operation should succeed");
    // JALR x0, x1, 0 -> jump to 0x1000
    bus.write_word(
        4,
        (0u32 << 20) | (1u32 << 15) | (0b000 << 12) | (0u32 << 7) | 0x67,
    )
    .expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    // Execute LUI
    cpu.step(&mut bus);
    assert_eq!(cpu.x[1], 0x1000);
    // Execute JALR -> sets PC to 0x1000
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x1000);

    // Next fetch from 0x1000 -> instruction page fault (no X permission)
    cpu.step(&mut bus);
    assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 0x1000, "mepc should be faulting PC");
}

#[test]
fn test_page_fault_load_no_read_permission() {
    // Load from page without R permission -> load page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    // L2[1] -> write-only page (no R)
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(3, mmu::PTE_V | mmu::PTE_W),
    )
    .expect("operation should succeed");

    // LUI x10, 0x1 -> x10 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed");
    // LW x5, 0(x10) -> load from 0x1000
    bus.write_word(
        4,
        (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03,
    )
    .expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // LW -> load page fault

    assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 4, "mepc should be PC of the LW instruction");
}

#[test]
fn test_page_fault_store_no_write_permission() {
    // Store to page without W permission -> store page fault.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    // L2[1] -> read-only page (no W)
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(3, mmu::PTE_V | mmu::PTE_R),
    )
    .expect("operation should succeed");

    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed"); // LUI x10, 0x1
    bus.write_word(4, addi(5, 0, 42))
        .expect("operation should succeed"); // ADDI x5, x0, 42
    bus.write_word(8, sw(5, 10, 0))
        .expect("operation should succeed"); // SW x5, 0(x10) -> store to 0x1000

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // ADDI
    assert_eq!(cpu.x[5], 42);
    cpu.step(&mut bus); // SW -> store page fault

    assert_eq!(cpu.csr.mcause, csr::CAUSE_STORE_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000, "mtval should be faulting VA");
    assert_eq!(cpu.csr.mepc, 8, "mepc should be PC of the SW instruction");
}

#[test]
fn test_page_fault_delegated_to_s_mode() {
    // Load page fault from U-mode delegated to S-mode via medeleg.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    // L2[0] -> code page (RXU)
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X | mmu::PTE_U),
    )
    .expect("operation should succeed");
    // L2[1] -> not mapped

    // Delegate load page fault (cause 13) to S-mode
    cpu.csr.medeleg = 1 << csr::CAUSE_LOAD_PAGE_FAULT;

    // LUI x10, 0x1
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed");
    // LW x5, 0(x10) -> unmapped
    bus.write_word(
        4,
        (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03,
    )
    .expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0300;
    cpu.csr.stvec = 0x8000_0400;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // LW -> load page fault, delegated to S-mode

    assert_eq!(cpu.csr.scause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.stval, 0x1000, "stval should be faulting VA");
    assert_eq!(cpu.csr.sepc, 4, "sepc should be PC of the LW instruction");
    assert_eq!(cpu.pc, 0x8000_0400, "should jump to stvec");
    assert_eq!(
        cpu.privilege,
        geometry_os::riscv::cpu::Privilege::Supervisor
    );
}

#[test]
fn test_page_fault_stval_for_s_mode_trap() {
    // Store page fault delegated to S-mode sets stval, not mtval.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X | mmu::PTE_U),
    )
    .expect("operation should succeed");

    cpu.csr.medeleg = 1 << csr::CAUSE_STORE_PAGE_FAULT;

    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed"); // LUI x10, 0x1
    bus.write_word(4, addi(5, 0, 99))
        .expect("operation should succeed"); // ADDI x5, x0, 99
    bus.write_word(8, sw(5, 10, 0))
        .expect("operation should succeed"); // SW x5, 0(x10)

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0300;
    cpu.csr.stvec = 0x8000_0400;

    cpu.step(&mut bus); // LUI
    cpu.step(&mut bus); // ADDI
    cpu.step(&mut bus); // SW -> store page fault, delegated to S-mode

    assert_eq!(cpu.csr.scause, csr::CAUSE_STORE_PAGE_FAULT);
    assert_eq!(cpu.csr.stval, 0x1000, "stval should be faulting VA");
    assert_eq!(cpu.csr.sepc, 8);
    assert_eq!(cpu.csr.mtval, 0, "mtval should be 0 (trap went to S-mode)");
    assert_eq!(
        cpu.privilege,
        geometry_os::riscv::cpu::Privilege::Supervisor
    );
}

#[test]
fn test_page_fault_mret_recovery() {
    // Load page fault -> trap to M -> fix page table -> MRET -> retry succeeds.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x4_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");
    // L2[1] initially unmapped

    // Put expected data at data page
    bus.write_word((data_ppn as u64) << 12, 0xFEED_FACE)
        .expect("operation should succeed");

    // LUI x10, 0x1
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
        .expect("operation should succeed");
    // LW x5, 0(x10)
    bus.write_word(
        4,
        (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03,
    )
    .expect("operation should succeed");
    // EBREAK
    bus.write_word(8, ebreak())
        .expect("operation should succeed");

    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    cpu.csr.mtvec = 0x8000_0200;

    // LUI
    cpu.step(&mut bus);
    assert_eq!(cpu.x[10], 0x1000);
    // LW -> page fault
    cpu.step(&mut bus);
    assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
    assert_eq!(cpu.csr.mtval, 0x1000);
    assert_eq!(cpu.csr.mepc, 4);

    // Fix page table externally
    bus.write_word(
        ((l2_ppn as u64) << 12) | 4,
        make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W),
    )
    .expect("operation should succeed");
    cpu.tlb.flush_all();

    // MRET (simulate)
    let restored = cpu
        .csr
        .trap_return(geometry_os::riscv::cpu::Privilege::Machine);
    cpu.pc = cpu.csr.mepc;
    cpu.privilege = restored;

    assert_eq!(cpu.pc, 4);

    // Retry LW -> succeeds now
    cpu.step(&mut bus);
    assert_eq!(
        cpu.x[5], 0xFEED_FACE,
        "retry should succeed after page table fix"
    );
    assert_eq!(cpu.pc, 8);
}

#[test]
fn test_page_fault_unmapped_va_all_three_types() {
    // Verify cause codes for all three page fault types with unmapped PTEs.
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;

    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V))
        .expect("operation should succeed");
    bus.write_word(
        (l2_ppn as u64) << 12,
        make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X),
    )
    .expect("operation should succeed");

    // Fetch page fault: PC = 0x1000 (VPN 1, unmapped)
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        cpu.pc = 0x1000;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.step(&mut bus);
        assert_eq!(cpu.csr.mcause, csr::CAUSE_FETCH_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }

    // Load page fault: LW from 0x1000
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        // LUI x10, 0x1; LW x5, 0(x10)
        bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
            .expect("operation should succeed");
        bus.write_word(
            4,
            (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03,
        )
        .expect("operation should succeed");
        cpu.pc = 0;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.tlb.flush_all();
        cpu.step(&mut bus); // LUI
        cpu.step(&mut bus); // LW -> fault
        assert_eq!(cpu.csr.mcause, csr::CAUSE_LOAD_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }

    // Store page fault: SW to 0x1000
    {
        let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
        bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37)
            .expect("operation should succeed");
        bus.write_word(4, sw(5, 10, 0))
            .expect("operation should succeed");
        cpu.x[5] = 42;
        cpu.pc = 0;
        cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
        cpu.csr.satp = make_satp(1, 0, root_ppn);
        cpu.csr.mtvec = 0x8000_0200;
        cpu.tlb.flush_all();
        cpu.step(&mut bus); // LUI
        cpu.step(&mut bus); // SW -> fault
        assert_eq!(cpu.csr.mcause, csr::CAUSE_STORE_PAGE_FAULT);
        assert_eq!(cpu.csr.mtval, 0x1000);
    }
}
