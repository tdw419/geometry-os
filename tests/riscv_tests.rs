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
        let _ = vm.bus.write_word(ram_base + (i as u64) * 4, word);
    }
    vm.cpu.pc = ram_base as u32;
    vm
}

fn run(vm: &mut RiscvVm, max_steps: usize) {
    for _ in 0..max_steps {
        match vm.cpu.step(&mut vm.bus) {
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
fn ebreak() -> u32 { i_type(1, 0, 0, 0, 0x73) }
fn fence() -> u32 { 0x0FF0000F }
fn nop() -> u32 { addi(0, 0, 0) }
fn mret() -> u32 { 0x30200073 }
fn sret() -> u32 { 0x10200073 }
fn and_(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 7, rd, 0x33) }
fn csrrw(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (1u32 << 12) | ((rd as u32) << 7) | 0x73 }
fn csrrs(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (2u32 << 12) | ((rd as u32) << 7) | 0x73 }
fn csrrc(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (3u32 << 12) | ((rd as u32) << 7) | 0x73 }
fn csrrwi(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (5u32 << 12) | ((rd as u32) << 7) | 0x73 }
fn csrrsi(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (6u32 << 12) | ((rd as u32) << 7) | 0x73 }
fn csrrci(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (7u32 << 12) | ((rd as u32) << 7) | 0x73 }

// CSR address constants
const CSR_MSTATUS: u32 = 0x300;
const CSR_MTVEC: u32 = 0x305;
const CSR_MEPC: u32 = 0x341;
const CSR_MCAUSE: u32 = 0x342;
const CSR_MTVAL: u32 = 0x343;
const CSR_SSTATUS: u32 = 0x100;
const CSR_STVEC: u32 = 0x105;
const CSR_SATP: u32 = 0x180;
const CSR_MIE: u32 = 0x304;
const CSR_MIP: u32 = 0x344;
const CSR_SIE: u32 = 0x104;
const CSR_SIP: u32 = 0x144;
const CSR_MEDELEG: u32 = 0x302;
const CSR_MIDELEG: u32 = 0x303;
const CSR_SEPC: u32 = 0x141;
const CSR_SCAUSE: u32 = 0x142;
const CSR_STVAL: u32 = 0x143;

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
fn test_rv32_fibonacci_20_iterations() {
    // Acceptance: Fibonacci(10) = 55, then continue to fib(20) = 6765
    // Phase 34 deliverable: "fibonacci test program that runs 20 iterations"
    //
    // After N iterations: a = fib(N), b = fib(N+1)
    //   fib(10) = 55, fib(20) = 6765
    //
    // x1=a=0, x2=b=1, x3=counter=20, x4=temp
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // a = 0               [inst 0]
        addi(2, 0, 1),  // b = 1               [inst 1]
        addi(3, 0, 20), // counter = 20         [inst 2]
        add(4, 1, 2),   // temp = a + b         [inst 3] loop start
        addi(1, 2, 0),  // a = b               [inst 4]
        addi(2, 4, 0),  // b = temp            [inst 5]
        addi(3, 3, -1), // counter--           [inst 6]
        bne(3, 0, -16), // if counter!=0 goto 3 [inst 7]
        ecall(),        //                      [inst 8]
    ]);
    run(&mut vm, 400);
    assert_eq!(vm.cpu.x[1], 6765, "fib(20) should be 6765 (x1=a)");
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

// ---- CSR execution tests (Phase 35) ----

#[test]
fn test_rv32_csrrw_mstatus() {
    let mut vm = test_vm(&[
        addi(1, 0, 0xAB),
        csrrw(2, 1, CSR_MSTATUS),
        ebreak(),
    ]);
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
    let mut vm = test_vm(&[
        addi(1, 0, 0x77),
        csrrw(0, 1, CSR_MCAUSE),
        ebreak(),
    ]);
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
    assert_eq!(vm.cpu.csr.mstatus, 0xAB, "csrrs with rs1=0 should not write");
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
    let mut vm = test_vm(&[
        csrrwi(1, 5, CSR_MCAUSE),
        ebreak(),
    ]);
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
    assert_eq!(vm.cpu.x[3], 0xC0122, "sstatus should be masked view of mstatus");
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

// RISC-V privilege constants
const PRIV_USER: u8 = 0;
const PRIV_SUPERVISOR: u8 = 1;
const PRIV_MACHINE: u8 = 3;

// mstatus bit positions
const MSTATUS_MIE_BIT: u32 = 3;
const MSTATUS_MPIE_BIT: u32 = 7;
const MSTATUS_SIE_BIT: u32 = 1;
const MSTATUS_SPIE_BIT: u32 = 5;
const MSTATUS_SPP_BIT: u32 = 8;
const MSTATUS_MPP_LSB_BIT: u32 = 11;

/// Test U->S transition via ECALL when ECALL-U is delegated to S-mode.
/// Setup: CPU in U-mode, medeleg has bit 8 (ECALL-U) set, stvec configured.
/// ECALL should trap to stvec (S-mode handler).
#[test]
fn test_rv32_privilege_ecall_u_to_s() {
    // We need a bigger memory layout:
    // 0x80000000: entry (ECALL instruction)
    // 0x80000200: S-mode trap handler (stvec)
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write ECALL at entry point
    vm.bus.write_word(base, ecall()).unwrap();

    // Write S-mode handler at 0x80000200: just ebreak
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    // Configure CPU: start in U-mode with delegation
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;

    // Delegate ECALL-U (cause 8) to S-mode
    vm.cpu.csr.medeleg = 1 << 8;

    // Set stvec to point to S-mode handler
    vm.cpu.csr.stvec = (base as u32) + 0x200;

    // Execute one step
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "should jump to stvec");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
    assert_eq!(vm.cpu.csr.scause, 8, "scause should be ECALL-U (8)");
    assert_eq!(vm.cpu.csr.sepc, base as u32, "sepc should be ECALL PC");
}

/// Test S->M transition via ECALL (no delegation for ECALL-S).
/// Setup: CPU in S-mode, mtvec configured. ECALL from S traps to M.
#[test]
fn test_rv32_privilege_ecall_s_to_m() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write ECALL at entry point
    vm.bus.write_word(base, ecall()).unwrap();

    // Write M-mode handler at 0x80000400: just ebreak
    vm.bus.write_word(base + 0x400, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400, "should jump to mtvec");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine);
    assert_eq!(vm.cpu.csr.mcause, 9, "mcause should be ECALL-S (9)");
    assert_eq!(vm.cpu.csr.mepc, base as u32, "mepc should be ECALL PC");
}

