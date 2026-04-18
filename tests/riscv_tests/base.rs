use super::*;

#[test]
fn test_rv32_add() {
    let mut vm = test_vm(&[addi(1, 0, 10), addi(2, 0, 20), add(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 30);
}

#[test]
fn test_rv32_sub() {
    let mut vm = test_vm(&[addi(1, 0, 30), addi(2, 0, 10), sub(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 20);
}

#[test]
fn test_rv32_sll() {
    let mut vm = test_vm(&[addi(1, 0, 1), addi(2, 0, 5), sll(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 32);
}

#[test]
fn test_rv32_slt_less() {
    let mut vm = test_vm(&[addi(1, 0, 5), addi(2, 0, 10), slt(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 1);
}

#[test]
fn test_rv32_slt_not_less() {
    let mut vm = test_vm(&[addi(1, 0, 10), addi(2, 0, 5), slt(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0);
}

#[test]
fn test_rv32_slt_signed_negative() {
    let mut vm = test_vm(&[
        addi(1, 0, -5i32 as i32),
        addi(2, 0, 3),
        slt(3, 1, 2),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 1);
}

#[test]
fn test_rv32_sltu() {
    let mut vm = test_vm(&[
        addi(1, 0, -1i32 as i32),
        addi(2, 0, 1),
        sltu(3, 2, 1), // 1 < 0xFFFFFFFF unsigned -> 1
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 1);
}

#[test]
fn test_rv32_xor() {
    let mut vm = test_vm(&[
        lui(1, 0xFF000000),
        addi(2, 0, 0x0F),
        xor_inst(3, 1, 2),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFF00000F);
}

#[test]
fn test_rv32_srl() {
    let mut vm = test_vm(&[
        lui(1, 0x80000000), // bit 31 set
        addi(2, 0, 4),
        srl(3, 1, 2), // logical shift -> 0x08000000
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0x08000000);
}

#[test]
fn test_rv32_sra() {
    let mut vm = test_vm(&[
        lui(1, 0x80000000),
        addi(2, 0, 4),
        sra(3, 1, 2), // arithmetic -> 0xF8000000
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xF8000000);
}

#[test]
fn test_rv32_or() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xF0),
        addi(2, 0, 0x0F),
        or_inst(3, 1, 2),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFF);
}

#[test]
fn test_rv32_and() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xFF),
        addi(2, 0, 0x0F),
        and_inst(3, 1, 2),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0x0F);
}

#[test]
fn test_rv32_x0_always_zero() {
    let mut vm = test_vm(&[addi(1, 0, 42), add(0, 1, 1), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[0], 0);
}

// ============================================================
// I-type ALU
// ============================================================

#[test]
fn test_rv32_addi_acceptance() {
    // Acceptance: x1 = x2 + 100
    let mut vm = test_vm(&[addi(2, 0, 50), addi(1, 2, 100), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 50);
    assert_eq!(vm.cpu.x[1], 150); // x1 = x2 + 100
}

#[test]
fn test_rv32_addi() {
    let mut vm = test_vm(&[addi(1, 0, 100), addi(2, 1, 50), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 150);
}

#[test]
fn test_rv32_addi_negative() {
    let mut vm = test_vm(&[addi(1, 0, 10), addi(2, 1, -5), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 5);
}

#[test]
fn test_rv32_slti() {
    let mut vm = test_vm(&[addi(1, 0, 5), slti(2, 1, 10), slti(3, 1, 3), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 1);
    assert_eq!(vm.cpu.x[3], 0);
}

#[test]
fn test_rv32_sltiu() {
    let mut vm = test_vm(&[sltiu(2, 0, -1i32 as i32), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 1);
}

#[test]
fn test_rv32_xori() {
    let mut vm = test_vm(&[addi(1, 0, 0xFF), xori(2, 1, 0x0F), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0xF0);
}

#[test]
fn test_rv32_ori() {
    let mut vm = test_vm(&[addi(1, 0, 0xF0), ori(2, 1, 0x0F), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0xFF);
}

#[test]
fn test_rv32_andi() {
    let mut vm = test_vm(&[addi(1, 0, 0xFF), andi(2, 1, 0x0F), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0x0F);
}

#[test]
fn test_rv32_slli() {
    let mut vm = test_vm(&[addi(1, 0, 1), slli(2, 1, 8), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 256);
}

#[test]
fn test_rv32_srli() {
    let mut vm = test_vm(&[lui(1, 0x80000000), srli(2, 1, 4), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0x08000000);
}

#[test]
fn test_rv32_srai() {
    let mut vm = test_vm(&[lui(1, 0x80000000), srai(2, 1, 4), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 0xF8000000);
}

// ============================================================
// Upper immediate
// ============================================================

#[test]
fn test_rv32_lui() {
    let mut vm = test_vm(&[lui(1, 0x12345000), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x12345000);
}

#[test]
fn test_rv32_auipc() {
    let mut vm = test_vm(&[auipc(1, 0x1000), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x80001000);
}

// ============================================================
// Jumps
// ============================================================

#[test]
fn test_rv32_jal() {
    let mut vm = test_vm(&[
        jal(1, 8),      // jump to PC+8, x1 = PC+4
        addi(2, 0, 0),  // skipped
        addi(3, 0, 0),  // skipped
        addi(4, 0, 42), // executed
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x80000004);
    assert_eq!(vm.cpu.x[4], 42);
    assert_eq!(vm.cpu.x[2], 0);
}

#[test]
fn test_rv32_jalr() {
    let mut vm = test_vm(&[
        auipc(5, 0x0),
        addi(5, 5, 12), // x5 = 0x8000000C (addr of 4th instr)
        jalr(1, 5, 0),
        addi(2, 0, 0),  // skipped
        addi(3, 0, 0),  // skipped
        addi(4, 0, 99), // executed
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 0x8000000C);
    assert_eq!(vm.cpu.x[4], 99);
}

#[test]
fn test_rv32_jalr_clears_lsb() {
    let mut vm = test_vm(&[
        auipc(5, 0x0),
        addi(5, 5, 16), // x5 = 0x80000010
        ori(5, 5, 1),   // x5 = 0x80000011 (LSB set)
        jalr(0, 5, 0),  // jump to 0x80000008 (LSB cleared)
        addi(1, 0, 0),  // skipped
        addi(2, 0, 42), // executed
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 42);
}

// ============================================================
// Branches
// ============================================================

#[test]
fn test_rv32_beq_taken() {
    let mut vm = test_vm(&[
        addi(1, 0, 5),
        addi(2, 0, 5),
        beq(1, 2, 8),  // taken
        addi(3, 0, 0), // skipped
        addi(3, 0, 0), // skipped
        addi(4, 0, 42),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[4], 42);
}

#[test]
fn test_rv32_beq_not_taken() {
    let mut vm = test_vm(&[
        addi(1, 0, 5),
        addi(2, 0, 10),
        beq(1, 2, 8),   // not taken
        addi(3, 0, 42), // executed
        addi(4, 0, 99),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 42);
}

#[test]
fn test_rv32_bne() {
    let mut vm = test_vm(&[
        addi(1, 0, 5),
        addi(2, 0, 10),
        bne(1, 2, 8),
        addi(3, 0, 0),
        addi(3, 0, 0),
        addi(4, 0, 42),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[4], 42);
}

#[test]
fn test_rv32_blt_signed() {
    let mut vm = test_vm(&[
        addi(1, 0, -5i32 as i32),
        addi(2, 0, 3),
        blt(1, 2, 8), // -5 < 3 signed -> taken
        addi(3, 0, 0),
        addi(3, 0, 0),
        addi(4, 0, 1),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[4], 1);
}

#[test]
fn test_rv32_bge_not_taken_signed() {
    let mut vm = test_vm(&[
        addi(1, 0, -1i32 as i32),
        addi(2, 0, 1),
        bge(1, 2, 8), // -1 >= 1? No -> not taken
        addi(3, 0, 42),
        addi(4, 0, 0),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 42);
}

#[test]
fn test_rv32_bgeu() {
    let mut vm = test_vm(&[
        addi(1, 0, -1i32 as i32),
        addi(2, 0, 1),
        bgeu(1, 2, 8), // 0xFFFFFFFF >= 1 unsigned -> taken
        addi(3, 0, 0),
        addi(3, 0, 0),
        addi(4, 0, 42),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[4], 42);
}

#[test]
fn test_rv32_bltu() {
    let mut vm = test_vm(&[
        addi(1, 0, 1),
        addi(2, 0, -1i32 as i32),
        bltu(1, 2, 8), // 1 < 0xFFFFFFFF unsigned -> taken
        addi(3, 0, 0),
        addi(3, 0, 0),
        addi(4, 0, 42),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[4], 42);
}

// ============================================================
// Loads and Stores
// ============================================================

#[test]
fn test_rv32_sw_lw() {
    let mut vm = test_vm(&[
        addi(1, 0, 42),
        auipc(2, 0x0),
        addi(2, 2, 100),
        sw(1, 2, 0),
        lw(3, 2, 0),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 42);
}

#[test]
fn test_rv32_sb_lb_sign_extend() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xFE), // 254 = -2 signed byte
        auipc(2, 0x0),
        addi(2, 2, 100),
        sb(1, 2, 0),
        lb(3, 2, 0), // sign-extended -> 0xFFFFFFFE
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFFFFFFFE);
}

#[test]
fn test_rv32_sb_lb_positive() {
    let mut vm = test_vm(&[
        addi(1, 0, 42),
        auipc(2, 0x0),
        addi(2, 2, 100),
        sb(1, 2, 0),
        lb(3, 2, 0),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 42);
}

#[test]
fn test_rv32_sh_lh_sign_extend() {
    let mut vm = test_vm(&[
        addi(1, 0, -1), // 0xFFFFFFFF
        auipc(2, 0x0),
        addi(2, 2, 100),
        sh(1, 2, 0), // store 0xFFFF
        lh(3, 2, 0), // sign-extended -> 0xFFFFFFFF
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFFFFFFFF);
}

#[test]
fn test_rv32_lhu() {
    let mut vm = test_vm(&[
        addi(1, 0, -1),
        auipc(2, 0x0),
        addi(2, 2, 100),
        sh(1, 2, 0),
        lhu(3, 2, 0), // unsigned -> 0x0000FFFF
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0x0000FFFF);
}

#[test]
fn test_rv32_lbu() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xFE),
        auipc(2, 0x0),
        addi(2, 2, 100),
        sb(1, 2, 0),
        lbu(3, 2, 0), // unsigned -> 0xFE
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFE);
}

#[test]
fn test_rv32_store_load_with_offset() {
    let mut vm = test_vm(&[
        addi(1, 0, 42), // 42 fits in 12-bit imm
        auipc(2, 0x0),
        addi(2, 2, 100),
        sw(1, 2, 8),
        lw(3, 2, 8),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 42);
}

// ============================================================
// System
// ============================================================

#[test]
fn test_rv32_ecall_stops() {
    let mut vm = test_vm(&[addi(1, 0, 42), ecall(), addi(1, 0, 99)]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 42);
}

#[test]
fn test_rv32_ebreak_stops() {
    let mut vm = test_vm(&[
        addi(1, 0, 42),
        i_type(1, 0, 0, 0, 0x73), // EBREAK: funct12=1
        addi(1, 0, 99),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 42);
}

#[test]
fn test_rv32_fence_is_nop() {
    let mut vm = test_vm(&[
        addi(1, 0, 10),
        i_type(0, 0, 0, 0, 0x0F), // FENCE
        addi(2, 1, 20),
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[2], 30);
}

#[test]
fn test_rv32_x0_load() {
    let mut vm = test_vm(&[
        addi(1, 0, 42),
        auipc(2, 0x0),
        addi(2, 2, 100),
        sw(1, 2, 0),
        lw(0, 2, 0), // load into x0 -> no effect
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[0], 0);
}
