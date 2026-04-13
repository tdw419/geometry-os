// tests/riscv_tests.rs -- Phase 34: RV32I instruction integration tests
//
// Tests every base RV32I instruction by running hand-encoded instruction
// sequences through the RiscvVm.

use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;

// ---- Helpers ----

fn test_vm(instrs: &[u32]) -> RiscvVm {
    let mut vm = RiscvVm::new(4096);
    let ram_base = 0x8000_0000u64;
    for (i, &word) in instrs.iter().enumerate() {
        let _ = vm.mem.write_word(ram_base + (i as u64) * 4, word);
    }
    vm.cpu.pc = ram_base as u32;
    vm
}

fn run(vm: &mut RiscvVm, max_steps: usize) {
    for _ in 0..max_steps {
        match vm.cpu.step(&mut vm.mem) {
            StepResult::Ecall | StepResult::Ebreak | StepResult::FetchFault => break,
            StepResult::Ok | StepResult::LoadFault | StepResult::StoreFault => {}
        }
    }
}

// ---- Encoding helpers ----

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((rd as u32) << 7)
        | opcode
}

fn i_type(imm: u32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm & 0xFFF) << 20) | ((rs1 as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | opcode
}

fn u_type(imm: u32, rd: u8, opcode: u32) -> u32 {
    (imm & 0xFFFF_F000) | ((rd as u32) << 7) | opcode
}

fn jal(rd: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    let bit20 = (imm >> 20) & 1;
    let bits10_1 = (imm >> 1) & 0x3FF;
    let bit11 = (imm >> 11) & 1;
    let bits19_12 = (imm >> 12) & 0xFF;
    let encoded = (bit20 << 31) | (bits10_1 << 21) | (bit11 << 20) | (bits19_12 << 12);
    encoded | ((rd as u32) << 7) | 0x6F
}

fn jalr(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm as u32, rs1, 0, rd, 0x67)
}

fn b_type(rs1: u8, rs2: u8, funct3: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    let imm12 = (imm >> 12) & 1;
    let imm10_5 = (imm >> 5) & 0x3F;
    let imm4_1 = (imm >> 1) & 0xF;
    let imm11 = (imm >> 11) & 1;
    (imm12 << 31)
        | (imm10_5 << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | (imm4_1 << 8)
        | (imm11 << 7)
        | 0x63
}

fn s_type(rs2: u8, rs1: u8, funct3: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    let imm4_0 = imm & 0x1F;
    let imm11_5 = (imm >> 5) & 0x7F;
    (imm11_5 << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | (imm4_0 << 7)
        | 0x23
}

// Instruction shorthand
fn ecall() -> u32 { i_type(0, 0, 0, 0, 0x73) }
fn lui(rd: u8, imm: u32) -> u32 { u_type(imm, rd, 0x37) }
fn auipc(rd: u8, imm: u32) -> u32 { u_type(imm, rd, 0x17) }
fn add(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b000, rd, 0x33) }
fn sub(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0b0100000, rs2, rs1, 0b000, rd, 0x33) }
fn sll(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b001, rd, 0x33) }
fn slt(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b010, rd, 0x33) }
fn sltu(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b011, rd, 0x33) }
fn xor_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b100, rd, 0x33) }
fn srl(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b101, rd, 0x33) }
fn sra(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0b0100000, rs2, rs1, 0b101, rd, 0x33) }
fn or_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b110, rd, 0x33) }
fn and_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b111, rd, 0x33) }
fn addi(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b000, rd, 0x13) }
fn slti(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b010, rd, 0x13) }
fn sltiu(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b011, rd, 0x13) }
fn xori(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b100, rd, 0x13) }
fn ori(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b110, rd, 0x13) }
fn andi(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b111, rd, 0x13) }
fn slli(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(shamt & 0x1F, rs1, 0b001, rd, 0x13) }
fn srli(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(shamt & 0x1F, rs1, 0b101, rd, 0x13) }
fn srai(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(0b0100000 << 5 | (shamt & 0x1F), rs1, 0b101, rd, 0x13) }
fn lw(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b010, rd, 0x03) }
fn lb(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b000, rd, 0x03) }
fn lh(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b001, rd, 0x03) }
fn lbu(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b100, rd, 0x03) }
fn lhu(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b101, rd, 0x03) }
fn sw(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b010, off) }
fn sb(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b000, off) }
fn sh(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b001, off) }
fn beq(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b000, off) }
fn bne(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b001, off) }
fn blt(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b100, off) }
fn bge(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b101, off) }
fn bltu(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b110, off) }
fn bgeu(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b111, off) }