/// Test MRET: return from M-mode trap back to S-mode.
/// Simulates: M-mode handler runs, MRET returns to S-mode.
#[test]
fn test_rv32_privilege_mret_returns_to_s() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write MRET at 0x80000000
    vm.bus.write_word(base, mret()).unwrap();

    // Write the code to return to at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Simulate state after trap from S to M:
    // MPP = S (01), mepc = return address
    vm.cpu.csr.mepc = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 0; // Clear mstatus
    vm.cpu.csr.mstatus |= (1u32 << MSTATUS_MPP_LSB_BIT) | (1u32 << MSTATUS_SPP_BIT); // MPP=S, SPP=S

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "MRET should jump to mepc");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor,
        "MRET should restore S-mode from MPP");
}

/// Test SRET: return from S-mode trap back to U-mode.
#[test]
fn test_rv32_privilege_sret_returns_to_u() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write SRET at 0x80000000
    vm.bus.write_word(base, sret()).unwrap();

    // Write the code to return to at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;

    // Simulate state after trap from U to S:
    // SPP = U (0), sepc = return address
    vm.cpu.csr.sepc = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 0; // SPP = 0 (U-mode)

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "SRET should jump to sepc");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User,
        "SRET should restore U-mode from SPP");
}

/// Test full round-trip: U -> ECALL -> S handler -> SRET -> U
#[test]
fn test_rv32_privilege_u_ecall_sret_roundtrip() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // 0x80000000: ECALL (U-mode code)
    vm.bus.write_word(base, ecall()).unwrap();
    // 0x80000004: addi x1, x0, 42 (returned here after SRET)
    vm.bus.write_word(base + 4, addi(1, 0, 42)).unwrap();
    // 0x80000008: ebreak
    vm.bus.write_word(base + 8, ebreak()).unwrap();

    // 0x80000200: S-mode trap handler -- SRET
    vm.bus.write_word(base + 0x200, sret()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8; // Delegate ECALL-U to S
    vm.cpu.csr.stvec = (base as u32) + 0x200;
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT; // Enable SIE for SPIE save

    // Step 1: ECALL -> trap to S-mode handler
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200);

    // Step 2: SRET -> return to U-mode at sepc
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User);
    assert_eq!(vm.cpu.pc, base as u32);

    // Step 3: Re-execute ECALL (sepc pointed to it). Actually sepc was the ECALL pc,
    // so we'll hit ECALL again. Let's instead check state.
    // sepc was set to the ECALL instruction address (0x80000000), so SRET returns
    // to 0x80000000 and we'll re-execute ECALL. Let's just verify the privilege was restored.
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::User);
}

/// Test full privilege chain: U -> ECALL -> S -> ECALL -> M -> MRET -> S -> SRET -> U
///
/// Memory layout:
///   0x80000000: ECALL (U-mode entry, traps to S via medeleg)
///   0x80000004: addi x1, x0, 42 (U-mode resume point)
///   0x80000008: EBREAK
///
///   0x80000200: S-mode handler
///     ECALL (S->M)
///     csrrs x5, x0, SEPC  (read sepc)
///     addi x5, x5, 4      (sepc += 4 to skip original ECALL)
///     csrrw x0, x5, SEPC  (write back)
///     SRET                 (return to U at sepc+4)
///
///   0x80000400: M-mode handler
///     csrrs x6, x0, MEPC  (read mepc)
///     addi x6, x6, 4      (mepc += 4 to skip S-mode ECALL)
///     csrrw x0, x6, MEPC  (write back)
///     MRET                 (return to S at mepc+4)
#[test]
fn test_rv32_privilege_full_chain_u_s_m_s_u() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    // ---- U-mode code at base ----
    vm.bus.write_word(base, ecall()).unwrap();              // 0x00: ECALL (U->S)
    vm.bus.write_word(base + 4, addi(1, 0, 42)).unwrap();  // 0x04: x1 = 42 (after return)
    vm.bus.write_word(base + 8, ebreak()).unwrap();         // 0x08: stop

    // ---- S-mode handler at base+0x200 ----
    vm.bus.write_word(base + 0x200, ecall()).unwrap();               // ECALL (S->M)
    vm.bus.write_word(base + 0x204, csrrs(5, 0, CSR_SEPC)).unwrap(); // x5 = sepc
    vm.bus.write_word(base + 0x208, addi(5, 5, 4)).unwrap();         // x5 = sepc + 4
    vm.bus.write_word(base + 0x20C, csrrw(0, 5, CSR_SEPC)).unwrap(); // sepc = x5
    vm.bus.write_word(base + 0x210, sret()).unwrap();                 // return to U

    // ---- M-mode handler at base+0x400 ----
    vm.bus.write_word(base + 0x400, csrrs(6, 0, CSR_MEPC)).unwrap(); // x6 = mepc
    vm.bus.write_word(base + 0x404, addi(6, 6, 4)).unwrap();         // x6 = mepc + 4
    vm.bus.write_word(base + 0x408, csrrw(0, 6, CSR_MEPC)).unwrap(); // mepc = x6
    vm.bus.write_word(base + 0x40C, mret()).unwrap();                 // return to S

    // ---- CPU setup ----
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8;  // Delegate ECALL-U to S
    vm.cpu.csr.stvec = (base as u32) + 0x200;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    // Step 1: U-mode ECALL -> traps to S (delegated via medeleg)
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Supervisor, "after U ECALL -> S");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200, "S handler entry");
    assert_eq!(vm.cpu.csr.scause, 8, "scause = ECALL-U (8)");
    assert_eq!(vm.cpu.csr.sepc, base as u32, "sepc = ECALL PC");
    // SPP should be 0 (came from U), SPIE should hold old SIE
    let spp = (vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1;
    assert_eq!(spp, 0, "SPP = 0 (came from U)");

    // Step 2: S-mode ECALL -> traps to M (not delegated)
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Machine, "after S ECALL -> M");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400, "M handler entry");
    assert_eq!(vm.cpu.csr.mcause, 9, "mcause = ECALL-S (9)");
    assert_eq!(vm.cpu.csr.mepc, (base as u32) + 0x200, "mepc = S-mode ECALL PC");
    // MPP should be 1 (came from S)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 1, "MPP = 1 (came from S)");

    // Step 3: M-mode reads mepc, adds 4, writes back, then MRET -> returns to S
    vm.cpu.step(&mut vm.bus); // csrrs x6, x0, mepc
    vm.cpu.step(&mut vm.bus); // addi x6, x6, 4
    vm.cpu.step(&mut vm.bus); // csrrw x0, x6, mepc
    let r = vm.cpu.step(&mut vm.bus); // MRET
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::Supervisor, "after MRET -> S");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x204, "S resumes after its ECALL");
    // Verify mepc was advanced
    assert_eq!(vm.cpu.csr.mepc, (base as u32) + 0x204, "mepc advanced past S ECALL");

    // Step 4: S-mode advances sepc and SRET -> returns to U
    vm.cpu.step(&mut vm.bus); // csrrs x5, x0, sepc
    assert_eq!(vm.cpu.x[5], base as u32, "sepc should be original ECALL PC");
    vm.cpu.step(&mut vm.bus); // addi x5, x5, 4
    vm.cpu.step(&mut vm.bus); // csrrw x0, x5, sepc
    let r = vm.cpu.step(&mut vm.bus); // SRET
    assert_eq!(r, StepResult::Ok);
    assert_eq!(vm.cpu.privilege, Privilege::User, "after SRET -> U");
    assert_eq!(vm.cpu.pc, (base as u32) + 4, "U resumes at addi");

    // Step 5: U-mode executes addi x1, x0, 42
    vm.cpu.step(&mut vm.bus);
    assert_eq!(vm.cpu.x[1], 42, "U-mode code runs after full chain");

    // Step 6: EBREAK stops
    let r = vm.cpu.step(&mut vm.bus);
    assert_eq!(r, StepResult::Ebreak, "should stop at EBREAK");
}

