use super::*;

// ---- CSR execution tests (Phase 35) ----

#[test]
fn test_rv32_csrrw_mstatus() {
    let mut vm = test_vm(&[addi(1, 0, 0xAB), csrrw(2, 1, CSR_MSTATUS), ebreak()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0, "old mstatus should be 0");
    assert_eq!(vm.cpu.csr.mstatus, 0xAB, "mstatus should be 0xAB");
}

#[test]
fn test_rv32_csrrw_swap() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xFF),
        csrrw(2, 1, CSR_MSTATUS),
        addi(3, 0, 0x42),
        csrrw(4, 3, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0);
    assert_eq!(vm.cpu.x[4], 0xFF);
    assert_eq!(vm.cpu.csr.mstatus, 0x42);
}

#[test]
fn test_rv32_csrrw_rd_zero() {
    let mut vm = test_vm(&[addi(1, 0, 0x77), csrrw(0, 1, CSR_MCAUSE), ebreak()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.csr.mcause, 0x77);
    assert_eq!(vm.cpu.x[0], 0);
}

#[test]
fn test_rv32_csrrs_set_bits() {
    let mut vm = test_vm(&[
        addi(1, 0, 0x0F),
        csrrw(2, 1, CSR_MSTATUS),
        addi(3, 0, 0xF0),
        csrrs(4, 3, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0);
    assert_eq!(vm.cpu.x[4], 0x0F, "csrrs should return old value");
    assert_eq!(vm.cpu.csr.mstatus, 0xFF, "csrrs should set bits");
}

#[test]
fn test_rv32_csrrs_rs1_zero_no_write() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xAB),
        csrrw(0, 1, CSR_MSTATUS),
        csrrs(2, 0, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0xAB, "csrrs with rs1=0 should read");
    assert_eq!(
        vm.cpu.csr.mstatus, 0xAB,
        "csrrs with rs1=0 should not write"
    );
}

#[test]
fn test_rv32_csrrc_clear_bits() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xFF),
        csrrw(0, 1, CSR_MSTATUS),
        addi(2, 0, 0x0F),
        csrrc(3, 2, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFF, "csrrc should return old value");
    assert_eq!(vm.cpu.csr.mstatus, 0xF0, "csrrc should clear bits");
}

#[test]
fn test_rv32_csrrc_rs1_zero_no_write() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xAB),
        csrrw(0, 1, CSR_MSTATUS),
        csrrc(2, 0, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0xAB);
    assert_eq!(vm.cpu.csr.mstatus, 0xAB);
}

#[test]
fn test_rv32_csrrwi() {
    let mut vm = test_vm(&[csrrwi(1, 5, CSR_MCAUSE), ebreak()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0, "old mcause should be 0");
    assert_eq!(vm.cpu.csr.mcause, 5, "mcause should be 5");
}

#[test]
fn test_rv32_csrrsi() {
    let mut vm = test_vm(&[
        csrrwi(0, 0x03, CSR_MSTATUS),
        csrrsi(1, 0x0C, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 3, "csrrsi should return old value");
    assert_eq!(vm.cpu.csr.mstatus, 0x0F, "csrrsi should set bits");
}

#[test]
fn test_rv32_csrrsi_uimm_zero_no_write() {
    let mut vm = test_vm(&[
        csrrwi(0, 0x1F, CSR_MSTATUS),
        csrrsi(1, 0, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x1F);
    assert_eq!(vm.cpu.csr.mstatus, 0x1F);
}

#[test]
fn test_rv32_csrrci() {
    let mut vm = test_vm(&[
        csrrwi(0, 0x1F, CSR_MSTATUS),
        csrrci(1, 0x0C, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x1F, "csrrci should return old value");
    assert_eq!(vm.cpu.csr.mstatus, 0x13, "csrrci should clear bits");
}

#[test]
fn test_rv32_csrrci_uimm_zero_no_write() {
    let mut vm = test_vm(&[
        csrrwi(0, 0x1F, CSR_MSTATUS),
        csrrci(1, 0, CSR_MSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x1F);
    assert_eq!(vm.cpu.csr.mstatus, 0x1F);
}

#[test]
fn test_rv32_csr_sstatus_view() {
    let mut vm = test_vm(&[
        addi(1, 0, -1),
        csrrw(2, 1, CSR_MSTATUS),
        csrrs(3, 0, CSR_SSTATUS),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(
        vm.cpu.x[3], 0xC0122,
        "sstatus should be masked view of mstatus"
    );
}

#[test]
fn test_rv32_csr_multiple_registers() {
    let mut vm = test_vm(&[
        addi(1, 0, 0x10),
        csrrw(0, 1, CSR_MTVEC),
        addi(2, 0, 0x20),
        csrrw(0, 2, CSR_MEPC),
        csrrs(3, 0, CSR_MTVEC),
        csrrs(4, 0, CSR_MEPC),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0x10, "mtvec should be 0x10");
    assert_eq!(vm.cpu.x[4], 0x20, "mepc should be 0x20");
}

#[test]
fn test_rv32_csr_mepc_alignment() {
    let mut vm = test_vm(&[
        addi(1, 0, 0x201),
        csrrw(0, 1, CSR_MEPC),
        csrrs(2, 0, CSR_MEPC),
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0x200, "mepc LSB should be cleared");
    assert_eq!(vm.cpu.csr.mepc, 0x200);
}

// ============================================================
// Phase 35: Privilege mode transitions
// ============================================================