// ============================================================
// R-type ALU
// ============================================================

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
    let mut vm = test_vm(&[addi(1, 0, 0xF0), addi(2, 0, 0x0F), or_inst(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFF);
}

#[test]
fn test_rv32_and() {
    let mut vm = test_vm(&[addi(1, 0, 0xFF), addi(2, 0, 0x0F), and_inst(3, 1, 2), ecall()]);
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
        beq(1, 2, 8),   // taken
        addi(3, 0, 0),  // skipped
        addi(3, 0, 0),  // skipped
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
        blt(1, 2, 8),   // -5 < 3 signed -> taken
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
        bge(1, 2, 8),   // -1 >= 1? No -> not taken
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
        bgeu(1, 2, 8),  // 0xFFFFFFFF >= 1 unsigned -> taken
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
        bltu(1, 2, 8),  // 1 < 0xFFFFFFFF unsigned -> taken
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
        lb(3, 2, 0),      // sign-extended -> 0xFFFFFFFE
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
        sh(1, 2, 0),    // store 0xFFFF
        lh(3, 2, 0),    // sign-extended -> 0xFFFFFFFF
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
        lhu(3, 2, 0),   // unsigned -> 0x0000FFFF
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
        lbu(3, 2, 0),   // unsigned -> 0xFE
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

// ============================================================
// Multi-instruction programs
// ============================================================

#[test]
fn test_rv32_fibonacci() {
    // Compute fib(10) = 55
    // x1=a=0, x2=b=1, x3=counter=10
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // a = 0
        addi(2, 0, 1),  // b = 1
        addi(3, 0, 10), // counter = 10
        add(4, 1, 2),   // temp = a + b        [inst 3]
        addi(1, 2, 0),  // a = b               [inst 4]
        addi(2, 4, 0),  // b = temp            [inst 5]
        addi(3, 3, -1), // counter--           [inst 6]
        bne(3, 0, -16), // if counter!=0 goto 3 [inst 7] -4 instr * 4 bytes
        ecall(),
    ]);
    run(&mut vm, 200);
    assert_eq!(vm.cpu.x[1], 55, "fib(10) should be 55 (x1=a)");
}

#[test]
fn test_rv32_add_overflow() {
    let mut vm = test_vm(&[
        lui(1, 0x80000000),
        lui(2, 0x80000000),
        add(3, 1, 2), // wrapping: 0x80000000 + 0x80000000 = 0
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0);
}

#[test]
fn test_rv32_sub_underflow() {
    let mut vm = test_vm(&[addi(1, 0, 0), addi(2, 0, 1), sub(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFFFFFFFF);
}

#[test]
fn test_rv32_loop_count() {
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // counter = 0
        addi(2, 0, 1),  // increment
        addi(3, 0, 10), // limit
        add(1, 1, 2),   // counter++        [inst 3]
        bne(1, 3, -8),  // if counter!=10 goto 3 [inst 4]
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 10);
}

#[test]
fn test_rv32_jal_function_call() {
    // Call a function that doubles x1
    let mut vm = test_vm(&[
        addi(1, 0, 21), // x1 = 21
        jal(5, 12),     // call function (skip 3 instr), x5 = 0x80000008
        ecall(),        // inst 2: return here, x1 should be 42
        addi(0, 0, 0), // inst 3: padding (NOP)
        add(1, 1, 1),  // inst 4: double x1
        jalr(0, 5, 0), // inst 5: return to x5
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 42);
}

#[test]
fn test_rv32_sum_1_to_10() {
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // sum = 0
        addi(2, 0, 1),  // i = 1
        addi(3, 0, 11), // limit = 11
        add(1, 1, 2),   // sum += i          [inst 3]
        addi(2, 2, 1),  // i++               [inst 4]
        bne(2, 3, -12), // if i!=11 goto 3   [inst 5]
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 55, "sum 1..10 should be 55");
}

#[test]
fn test_rv32_memory_roundtrip() {
    let mut vm = test_vm(&[
        auipc(5, 0x0),
        addi(5, 5, 200), // base = 0x800000C8
        lui(1, 0xDEADB000),
        ori(1, 1, 0x0EF), // x1 = 0xDEADB0EF (low 12 bits must have bit 11 = 0)
        sw(1, 5, 0),
        addi(2, 0, 0xCA),
        sh(2, 5, 4),     // store half at offset 4
        addi(3, 0, 0x42),
        sb(3, 5, 6),     // store byte at offset 6
        lw(10, 5, 0),    // x10 = 0xDEADBEEF
        lhu(11, 5, 4),   // x11 = 0x00CA
        lbu(12, 5, 6),   // x12 = 0x42
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[10], 0xDEADB0EF);
    assert_eq!(vm.cpu.x[11], 0x00CA);
    assert_eq!(vm.cpu.x[12], 0x42);
}