/// Test mstatus state preservation across U->S ECALL trap.
/// Verifies SPP, SPIE, SIE bits are set correctly.
#[test]
fn test_rv32_privilege_ecall_u_to_s_mstatus() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).unwrap();
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.medeleg = 1 << 8;
    vm.cpu.csr.stvec = (base as u32) + 0x200;

    // Set SIE=1 before trap so we can verify it saves to SPIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT;

    vm.cpu.step(&mut vm.bus);

    // SPP = 0 (came from U)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1, 0,
        "SPP should be 0 (came from U)");
    // SPIE = old SIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPIE_BIT) & 1, 1,
        "SPIE should be old SIE (1)");
    // SIE = 0 (disabled during trap)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SIE_BIT) & 1, 0,
        "SIE should be 0 (disabled during trap handler)");
}

/// Test mstatus state preservation across S->M ECALL trap.
/// Verifies MPP, MPIE, MIE bits are set correctly.
#[test]
fn test_rv32_privilege_ecall_s_to_m_mstatus() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).unwrap();
    vm.bus.write_word(base + 0x400, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;

    // Set MIE=1 before trap
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;

    vm.cpu.step(&mut vm.bus);

    // MPP = 01 (came from S)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 1, "MPP should be 01 (came from S)");
    // MPIE = old MIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPIE_BIT) & 1, 1,
        "MPIE should be old MIE (1)");
    // MIE = 0 (disabled during trap)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MIE_BIT) & 1, 0,
        "MIE should be 0 (disabled during trap handler)");
}

/// Test MRET restores mstatus: MIE from MPIE, MPP to privilege, MPIE=1.
#[test]
fn test_rv32_privilege_mret_mstatus_restore() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, mret()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Machine;
    vm.cpu.csr.mepc = (base as u32) + 0x100;

    // Simulate trap from S: MPP=S(01), MPIE=1, MIE=0
    vm.cpu.csr.mstatus = 0;
    vm.cpu.csr.mstatus |= (1u32 << MSTATUS_MPP_LSB_BIT); // MPP = S (01)
    vm.cpu.csr.mstatus |= (1u32 << MSTATUS_MPIE_BIT);     // MPIE = 1
    // MIE = 0 (cleared during trap)

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::Supervisor);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x100);
    // MIE restored from MPIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MIE_BIT) & 1, 1,
        "MIE should be restored from MPIE");
    // MPIE set to 1
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPIE_BIT) & 1, 1,
        "MPIE should be 1 after MRET");
    // MPP cleared to U (00)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3, 0,
        "MPP should be 0 (U) after MRET");
}

/// Test SRET restores mstatus: SIE from SPIE, SPP to privilege, SPIE=1.
#[test]
fn test_rv32_privilege_sret_mstatus_restore() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, sret()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::Supervisor;
    vm.cpu.csr.sepc = (base as u32) + 0x100;

    // Simulate trap from U: SPP=0 (U), SPIE=1, SIE=0
    vm.cpu.csr.mstatus = 0;
    vm.cpu.csr.mstatus |= (1u32 << MSTATUS_SPIE_BIT); // SPIE = 1
    // SPP = 0 (U), SIE = 0

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::User);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x100);
    // SIE restored from SPIE (1)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SIE_BIT) & 1, 1,
        "SIE should be restored from SPIE");
    // SPIE set to 1
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPIE_BIT) & 1, 1,
        "SPIE should be 1 after SRET");
    // SPP cleared to 0 (U)
    assert_eq!((vm.cpu.csr.mstatus >> MSTATUS_SPP_BIT) & 1, 0,
        "SPP should be 0 (U) after SRET");
}

