// tests/riscv_tests.rs -- Phase 34: RV32I instruction integration tests
//
// Tests every base RV32I instruction by running hand-encoded instruction
// sequences through the RiscvVm.
//
// Split into focused modules for maintainability.
// Originally a single 3150+ line file.

#[path = "riscv_tests/base.rs"]
pub mod base;
#[path = "riscv_tests/csr.rs"]
pub mod csr;
#[path = "riscv_tests/privilege.rs"]
pub mod privilege;
#[path = "riscv_tests/clint.rs"]
pub mod clint;
#[path = "riscv_tests/sv32.rs"]
pub mod sv32;
#[path = "riscv_tests/sv32_tlb.rs"]
pub mod sv32_tlb;
#[path = "riscv_tests/sv32_faults.rs"]
pub mod sv32_faults;
#[path = "riscv_tests/sv32_interrupt.rs"]
pub mod sv32_interrupt;
#[path = "riscv_tests/integration.rs"]
pub mod integration;
#[path = "riscv_tests/base_programs.rs"]
pub mod base_programs;

use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;

// ---- Helpers ----

pub fn test_vm(instrs: &[u32]) -> RiscvVm {
    let mut vm = RiscvVm::new(4096);
    let ram_base = 0x8000_0000u64;
    for (i, &word) in instrs.iter().enumerate() {
        let _ = vm.bus.write_word(ram_base + (i as u64) * 4, word);
    }
    vm.cpu.pc = ram_base as u32;
    vm
}

pub fn run(vm: &mut RiscvVm, max_steps: usize) {
    // Prevent SBI interception: set a7 (x17) to a non-SBI value so ECALL
    // traps normally instead of being silently handled. This is needed
    // because M-mode SBI interception was added for Linux boot support,
    // and test VMs start in M-mode with all registers at 0 (a7=0 = SBI_CONSOLE_PUTCHAR).
    vm.cpu.x[17] = 0xDEAD;
    for _ in 0..max_steps {
        match vm.cpu.step(&mut vm.bus) {
            StepResult::Ecall | StepResult::Ebreak | StepResult::FetchFault => break,
            StepResult::Ok | StepResult::LoadFault | StepResult::StoreFault => {}
        }
    }
}

// ---- Encoding helpers ----

pub fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((rd as u32) << 7)
        | opcode
}

pub fn i_type(imm: u32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm & 0xFFF) << 20) | ((rs1 as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | opcode
}

pub fn u_type(imm: u32, rd: u8, opcode: u32) -> u32 {
    (imm & 0xFFFF_F000) | ((rd as u32) << 7) | opcode
}

pub fn jal(rd: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    let bit20 = (imm >> 20) & 1;
    let bits10_1 = (imm >> 1) & 0x3FF;
    let bit11 = (imm >> 11) & 1;
    let bits19_12 = (imm >> 12) & 0xFF;
    let encoded = (bit20 << 31) | (bits10_1 << 21) | (bit11 << 20) | (bits19_12 << 12);
    encoded | ((rd as u32) << 7) | 0x6F
}

pub fn jalr(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm as u32, rs1, 0, rd, 0x67)
}

