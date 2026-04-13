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

/// Test CLINT MMIO read: guest code reads mtime via LW from CLINT address.
#[test]
fn test_clint_mmio_read_mtime() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // Set mtime to a known value
    vm.bus.clint.mtime = 0x0000_0042_0000_0100;

    // Program: load mtime low word from CLINT MMIO address
    // x5 = 0x0200BFF8 (mtime address)
    // LW x1, 0(x5)
    // EBREAK
    vm.bus.write_word(base, lui(5, 0x0200B000)).unwrap();     // x5 = 0x0200B000
    vm.bus.write_word(base + 4, addi(5, 5, -8)).unwrap();     // x5 = 0x0200AFF8... hmm
    // Actually, let me use a simpler approach with ori
    vm.bus.write_word(base, lui(5, 0x0200C000)).unwrap();     // x5 upper bits
    vm.bus.write_word(base + 4, addi(5, 5, -8)).unwrap();     // x5 = 0x0200BFF8
    vm.bus.write_word(base + 8, lw(1, 5, 0)).unwrap();        // x1 = mtime[31:0]
    vm.bus.write_word(base + 12, lw(2, 5, 4)).unwrap();       // x2 = mtime[63:32]
    vm.bus.write_word(base + 16, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    assert_eq!(vm.cpu.x[1], 0x0000_0100, "mtime low word");
    assert_eq!(vm.cpu.x[2], 0x0000_0042, "mtime high word");
}

/// Test CLINT MMIO write: guest code writes mtimecmp to clear timer interrupt.
#[test]
fn test_clint_mmio_write_mtimecmp() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // x5 = 0x02004000 (mtimecmp address)
    vm.bus.write_word(base, lui(5, 0x02005000)).unwrap();     // x5 upper bits
    vm.bus.write_word(base + 4, addi(5, 5, -0x1000)).unwrap(); // x5 = 0x02004000
    vm.bus.write_word(base + 8, addi(1, 0, 0x100)).unwrap();  // x1 = 0x100
    vm.bus.write_word(base + 12, sw(1, 5, 0)).unwrap();       // mtimecmp[31:0] = 0x100
    vm.bus.write_word(base + 16, addi(2, 0, 0)).unwrap();     // x2 = 0
    vm.bus.write_word(base + 20, sw(2, 5, 4)).unwrap();       // mtimecmp[63:32] = 0
    vm.bus.write_word(base + 24, ebreak()).unwrap();

    vm.cpu.pc = base as u32;
    run_vm(&mut vm, 20);

    assert_eq!(vm.bus.clint.mtimecmp, 0x100, "mtimecmp should be 0x100");
}

/// Test CLINT msip via MMIO write: guest triggers software interrupt.
#[test]
fn test_clint_mmio_write_msip() {
    let mut vm = RiscvVm::new(8192);
    let base = 0x8000_0000u64;

    // x5 = 0x02000000 (msip address)
    vm.bus.write_word(base, lui(5, 0x02001000)).unwrap();
    vm.bus.write_word(base + 4, addi(5, 5, -0x1000)).unwrap(); // x5 = 0x02000000
    vm.bus.write_word(base + 8, addi(1, 0, 1)).unwrap();
    vm.bus.write_word(base + 12, sw(1, 5, 0)).unwrap();        // msip = 1
    vm.bus.write_word(base + 16, ebreak()).unwrap();

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
    // x5 = mtimecmp address
    vm.bus.write_word(base + 0x200, lui(5, 0x02005000)).unwrap();
    vm.bus.write_word(base + 0x204, addi(5, 5, -0x1000)).unwrap(); // x5 = 0x02004000
    vm.bus.write_word(base + 0x208, lui(6, 0xFFFFF000)).unwrap();  // x6 = 0xFFFFF000
    vm.bus.write_word(base + 0x20C, ori(6, 6, 0xFFF)).unwrap();   // x6 = 0xFFFFFFFF
    vm.bus.write_word(base + 0x210, sw(6, 5, 0)).unwrap();        // mtimecmp low = 0xFFFFFFFF
    vm.bus.write_word(base + 0x214, sw(6, 5, 4)).unwrap();        // mtimecmp high = 0xFFFFFFFF
    vm.bus.write_word(base + 0x218, mret()).unwrap();

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