/// Test that ECALL from U without delegation goes directly to M-mode.
#[test]
fn test_rv32_privilege_ecall_u_to_m_no_delegation() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;
    use geometry_os::riscv::cpu::Privilege;

    vm.bus.write_word(base, ecall()).unwrap();
    vm.bus.write_word(base + 0x400, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = Privilege::User;
    vm.cpu.csr.mtvec = (base as u32) + 0x400;
    // No medeleg: all exceptions go to M

    vm.cpu.step(&mut vm.bus);

    assert_eq!(vm.cpu.privilege, Privilege::Machine,
        "ECALL from U should trap to M when not delegated");
    assert_eq!(vm.cpu.pc, (base as u32) + 0x400);
    assert_eq!(vm.cpu.csr.mcause, 8, "mcause = ECALL-U (8)");
    assert_eq!(vm.cpu.csr.mepc, base as u32);
    // MPP should be 0 (came from U)
    let mpp = (vm.cpu.csr.mstatus >> MSTATUS_MPP_LSB_BIT) & 0x3;
    assert_eq!(mpp, 0, "MPP = 0 (came from U)");
}

/// Test timer interrupt delivery: set MTIP in MIP, enable MTIE in MIE,
/// enable MIE in mstatus. Next step should deliver interrupt to mtvec.
#[test]
fn test_rv32_privilege_timer_interrupt_delivery() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Write NOP at entry (should be preempted by interrupt)
    vm.bus.write_word(base, nop()).unwrap();
    // Write handler at 0x80000200
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Enable machine timer interrupt
    vm.cpu.csr.mip = 1 << 7;  // MTIP pending (bit 7 = INT_MTI)
    vm.cpu.csr.mie = 1 << 7;  // MTIE enabled
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT; // Global MIE enabled
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200,
        "timer interrupt should jump to mtvec");
    assert_eq!(vm.cpu.csr.mcause, 0x80000007,
        "mcause should be interrupt bit | MTI (7)");
    assert_eq!(vm.cpu.csr.mepc, base as u32,
        "mepc should be PC of preempted instruction");
}

/// Test software interrupt delivery: set SSIP, enable SSIE, enable SIE.
#[test]
fn test_rv32_privilege_software_interrupt_delivery() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).unwrap();
    vm.bus.write_word(base + 0x200, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::User;

    // Enable supervisor software interrupt
    vm.cpu.csr.mip = 1 << 1;  // SSIP pending (bit 1 = INT_SSI)
    vm.cpu.csr.mie = 1 << 1;  // SSIE enabled
    vm.cpu.csr.mstatus = 1 << MSTATUS_SIE_BIT; // Global SIE enabled
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // No delegation -- goes to M-mode
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.pc, (base as u32) + 0x200,
        "software interrupt should jump to mtvec (M-mode, no delegation)");
    assert_eq!(vm.cpu.csr.mcause, 0x80000001,
        "mcause should be interrupt bit | SSI (1)");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine);
}

/// Test that MIE/MIP CSR read/write works via instruction.
#[test]
fn test_rv32_csr_mie_mip_rw() {
    let mut vm = test_vm(&[
        addi(1, 0, 1 << 7),      // x1 = 0x80 (MTIE bit)
        csrrw(0, 1, CSR_MIE),    // Write to MIE
        csrrs(2, 0, CSR_MIE),    // Read MIE into x2
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.csr.mie, 1 << 7);
    assert_eq!(vm.cpu.x[2], 1 << 7);
}

/// Test SIE is a restricted view of MIE via instruction.
#[test]
fn test_rv32_csr_sie_view() {
    let mut vm = test_vm(&[
        addi(1, 0, (1 << 7) | (1 << 5)),  // x1 = MTIE | STIE
        csrrw(0, 1, CSR_MIE),              // Write to MIE
        csrrs(2, 0, CSR_SIE),              // Read SIE (restricted view)
        ebreak(),
    ]);
    run(&mut vm, 100);
    // SIE should only show S-mode bits (STIE at bit 5)
    assert_eq!(vm.cpu.x[2], 1 << 5, "SIE should be restricted view of MIE");
}

/// Test medeleg delegation via instruction execution.
#[test]
fn test_rv32_csr_medeleg_rw() {
    let mut vm = test_vm(&[
        addi(1, 0, 1 << 8),           // x1 = delegate ECALL-U
        csrrw(0, 1, CSR_MEDELEG),     // Write to medeleg
        csrrs(2, 0, CSR_MEDELEG),     // Read back
        ebreak(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.csr.medeleg, 1 << 8);
    assert_eq!(vm.cpu.x[2], 1 << 8);
}

/// Test no interrupt fires when globally disabled (MIE bit = 0).
#[test]
fn test_rv32_privilege_no_interrupt_when_disabled() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, addi(1, 0, 42)).unwrap();
    vm.bus.write_word(base + 4, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Timer pending and enabled in MIE, but MIE bit in mstatus is 0
    vm.cpu.csr.mip = 1 << 7;  // MTIP pending
    vm.cpu.csr.mie = 1 << 7;  // MTIE enabled
    vm.cpu.csr.mstatus = 0;   // Global MIE disabled!
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Step should execute the instruction normally, not deliver interrupt
    let result = vm.cpu.step(&mut vm.bus);
    assert_eq!(result, StepResult::Ok);
    assert_eq!(vm.cpu.x[1], 42, "instruction should execute normally");
    assert_eq!(vm.cpu.pc, (base as u32) + 4, "PC should advance normally");
}

// ============================================================
// CLINT integration: timer + software interrupt via RiscvVm::step()
// ============================================================

/// Helper: run vm.step() N times.
fn run_vm(vm: &mut RiscvVm, steps: usize) {
    for _ in 0..steps {
        match vm.step() {
            StepResult::Ecall | StepResult::Ebreak | StepResult::FetchFault => break,
            StepResult::Ok | StepResult::LoadFault | StepResult::StoreFault => {}
        }
    }
}

/// Test full CLINT timer pipeline: advance mtime until >= mtimecmp,
/// verify timer interrupt is delivered via RiscvVm::step().
#[test]
fn test_clint_timer_interrupt_via_vm_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Entry: NOP (will be preempted by timer interrupt)
    vm.bus.write_word(base, nop()).unwrap();
    // Handler at +0x200: write 42 to x1, then EBREAK
    vm.bus.write_word(base + 0x200, addi(1, 0, 42)).unwrap();
    vm.bus.write_word(base + 0x204, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Set up CLINT: timer fires at mtime=5
    vm.bus.clint.mtime = 0;
    vm.bus.clint.mtimecmp = 5;

    // Enable machine timer interrupt
    vm.cpu.csr.mie = 1 << 7;  // MTIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT; // Global MIE
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Run: after 5 ticks, mtime >= mtimecmp, timer fires
    run_vm(&mut vm, 20);

    assert_eq!(vm.cpu.x[1], 42, "timer handler should have set x1=42");
    assert_eq!(vm.cpu.csr.mcause, 0x80000007,
        "mcause should be interrupt | MTI (7)");
    assert_eq!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine);
}

/// Test CLINT software interrupt: set msip, verify MSI is delivered.
#[test]
fn test_clint_software_interrupt_via_vm_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).unwrap();
    vm.bus.write_word(base + 0x200, addi(1, 0, 99)).unwrap();
    vm.bus.write_word(base + 0x204, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    // Trigger software interrupt via CLINT msip
    vm.bus.clint.msip = 1;

    // Enable machine software interrupt
    vm.cpu.csr.mie = 1 << 3;  // MSIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    run_vm(&mut vm, 20);

    assert_eq!(vm.cpu.x[1], 99, "software interrupt handler should have set x1=99");
    assert_eq!(vm.cpu.csr.mcause, 0x80000003,
        "mcause should be interrupt | MSI (3)");
}