pub fn b_type(rs1: u8, rs2: u8, funct3: u32, imm: i32) -> u32 {
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

pub fn s_type(rs2: u8, rs1: u8, funct3: u32, imm: i32) -> u32 {
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
pub fn ecall() -> u32 { i_type(0, 0, 0, 0, 0x73) }
pub fn lui(rd: u8, imm: u32) -> u32 { u_type(imm, rd, 0x37) }
pub fn auipc(rd: u8, imm: u32) -> u32 { u_type(imm, rd, 0x17) }
pub fn add(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b000, rd, 0x33) }
pub fn sub(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0b0100000, rs2, rs1, 0b000, rd, 0x33) }
pub fn sll(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b001, rd, 0x33) }
pub fn slt(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b010, rd, 0x33) }
pub fn sltu(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b011, rd, 0x33) }
pub fn xor_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b100, rd, 0x33) }
pub fn srl(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b101, rd, 0x33) }
pub fn sra(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0b0100000, rs2, rs1, 0b101, rd, 0x33) }
pub fn or_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b110, rd, 0x33) }
pub fn and_inst(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0b111, rd, 0x33) }
pub fn addi(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b000, rd, 0x13) }
pub fn slti(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b010, rd, 0x13) }
pub fn sltiu(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b011, rd, 0x13) }
pub fn xori(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b100, rd, 0x13) }
pub fn ori(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b110, rd, 0x13) }
pub fn andi(rd: u8, rs1: u8, imm: i32) -> u32 { i_type(imm as u32, rs1, 0b111, rd, 0x13) }
pub fn slli(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(shamt & 0x1F, rs1, 0b001, rd, 0x13) }
pub fn srli(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(shamt & 0x1F, rs1, 0b101, rd, 0x13) }
pub fn srai(rd: u8, rs1: u8, shamt: u32) -> u32 { i_type(0b0100000 << 5 | (shamt & 0x1F), rs1, 0b101, rd, 0x13) }
pub fn lw(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b010, rd, 0x03) }
pub fn lb(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b000, rd, 0x03) }
pub fn lh(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b001, rd, 0x03) }
pub fn lbu(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b100, rd, 0x03) }
pub fn lhu(rd: u8, rs1: u8, off: i32) -> u32 { i_type(off as u32, rs1, 0b101, rd, 0x03) }
pub fn sw(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b010, off) }
pub fn sb(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b000, off) }
pub fn sh(rs2: u8, rs1: u8, off: i32) -> u32 { s_type(rs2, rs1, 0b001, off) }
pub fn beq(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b000, off) }
pub fn bne(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b001, off) }
pub fn blt(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b100, off) }
pub fn bge(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b101, off) }
pub fn bltu(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b110, off) }
pub fn bgeu(rs1: u8, rs2: u8, off: i32) -> u32 { b_type(rs1, rs2, 0b111, off) }
pub fn ebreak() -> u32 { i_type(1, 0, 0, 0, 0x73) }
#[allow(dead_code)]
pub fn fence() -> u32 { 0x0FF0000F }
pub fn nop() -> u32 { addi(0, 0, 0) }
pub fn mret() -> u32 { 0x30200073 }
pub fn sret() -> u32 { 0x10200073 }
#[allow(dead_code)]
pub fn and_(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 7, rd, 0x33) }
pub fn csrrw(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (1u32 << 12) | ((rd as u32) << 7) | 0x73 }
pub fn csrrs(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (2u32 << 12) | ((rd as u32) << 7) | 0x73 }
pub fn csrrc(rd: u8, rs1: u8, csr: u32) -> u32 { (csr << 20) | ((rs1 as u32) << 15) | (3u32 << 12) | ((rd as u32) << 7) | 0x73 }
pub fn csrrwi(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (5u32 << 12) | ((rd as u32) << 7) | 0x73 }
pub fn csrrsi(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (6u32 << 12) | ((rd as u32) << 7) | 0x73 }
pub fn csrrci(rd: u8, uimm: u8, csr: u32) -> u32 { (csr << 20) | ((uimm as u32) << 15) | (7u32 << 12) | ((rd as u32) << 7) | 0x73 }

// CSR address constants
pub const CSR_MSTATUS: u32 = 0x300;
pub const CSR_MTVEC: u32 = 0x305;
pub const CSR_MEPC: u32 = 0x341;
pub const CSR_MCAUSE: u32 = 0x342;
#[allow(dead_code)]
pub const CSR_MTVAL: u32 = 0x343;
pub const CSR_SSTATUS: u32 = 0x100;
#[allow(dead_code)]
pub const CSR_STVEC: u32 = 0x105;
#[allow(dead_code)]
pub const CSR_SATP: u32 = 0x180;
pub const CSR_MIE: u32 = 0x304;
#[allow(dead_code)]
pub const CSR_MIP: u32 = 0x344;
pub const CSR_SIE: u32 = 0x104;
#[allow(dead_code)]
pub const CSR_SIP: u32 = 0x144;
pub const CSR_MEDELEG: u32 = 0x302;
#[allow(dead_code)]
pub const CSR_MIDELEG: u32 = 0x303;
pub const CSR_SEPC: u32 = 0x141;
#[allow(dead_code)]
pub const CSR_SCAUSE: u32 = 0x142;
#[allow(dead_code)]
pub const CSR_STVAL: u32 = 0x143;