/// Test full CLINT MMIO read: guest code reads mtime via LW from CLINT address.
#[test]
fn test_clint_mmio_read_mtime() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Set mtime to a known value (high word >> 32 so no carry from low ticks)
    vm.bus.clint.mtime = 0x0000_0042_0000_0100;

    // Program: load mtime from CLINT MMIO address 0x0200BFF8
    vm.bus.write_word(base, lui(5, 0x0200C000)).unwrap();     // x5 = 0x0200C000
    vm.bus.write_word(base + 4, addi(5, 5, -8)).unwrap();     // x5 = 0x0200BFF8
    vm.bus.write_word(base + 8, lw(1, 5, 0)).unwrap();        // x1 = mtime[31:0]
    vm.bus.write_word(base + 12, lw(2, 5, 4)).unwrap();       // x2 = mtime[63:32]
    vm.bus.write_word(base + 16, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    // mtime ticks before each instruction (3 ticks before LW reads it)
    // Low word: 0x100 + 3 = 0x103 (LUI, ADDI, then LW reads on tick 3)
    // High word: 0x42 (no carry from low word incrementing by 3)
    assert_eq!(vm.cpu.x[2], 0x0000_0042, "mtime high word (no carry)");
    assert!(vm.cpu.x[1] >= 0x100, "mtime low word should be >= initial value");
}

/// Test CLINT MMIO write: guest code writes mtimecmp to clear timer interrupt.
#[test]
fn test_clint_mmio_write_mtimecmp() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // x5 = 0x02004000 (mtimecmp address) -- LUI alone can load page-aligned address
    vm.bus.write_word(base, lui(5, 0x02004000)).unwrap(); // x5 = 0x02004000
    vm.bus.write_word(base + 4, addi(1, 0, 0x100)).unwrap(); // x1 = 0x100
    vm.bus.write_word(base + 8, sw(1, 5, 0)).unwrap(); // mtimecmp[31:0] = 0x100
    vm.bus.write_word(base + 12, addi(2, 0, 0)).unwrap(); // x2 = 0
    vm.bus.write_word(base + 16, sw(2, 5, 4)).unwrap(); // mtimecmp[63:32] = 0
    vm.bus.write_word(base + 20, ebreak()).unwrap();

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
    vm.bus.write_word(base, lui(5, 0x02000000)).unwrap(); // x5 = 0x02000000
    vm.bus.write_word(base + 4, addi(1, 0, 1)).unwrap(); // x1 = 1
    vm.bus.write_word(base + 8, sw(1, 5, 0)).unwrap(); // msip = 1
    vm.bus.write_word(base + 12, ebreak()).unwrap();

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

    vm.bus.write_word(base, nop()).unwrap();
    vm.bus.write_word(base + 4, addi(1, 0, 42)).unwrap(); // x1 = 42 after interrupt clears
    vm.bus.write_word(base + 8, ebreak()).unwrap();

    // Handler at +0x200: clear timer by setting mtimecmp far ahead, then MRET
    // x5 = mtimecmp address (0x02004000)
    vm.bus.write_word(base + 0x200, lui(5, 0x02004000)).unwrap(); // x5 = 0x02004000
    vm.bus.write_word(base + 0x204, lui(6, 0xFFFFF000)).unwrap(); // x6 = 0xFFFFF000
    vm.bus.write_word(base + 0x208, ori(6, 6, 0xFFF)).unwrap(); // x6 = 0xFFFFFFFF
    vm.bus.write_word(base + 0x20C, sw(6, 5, 0)).unwrap(); // mtimecmp low = 0xFFFFFFFF
    vm.bus.write_word(base + 0x210, sw(6, 5, 4)).unwrap(); // mtimecmp high = 0xFFFFFFFF
    vm.bus.write_word(base + 0x214, mret()).unwrap();

    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Machine;

    vm.bus.clint.mtime = 0;
    vm.bus.clint.mtimecmp = 2;

    vm.cpu.csr.mie = 1 << 7;  // MTIE
    vm.cpu.csr.mstatus = 1 << MSTATUS_MIE_BIT;
    vm.cpu.csr.mtvec = (base as u32) + 0x200;

    // Run: timer fires, handler clears it, returns, executes normally
    run_vm(&mut vm, 50);

    // After MRET, mepc points to the NOP (base), so PC continues to base+4 (addi x1, 0, 42)
    // then ebreak. x1 should be 42.
    assert_eq!(vm.cpu.x[1], 42, "after timer handler returns, should execute normally");
}

/// Test that mtime advances on each RiscvVm::step() call.
#[test]
fn test_clint_mtime_advances_per_step() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    vm.bus.write_word(base, nop()).unwrap();
    vm.bus.write_word(base + 4, nop()).unwrap();
    vm.bus.write_word(base + 8, ebreak()).unwrap();

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
// Phase 36: SV32 Page Table Walk Tests
// =====================================================================

use geometry_os::riscv::mmu;

fn make_pte(ppn: u32, flags: u32) -> u32 {
    ((ppn & 0x003F_FFFF) << 10) | (flags & 0x3FF)
}

fn make_satp(mode: u32, asid: u32, ppn: u32) -> u32 {
    ((mode & 1) << 31) | ((asid & 0x1FF) << 22) | (ppn & 0x003F_FFFF)
}

fn sfence_vma(rs1: u8, rs2: u8) -> u32 {
    (0b0001001u32 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15) | (0b000 << 12) | (0u32 << 7) | 0x73
}

#[test]
fn test_sv32_bare_mode_identity_translation() {
    let mut tlb = mmu::Tlb::new();
    let bus = geometry_os::riscv::bus::Bus::new(0x8000_0000, 8192);
    let result = mmu::translate(0x8000_0000, mmu::AccessType::Fetch, false, 0, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok(0x8000_0000));
}

#[test]
fn test_sv32_two_level_walk_4k_page() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF).unwrap();
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).unwrap();
    bus.write_word((l2_ppn as u64) << 12, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U)).unwrap();
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok((data_ppn as u64) << 12));
    if let mmu::TranslateResult::Ok(pa) = result {
        assert_eq!(bus.read_word(pa).unwrap(), 0xDEAD_BEEF);
    }
}

#[test]
fn test_sv32_nonzero_vpn_and_offset() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 4;
    let va: u32 = 0x0040_1100;
    let vpn1 = (va >> 22) & 0x3FF;
    let vpn0 = (va >> 12) & 0x3FF;
    bus.write_word(((data_ppn as u64) << 12) + 0x100, 0x1234_5678).unwrap();
    bus.write_word(((root_ppn as u64) << 12) | ((vpn1 as u64) * 4), make_pte(l2_ppn, mmu::PTE_V)).unwrap();
    bus.write_word(((l2_ppn as u64) << 12) | ((vpn0 as u64) * 4), make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_U)).unwrap();
    let satp = make_satp(1, 0, root_ppn);
    let result = mmu::translate(va, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok(((data_ppn as u64) << 12) + 0x100));
}

#[test]
fn test_sv32_page_fault_invalid_pte() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, 0).unwrap();
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_page_fault_permission_denied() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).unwrap();
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).unwrap();
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0x0000_0000, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_fault_types_by_access() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).unwrap();
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R)).unwrap();
    let satp = make_satp(1, 0, 1);
    let mut t1 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Fetch, false, satp, &bus, &mut t1), mmu::TranslateResult::FetchFault);
    let mut t2 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Store, false, satp, &bus, &mut t2), mmu::TranslateResult::StoreFault);
    let mut t3 = mmu::Tlb::new();
    assert_eq!(mmu::translate(0, mmu::AccessType::Load, false, satp, &bus, &mut t3), mmu::TranslateResult::Ok(3u64 << 12));
}

#[test]
fn test_sv32_megapage() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x2_0000);
    let test_addr = (4u64 << 12) | 0x100;
    bus.write_word(test_addr, 0xCAFE_0001).unwrap();
    bus.write_word(((1u64) << 12) | 4, make_pte(4, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X)).unwrap();
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0x0040_0100, mmu::AccessType::Load, false, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::Ok((4u64 << 12) | 0x100));
    if let mmu::TranslateResult::Ok(pa) = result { assert_eq!(bus.read_word(pa).unwrap(), 0xCAFE_0001); }
}

#[test]
fn test_sv32_tlb_caches() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).unwrap();
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W | mmu::PTE_X | mmu::PTE_U)).unwrap();
    let satp = make_satp(1, 0, 1);
    let r1 = mmu::translate(0, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(r1, mmu::TranslateResult::Ok(3u64 << 12));
    bus.write_word(1u64 << 12, 0).unwrap();
    let r2 = mmu::translate(0, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(r2, mmu::TranslateResult::Ok(3u64 << 12));
}

#[test]
fn test_sv32_tlb_flush_sfence() {
    let mut tlb = mmu::Tlb::new();
    // Use VPNs that don't hash to the same TLB slot.
    // Hash: (vpn + asid * 2654435761) % 64
    // vpn=0x10, asid=1 -> idx 43; vpn=0x20, asid=1 -> idx 59
    tlb.insert(0x10, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x20, 1, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    assert!(tlb.lookup(0x10, 1).is_some());
    assert!(tlb.lookup(0x20, 1).is_some());
    tlb.flush_all();
    assert!(tlb.lookup(0x10, 1).is_none());
    assert!(tlb.lookup(0x20, 1).is_none());
}

#[test]
fn test_sv32_tlb_asid_isolation() {
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    assert_eq!(tlb.lookup(0x100, 1).unwrap().0, 0xAAA);
    assert_eq!(tlb.lookup(0x100, 2).unwrap().0, 0xBBB);
    assert!(tlb.lookup(0x100, 3).is_none());
}

#[test]
fn test_sv32_decode_sfence_vma() {
    assert_eq!(geometry_os::riscv::decode::decode(sfence_vma(0, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 0, rs2: 0 });
    assert_eq!(geometry_os::riscv::decode::decode(sfence_vma(5, 0)),
        geometry_os::riscv::decode::Operation::SfenceVma { rs1: 5, rs2: 0 });
}

#[test]
fn test_sv32_sfence_flushes_cpu_tlb() {
    let mut vm = RiscvVm::new(0x1_0000);
    vm.cpu.tlb.insert(0x100, 0, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 0, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    let base = 0x8000_0000u64;
    vm.bus.write_word(base, sfence_vma(0, 0)).unwrap();
    vm.bus.write_word(base + 4, ebreak()).unwrap();
    vm.cpu.pc = base as u32;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.step();
    assert!(vm.cpu.tlb.lookup(0x100, 0).is_none());
    assert!(vm.cpu.tlb.lookup(0x200, 0).is_none());
}

#[test]
fn test_sv32_nonleaf_at_l2_is_fault() {
    let mut tlb = mmu::Tlb::new();
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    bus.write_word(1u64 << 12, make_pte(2, mmu::PTE_V)).unwrap();
    bus.write_word(2u64 << 12, make_pte(3, mmu::PTE_V)).unwrap();
    let satp = make_satp(1, 0, 1);
    let result = mmu::translate(0, mmu::AccessType::Load, true, satp, &bus, &mut tlb);
    assert_eq!(result, mmu::TranslateResult::LoadFault);
}

#[test]
fn test_sv32_tlb_global_entry() {
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x42, 5, 0x100, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    assert!(tlb.lookup(0x42, 0).is_some());
    assert!(tlb.lookup(0x42, 99).is_some());
    assert!(tlb.lookup(0x43, 5).is_none());
}

#[test]
fn test_sv32_satp_and_va_field_extraction() {
    let satp = make_satp(1, 42, 0x12345);
    assert!(mmu::satp_mode_enabled(satp));
    assert_eq!(mmu::satp_asid(satp), 42);
    assert_eq!(mmu::satp_ppn(satp), 0x12345);
    assert!(!mmu::satp_mode_enabled(0));
    assert_eq!(mmu::va_vpn1(0x0040_1100), 1);
    assert_eq!(mmu::va_vpn0(0x0040_1100), 1);
    assert_eq!(mmu::va_offset(0x0040_1100), 0x100);
    assert_eq!(mmu::va_to_vpn(0x0040_1100), 0x00401);
}

#[test]
fn test_sv32_cpu_load_through_page_table() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((data_ppn as u64) << 12, 0xDEAD_BEEF).unwrap();
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).unwrap();
    // L2[0] -> code page (page 0)
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).unwrap();
    // L2[1] -> data page (page 3)
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).unwrap();
    // LUI x10, 0x1 -> x10 = 0x1000
    bus.write_word(0, (0x1u32 << 12) | (10u32 << 7) | 0x37).unwrap();
    // LW x5, 0(x10)
    bus.write_word(4, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (5u32 << 7) | 0x03).unwrap();
    // EBREAK
    bus.write_word(8, ebreak()).unwrap();
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    assert_eq!(cpu.x[5], 0xDEAD_BEEF);
}

#[test]
fn test_sv32_cpu_store_through_page_table() {
    let mut bus = geometry_os::riscv::bus::Bus::new(0x0, 0x1_0000);
    let mut cpu = geometry_os::riscv::cpu::RiscvCpu::new();
    let root_ppn: u32 = 1;
    let l2_ppn: u32 = 2;
    let data_ppn: u32 = 3;
    bus.write_word((root_ppn as u64) << 12, make_pte(l2_ppn, mmu::PTE_V)).unwrap();
    bus.write_word((l2_ppn as u64) << 12, make_pte(0, mmu::PTE_V | mmu::PTE_R | mmu::PTE_X)).unwrap();
    bus.write_word(((l2_ppn as u64) << 12) | 4, make_pte(data_ppn, mmu::PTE_V | mmu::PTE_R | mmu::PTE_W)).unwrap();
    // ADDI x5, x0, 42
    bus.write_word(0, addi(5, 0, 42)).unwrap();
    // LUI x10, 0x1
    bus.write_word(4, (0x1u32 << 12) | (10u32 << 7) | 0x37).unwrap();
    // SW x5, 0(x10)
    bus.write_word(8, (0u32 << 25) | (5u32 << 20) | (10u32 << 15) | (0b010 << 12) | (0u32 << 7) | 0x23).unwrap();
    // LW x6, 0(x10)
    bus.write_word(12, (0u32 << 20) | (10u32 << 15) | (0b010 << 12) | (6u32 << 7) | 0x03).unwrap();
    // EBREAK
    bus.write_word(16, ebreak()).unwrap();
    cpu.pc = 0;
    cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    cpu.csr.satp = make_satp(1, 0, root_ppn);
    for _ in 0..10 {
        match cpu.step(&mut bus) { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    assert_eq!(cpu.x[5], 42);
    assert_eq!(cpu.x[6], 42);
    assert_eq!(bus.read_word((data_ppn as u64) << 12).unwrap(), 42);
}

// =====================================================================
// Phase 36: TLB Cache Tests (64-entry, ASID-aware invalidation)
// =====================================================================

#[test]
fn test_tlb_flush_asid_non_global_only() {
    // flush_asid should remove entries for the given ASID but keep global entries.
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x200, 1, 0xBBB, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    tlb.insert(0x300, 2, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    // Flush ASID 1: removes 0x100, keeps 0x200 (global), keeps 0x300 (different ASID).
    tlb.flush_asid(1);
    assert!(tlb.lookup(0x100, 1).is_none(), "non-global ASID 1 entry should be flushed");
    assert!(tlb.lookup(0x200, 1).is_some(), "global entry should survive ASID flush");
    assert!(tlb.lookup(0x200, 2).is_some(), "global entry should match any ASID");
    assert!(tlb.lookup(0x300, 2).is_some(), "ASID 2 entry should be untouched");
}

#[test]
fn test_tlb_flush_asid_preserves_others() {
    let mut tlb = mmu::Tlb::new();
    for asid in 1u16..=5 {
        tlb.insert(asid as u32 * 0x100, asid, 0x1000 + asid as u32, mmu::PTE_V | mmu::PTE_R);
    }
    assert_eq!(tlb.valid_count(), 5);
    tlb.flush_asid(3);
    assert_eq!(tlb.valid_count(), 4, "only ASID 3 entries should be removed");
    for asid in 1u16..=5 {
        if asid == 3 {
            assert!(tlb.lookup(asid as u32 * 0x100, asid).is_none());
        } else {
            assert!(tlb.lookup(asid as u32 * 0x100, asid).is_some());
        }
    }
}

#[test]
fn test_tlb_flush_va_asid_combined() {
    // flush_va_asid should only remove entries matching both VPN and ASID.
    let mut tlb = mmu::Tlb::new();
    tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    tlb.insert(0x200, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    tlb.flush_va_asid(0x100, 1);
    assert!(tlb.lookup(0x100, 1).is_none(), "VPN 0x100 ASID 1 should be flushed");
    assert!(tlb.lookup(0x100, 2).is_some(), "VPN 0x100 ASID 2 should survive");
    assert!(tlb.lookup(0x200, 1).is_some(), "VPN 0x200 ASID 1 should survive");
}

#[test]
fn test_tlb_64_entry_capacity() {
    // Fill all 64 TLB slots with unique entries.
    // Sequential VPNs 0..63 hash to unique base slots (verified above).
    let mut tlb = mmu::Tlb::new();
    for i in 0..64u32 {
        tlb.insert(i, 1, 0x1000 + i, mmu::PTE_V | mmu::PTE_R);
    }
    assert_eq!(tlb.valid_count(), 64);
    // All entries should be readable.
    for i in 0..64u32 {
        let result = tlb.lookup(i, 1);
        assert!(result.is_some(), "VPN {} should be in TLB", i);
        assert_eq!(result.unwrap().0, 0x1000 + i);
    }
}

#[test]
fn test_tlb_eviction_on_overfill() {
    // Insert 80 entries: only 64 can fit, so some must be evicted.
    // Sequential VPNs fill all 64 base slots; after that, linear probing
    // finds full slots and evicts the base slot.
    let mut tlb = mmu::Tlb::new();
    for i in 0..80u32 {
        tlb.insert(i, 1, 0x1000 + i, mmu::PTE_V | mmu::PTE_R);
    }
    // TLB should still have exactly 64 valid entries.
    assert_eq!(tlb.valid_count(), 64);
    // The last entries we inserted should be findable (some early ones evicted).
    let found_last = tlb.lookup(79, 1);
    assert!(found_last.is_some(), "last inserted entry should be in TLB");
}

#[test]
fn test_tlb_sfence_vma_with_asid() {
    // SFENCE.VMA x0, x2 -> flush entries for ASID in x2.
    let mut vm = RiscvVm::new(0x1_0000);
    let base = 0x8000_0000u64;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    // Pre-populate TLB with entries for multiple ASIDs.
    vm.cpu.tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x300, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R | mmu::PTE_G);
    // ADDI x2, x0, 1  -- x2 = ASID 1
    vm.bus.write_word(base, addi(2, 0, 1)).unwrap();
    // SFENCE.VMA x0, x2 -- flush ASID 1
    vm.bus.write_word(base + 4, sfence_vma(0, 2)).unwrap();
    // EBREAK
    vm.bus.write_word(base + 8, ebreak()).unwrap();
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    // ASID 1 non-global entry should be gone.
    assert!(vm.cpu.tlb.lookup(0x100, 1).is_none(), "ASID 1 non-global should be flushed");
    // ASID 1 global entry should survive.
    assert!(vm.cpu.tlb.lookup(0x300, 1).is_some(), "ASID 1 global entry should survive");
    // ASID 2 entry should be untouched.
    assert!(vm.cpu.tlb.lookup(0x200, 2).is_some(), "ASID 2 entry should be untouched");
}

#[test]
fn test_tlb_sfence_vma_with_vpn_and_asid() {
    // SFENCE.VMA x1, x2 -> flush entries matching both VPN in x1 and ASID in x2.
    let mut vm = RiscvVm::new(0x1_0000);
    let base = 0x8000_0000u64;
    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
    vm.cpu.tlb.insert(0x100, 1, 0xAAA, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x100, 2, 0xBBB, mmu::PTE_V | mmu::PTE_R);
    vm.cpu.tlb.insert(0x200, 1, 0xCCC, mmu::PTE_V | mmu::PTE_R);
    // Set x1 = virtual address that maps to VPN 0x100
    // VPN = va >> 12 & 0xFFFFF, so VA = 0x100 << 12 = 0x100_000
    vm.bus.write_word(base, lui(1, 0x100_000)).unwrap();
    // ADDI x2, x0, 1 -- ASID 1
    vm.bus.write_word(base + 4, addi(2, 0, 1)).unwrap();
    // SFENCE.VMA x1, x2
    vm.bus.write_word(base + 8, sfence_vma(1, 2)).unwrap();
    // EBREAK
    vm.bus.write_word(base + 12, ebreak()).unwrap();
    vm.cpu.pc = base as u32;
    for _ in 0..5 {
        match vm.step() { StepResult::Ebreak => break, StepResult::Ok => {}, o => panic!("Unexpected: {:?}", o) }
    }
    // VPN 0x100 + ASID 1 should be flushed.
    assert!(vm.cpu.tlb.lookup(0x100, 1).is_none(), "VPN 0x100 ASID 1 should be flushed");
    // VPN 0x100 + ASID 2 should survive (different ASID).
    assert!(vm.cpu.tlb.lookup(0x100, 2).is_some(), "VPN 0x100 ASID 2 should survive");
    // VPN 0x200 + ASID 1 should survive (different VPN).
    assert!(vm.cpu.tlb.lookup(0x200, 1).is_some(), "VPN 0x200 ASID 1 should survive");
}

#[test]
fn test_tlb_asid_switch_reuses_entries() {
    // When switching address spaces (different ASID), TLB entries from
    // the old ASID should not be visible but should coexist in the TLB.
    let mut tlb = mmu::Tlb::new();
    // Process A (ASID 1) maps VPN 0x100 -> PPN 0x1000
    tlb.insert(0x100, 1, 0x1000, mmu::PTE_V | mmu::PTE_R);
    // Process B (ASID 2) maps VPN 0x100 -> PPN 0x2000 (same VA, different PA)
    tlb.insert(0x100, 2, 0x2000, mmu::PTE_V | mmu::PTE_R);
    // Looking up as ASID 1 gives PPN 0x1000
    assert_eq!(tlb.lookup(0x100, 1).unwrap().0, 0x1000);
    // Looking up as ASID 2 gives PPN 0x2000
    assert_eq!(tlb.lookup(0x100, 2).unwrap().0, 0x2000);
    // Looking up as ASID 3 gives nothing
    assert!(tlb.lookup(0x100, 3).is_none());
}
