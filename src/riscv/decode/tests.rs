// riscv/decode/tests.rs -- Tests for RV32I and RV32C instruction decode

use super::compressed::{decode_c, is_compressed};
use super::operation::Operation;
use super::rv32i::decode;

fn encode_r(funct7: u32, rs2: u32, rs1: u32, funct3: u32, rd: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | 0x33
}
fn encode_i(imm: u32, rs1: u32, funct3: u32, rd: u32, opcode: u32) -> u32 {
    (imm << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}
fn encode_s(imm: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
    ((imm >> 5) << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | ((imm & 0x1F) << 7) | 0x23
}
fn encode_b(imm: u32, rs2: u32, rs1: u32, funct3: u32) -> u32 {
    ((imm >> 12) << 31)
        | (((imm >> 5) & 0x3F) << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xF) << 8)
        | (((imm >> 11) & 1) << 7)
        | 0x63
}

// R-type
#[test]
fn decode_add() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 0, 5)),
        Operation::Add {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_sub() {
    assert_eq!(
        decode(encode_r(0x20, 3, 1, 0, 5)),
        Operation::Sub {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_sll() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 1, 5)),
        Operation::Sll {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_slt() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 2, 5)),
        Operation::Slt {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_sltu() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 3, 5)),
        Operation::Sltu {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_xor() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 4, 5)),
        Operation::Xor {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_srl() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 5, 5)),
        Operation::Srl {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_sra() {
    assert_eq!(
        decode(encode_r(0x20, 3, 1, 5, 5)),
        Operation::Sra {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_or() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 6, 5)),
        Operation::Or {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}
#[test]
fn decode_and() {
    assert_eq!(
        decode(encode_r(0, 3, 1, 7, 5)),
        Operation::And {
            rd: 5,
            rs1: 1,
            rs2: 3
        }
    );
}

// I-type ALU
#[test]
fn decode_addi() {
    assert_eq!(
        decode(encode_i(42, 2, 0, 1, 0x13)),
        Operation::Addi {
            rd: 1,
            rs1: 2,
            imm: 42
        }
    );
}
#[test]
fn decode_addi_neg() {
    assert_eq!(
        decode(encode_i(0xFFF, 2, 0, 1, 0x13)),
        Operation::Addi {
            rd: 1,
            rs1: 2,
            imm: -1
        }
    );
}
#[test]
fn decode_slti() {
    assert_eq!(
        decode(encode_i(10, 3, 2, 7, 0x13)),
        Operation::Slti {
            rd: 7,
            rs1: 3,
            imm: 10
        }
    );
}
#[test]
fn decode_sltiu() {
    assert_eq!(
        decode(encode_i(10, 3, 3, 7, 0x13)),
        Operation::Sltiu {
            rd: 7,
            rs1: 3,
            imm: 10
        }
    );
}
#[test]
fn decode_xori() {
    assert_eq!(
        decode(encode_i(0xFF, 4, 4, 8, 0x13)),
        Operation::Xori {
            rd: 8,
            rs1: 4,
            imm: 0xFF
        }
    );
}
#[test]
fn decode_ori() {
    assert_eq!(
        decode(encode_i(0xFF, 4, 6, 8, 0x13)),
        Operation::Ori {
            rd: 8,
            rs1: 4,
            imm: 0xFF
        }
    );
}
#[test]
fn decode_andi() {
    assert_eq!(
        decode(encode_i(0xFF, 4, 7, 8, 0x13)),
        Operation::Andi {
            rd: 8,
            rs1: 4,
            imm: 0xFF
        }
    );
}
#[test]
fn decode_slli() {
    let w = (0u32 << 25) | (5u32 << 20) | (2u32 << 15) | (1u32 << 12) | (1u32 << 7) | 0x13;
    assert_eq!(
        decode(w),
        Operation::Slli {
            rd: 1,
            rs1: 2,
            shamt: 5
        }
    );
}
#[test]
fn decode_srli() {
    let w = (0u32 << 25) | (7u32 << 20) | (3u32 << 15) | (5u32 << 12) | (1u32 << 7) | 0x13;
    assert_eq!(
        decode(w),
        Operation::Srli {
            rd: 1,
            rs1: 3,
            shamt: 7
        }
    );
}
#[test]
fn decode_srai() {
    let w = (0x20u32 << 25) | (7u32 << 20) | (3u32 << 15) | (5u32 << 12) | (1u32 << 7) | 0x13;
    assert_eq!(
        decode(w),
        Operation::Srai {
            rd: 1,
            rs1: 3,
            shamt: 7
        }
    );
}

// Load
#[test]
fn decode_lb() {
    assert_eq!(
        decode(encode_i(8, 1, 0, 3, 0x03)),
        Operation::Lb {
            rd: 3,
            rs1: 1,
            imm: 8
        }
    );
}
#[test]
fn decode_lh() {
    assert_eq!(
        decode(encode_i(8, 1, 1, 3, 0x03)),
        Operation::Lh {
            rd: 3,
            rs1: 1,
            imm: 8
        }
    );
}
#[test]
fn decode_lw() {
    assert_eq!(
        decode(encode_i(8, 1, 2, 3, 0x03)),
        Operation::Lw {
            rd: 3,
            rs1: 1,
            imm: 8
        }
    );
}
#[test]
fn decode_lbu() {
    assert_eq!(
        decode(encode_i(8, 1, 4, 3, 0x03)),
        Operation::Lbu {
            rd: 3,
            rs1: 1,
            imm: 8
        }
    );
}
#[test]
fn decode_lhu() {
    assert_eq!(
        decode(encode_i(8, 1, 5, 3, 0x03)),
        Operation::Lhu {
            rd: 3,
            rs1: 1,
            imm: 8
        }
    );
}

// Store
#[test]
fn decode_sb() {
    assert_eq!(
        decode(encode_s(4, 5, 1, 0)),
        Operation::Sb {
            rs1: 1,
            rs2: 5,
            imm: 4
        }
    );
}
#[test]
fn decode_sh() {
    assert_eq!(
        decode(encode_s(4, 5, 1, 1)),
        Operation::Sh {
            rs1: 1,
            rs2: 5,
            imm: 4
        }
    );
}
#[test]
fn decode_sw() {
    assert_eq!(
        decode(encode_s(4, 5, 1, 2)),
        Operation::Sw {
            rs1: 1,
            rs2: 5,
            imm: 4
        }
    );
}

// Branch
#[test]
fn decode_beq() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 0)),
        Operation::Beq {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}
#[test]
fn decode_bne() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 1)),
        Operation::Bne {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}
#[test]
fn decode_blt() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 4)),
        Operation::Blt {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}
#[test]
fn decode_bge() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 5)),
        Operation::Bge {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}
#[test]
fn decode_bltu() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 6)),
        Operation::Bltu {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}
#[test]
fn decode_bgeu() {
    assert_eq!(
        decode(encode_b(8, 2, 1, 7)),
        Operation::Bgeu {
            rs1: 1,
            rs2: 2,
            imm: 8
        }
    );
}

// Upper
#[test]
fn decode_lui() {
    assert_eq!(
        decode(0x123452B7),
        Operation::Lui {
            rd: 5,
            imm: 0x12345000
        }
    );
}
#[test]
fn decode_auipc() {
    let w = (0x12345u32 << 12) | (5u32 << 7) | 0x17;
    assert_eq!(
        decode(w),
        Operation::Auipc {
            rd: 5,
            imm: 0x12345000
        }
    );
}

// Jump
#[test]
fn decode_jal() {
    let w = (0u32 << 31) | (4u32 << 21) | (0u32 << 20) | (0u32 << 12) | (1u32 << 7) | 0x6F;
    assert_eq!(decode(w), Operation::Jal { rd: 1, imm: 8 });
}
#[test]
fn decode_jalr() {
    assert_eq!(
        decode(encode_i(0, 1, 0, 5, 0x67)),
        Operation::Jalr {
            rd: 5,
            rs1: 1,
            imm: 0
        }
    );
}

// System
#[test]
fn decode_ecall() {
    assert_eq!(decode(0x00000073), Operation::Ecall);
}
#[test]
fn decode_ebreak() {
    assert_eq!(decode(0x00100073), Operation::Ebreak);
}
#[test]
fn decode_fence() {
    assert_eq!(decode(0x0FF0000F), Operation::Fence);
}

// Invalid
#[test]
fn decode_invalid_zero() {
    assert_eq!(decode(0), Operation::Invalid(0));
}
#[test]
fn decode_rd_zero_valid() {
    assert_eq!(
        decode(encode_r(0, 2, 1, 0, 0)),
        Operation::Add {
            rd: 0,
            rs1: 1,
            rs2: 2
        }
    );
}
#[test]
fn decode_branch_neg_offset() {
    // BEQ with -16 offset
    let imm_raw: u32 = 0x1FF0; // 13-bit representation of -16
    let w = encode_b(imm_raw, 2, 1, 0);
    assert_eq!(
        decode(w),
        Operation::Beq {
            rs1: 1,
            rs2: 2,
            imm: -16
        }
    );
}

// CSR instructions
fn encode_csr(funct3: u32, rd: u8, rs1_uimm: u8, csr: u32) -> u32 {
    ((csr & 0xFFF) << 20) | ((rs1_uimm as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | 0x73
}
#[test]
fn decode_csrrw() {
    assert_eq!(
        decode(encode_csr(0b001, 3, 5, 0x300)),
        Operation::Csrrw {
            rd: 3,
            rs1: 5,
            csr: 0x300
        }
    );
}
#[test]
fn decode_csrrs() {
    assert_eq!(
        decode(encode_csr(0b010, 3, 5, 0x305)),
        Operation::Csrrs {
            rd: 3,
            rs1: 5,
            csr: 0x305
        }
    );
}
#[test]
fn decode_csrrc() {
    assert_eq!(
        decode(encode_csr(0b011, 3, 5, 0x341)),
        Operation::Csrrc {
            rd: 3,
            rs1: 5,
            csr: 0x341
        }
    );
}
#[test]
fn decode_csrrwi() {
    assert_eq!(
        decode(encode_csr(0b101, 3, 7, 0x342)),
        Operation::Csrrwi {
            rd: 3,
            uimm: 7,
            csr: 0x342
        }
    );
}
#[test]
fn decode_csrrsi() {
    assert_eq!(
        decode(encode_csr(0b110, 3, 15, 0x100)),
        Operation::Csrrsi {
            rd: 3,
            uimm: 15,
            csr: 0x100
        }
    );
}
#[test]
fn decode_csrrci() {
    assert_eq!(
        decode(encode_csr(0b111, 3, 31, 0x180)),
        Operation::Csrrci {
            rd: 3,
            uimm: 31,
            csr: 0x180
        }
    );
}

// ---- C extension (RV32C compressed instruction) tests ----
// All encodings verified by Python reference encoder matching the Rust extraction logic.

#[test]
fn c_is_compressed() {
    assert!(is_compressed(0x0000)); // bits01 = 00
    assert!(is_compressed(0x0001)); // bits01 = 01
    assert!(is_compressed(0x0002)); // bits01 = 10
    assert!(!is_compressed(0x0003)); // bits01 = 11 (32-bit)
    assert!(!is_compressed(0xFF03));
    assert!(!is_compressed(0x1233));
}

#[test]
fn c_nop() {
    let op = decode_c(0x0001);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 0,
            rs1: 0,
            imm: 0
        }
    );
}

#[test]
fn c_addi() {
    // C.ADDI rd=5, imm=6
    let op = decode_c(0x0299);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 5,
            rs1: 5,
            imm: 6
        }
    );
}

#[test]
fn c_addi_negative() {
    // C.ADDI rd=5, imm=-1
    let op = decode_c(0x12FD);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 5,
            rs1: 5,
            imm: -1
        }
    );
}

#[test]
fn c_li() {
    // C.LI rd=3, imm=10
    let op = decode_c(0x41A9);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 3,
            rs1: 0,
            imm: 10
        }
    );
}

#[test]
fn c_lui() {
    // C.LUI rd=1, nzimm=0x1000
    let op = decode_c(0x6085);
    assert_eq!(op, Operation::Lui { rd: 1, imm: 0x1000 });
}

#[test]
fn c_lui_zero_is_invalid() {
    let op = decode_c(0x6081);
    assert!(matches!(op, Operation::Invalid(_)));
}

#[test]
fn c_addi16sp() {
    // C.ADDI16SP rd=2, imm=16
    // nzimm[4]=1 (16=2^4), maps to inst bit[6]
    let op = decode_c(0x6141);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 2,
            rs1: 2,
            imm: 16
        }
    );
}

#[test]
fn c_mv() {
    // C.MV rd=10, rs2=11
    let op = decode_c(0x852E);
    assert_eq!(
        op,
        Operation::Add {
            rd: 10,
            rs1: 0,
            rs2: 11
        }
    );
}

#[test]
fn c_add() {
    // C.ADD rd=10, rs2=11
    let op = decode_c(0x952E);
    assert_eq!(
        op,
        Operation::Add {
            rd: 10,
            rs1: 10,
            rs2: 11
        }
    );
}

#[test]
fn c_jr() {
    // C.JR rs1=5
    let op = decode_c(0x8282);
    assert_eq!(
        op,
        Operation::Jalr {
            rd: 0,
            rs1: 5,
            imm: 0
        }
    );
}

#[test]
fn c_jalr() {
    // C.JALR rs1=5 -> jalr x1, x5, 0
    let op = decode_c(0x9282);
    assert_eq!(
        op,
        Operation::Jalr {
            rd: 1,
            rs1: 5,
            imm: 0
        }
    );
}

#[test]
fn c_ebreak() {
    let op = decode_c(0x9002);
    assert_eq!(op, Operation::Ebreak);
}

#[test]
fn c_jal() {
    // C.JAL imm=4
    let op = decode_c(0x2011);
    assert_eq!(op, Operation::Jal { rd: 1, imm: 4 });
}

#[test]
fn c_j() {
    // C.J imm=8
    let op = decode_c(0xA021);
    assert_eq!(op, Operation::Jal { rd: 0, imm: 8 });
}

#[test]
fn c_beqz() {
    // C.BEQZ rs1'=2 (x10), imm=0
    let op = decode_c(0xC101);
    assert_eq!(
        op,
        Operation::Beq {
            rs1: 10,
            rs2: 0,
            imm: 0
        }
    );
}

#[test]
fn c_bnez() {
    // C.BNEZ rs1'=0 (x8), imm=0
    let op = decode_c(0xE001);
    assert_eq!(
        op,
        Operation::Bne {
            rs1: 8,
            rs2: 0,
            imm: 0
        }
    );
}

#[test]
fn c_slli() {
    // C.SLLI rd=5, shamt=3
    let op = decode_c(0x028E);
    assert_eq!(
        op,
        Operation::Slli {
            rd: 5,
            rs1: 5,
            shamt: 3
        }
    );
}

#[test]
fn c_srli() {
    // C.SRLI rd'=2 (x10), shamt=5
    let op = decode_c(0x8115);
    assert_eq!(
        op,
        Operation::Srli {
            rd: 10,
            rs1: 10,
            shamt: 5
        }
    );
}

#[test]
fn c_srai() {
    // C.SRAI rd'=2 (x10), shamt=5
    let op = decode_c(0x8515);
    assert_eq!(
        op,
        Operation::Srai {
            rd: 10,
            rs1: 10,
            shamt: 5
        }
    );
}

#[test]
fn c_andi() {
    // C.ANDI rd'=2 (x10), imm=7
    let op = decode_c(0x891D);
    assert_eq!(
        op,
        Operation::Andi {
            rd: 10,
            rs1: 10,
            imm: 7
        }
    );
}

#[test]
fn c_sub() {
    // C.SUB rd'=2 (x10), rs2'=3 (x11)
    let op = decode_c(0x8D0D);
    assert_eq!(
        op,
        Operation::Sub {
            rd: 10,
            rs1: 10,
            rs2: 11
        }
    );
}

#[test]
fn c_xor() {
    // C.XOR rd'=2 (x10), rs2'=3 (x11)
    let op = decode_c(0x8D2D);
    assert_eq!(
        op,
        Operation::Xor {
            rd: 10,
            rs1: 10,
            rs2: 11
        }
    );
}

#[test]
fn c_or() {
    // C.OR rd'=2 (x10), rs2'=3 (x11)
    let op = decode_c(0x8D4D);
    assert_eq!(
        op,
        Operation::Or {
            rd: 10,
            rs1: 10,
            rs2: 11
        }
    );
}

#[test]
fn c_and() {
    // C.AND rd'=2 (x10), rs2'=3 (x11)
    let op = decode_c(0x8D6D);
    assert_eq!(
        op,
        Operation::And {
            rd: 10,
            rs1: 10,
            rs2: 11
        }
    );
}

#[test]
fn c_lw() {
    // C.LW rd'=1 (x9), rs1'=2 (x10), offset=4
    let op = decode_c(0x4144);
    assert_eq!(
        op,
        Operation::Lw {
            rd: 9,
            rs1: 10,
            imm: 4
        }
    );
}

#[test]
fn c_sw() {
    // C.SW rs2'=1 (x9), rs1'=2 (x10), offset=4
    let op = decode_c(0xC144);
    assert_eq!(
        op,
        Operation::Sw {
            rs1: 10,
            rs2: 9,
            imm: 4
        }
    );
}

#[test]
fn c_lwsp() {
    // C.LWSP rd=5, offset=8
    // Encoding verified with riscv64-linux-gnu-as (0x42A2, not 0x4282 which is offset=0)
    let op = decode_c(0x42A2);
    assert_eq!(
        op,
        Operation::Lw {
            rd: 5,
            rs1: 2,
            imm: 8
        }
    );
}

#[test]
fn c_lwsp_offset_0() {
    // C.LWSP rd=5, offset=0
    let op = decode_c(0x4282);
    assert_eq!(
        op,
        Operation::Lw {
            rd: 5,
            rs1: 2,
            imm: 0
        }
    );
}

#[test]
fn c_lwsp_offset_60() {
    // C.LWSP rd=5, offset=60 (max for nzuimm[5:2])
    let op = decode_c(0x52F2);
    assert_eq!(
        op,
        Operation::Lw {
            rd: 5,
            rs1: 2,
            imm: 60
        }
    );
}

#[test]
fn c_swsp() {
    // C.SWSP rs2=5, offset=4
    let op = decode_c(0xC216);
    assert_eq!(
        op,
        Operation::Sw {
            rs1: 2,
            rs2: 5,
            imm: 4
        }
    );
}

#[test]
fn c_addi4spn() {
    // C.ADDI4SPN rd'=1 (x9), nzuimm=8
    let op = decode_c(0x0024);
    assert_eq!(
        op,
        Operation::Addi {
            rd: 9,
            rs1: 2,
            imm: 8
        }
    );
}

#[test]
fn c_addi4spn_zero_is_hint_nop() {
    let op = decode_c(0x0004);
    // Per RISC-V spec, C.ADDI4SPN with nzuimm=0 is a HINT (executes as NOP).
    // We implement it as ADDI x0, x0, 0 (write to zero register = no side effect).
    assert!(matches!(
        op,
        Operation::Addi {
            rd: 0,
            rs1: 0,
            imm: 0
        }
    ));
}

// ---- C extension integration test: compressed instruction execution ----
#[test]
fn c_addi_executes_in_cpu() {
    use super::super::bus::Bus;
    use super::super::cpu::RiscvCpu;
    let mut cpu = RiscvCpu::new();
    let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
    cpu.x[5] = 10;
    // Write C.ADDI x5, 6 as a 16-bit value in the low halfword
    // On little-endian: the 16-bit compressed instruction goes in the low 2 bytes
    bus.write_word(0x8000_0000, 0x0299)
        .expect("operation should succeed");
    assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
    assert_eq!(cpu.x[5], 16); // 10 + 6
    assert_eq!(cpu.pc, 0x8000_0002); // PC advanced by 2
}

#[test]
fn c_mv_executes_in_cpu() {
    use super::super::bus::Bus;
    use super::super::cpu::RiscvCpu;
    let mut cpu = RiscvCpu::new();
    let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
    cpu.x[11] = 42;
    // C.MV x10, x11 -> 0x852E
    bus.write_word(0x8000_0000, 0x852E)
        .expect("operation should succeed");
    assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
    assert_eq!(cpu.x[10], 42);
    assert_eq!(cpu.pc, 0x8000_0002);
}

#[test]
fn c_32bit_still_works() {
    use super::super::bus::Bus;
    use super::super::cpu::RiscvCpu;
    let mut cpu = RiscvCpu::new();
    let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
    // ADDI x5, x0, 100 (32-bit): opcode=0010011, rd=5, funct3=000, rs1=0, imm=100
    let word = (100 << 20) | (0 << 15) | (0b000 << 12) | (5 << 7) | 0x13;
    bus.write_word(0x8000_0000, word)
        .expect("operation should succeed");
    assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
    assert_eq!(cpu.x[5], 100);
    assert_eq!(cpu.pc, 0x8000_0004); // PC advanced by 4
}

#[test]
fn c_jal_sets_return_address_pc_plus_2() {
    use super::super::bus::Bus;
    use super::super::cpu::RiscvCpu;
    let mut cpu = RiscvCpu::new();
    let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
    cpu.pc = 0x8000_0000;
    // C.JAL imm=4 -> jal x1, 4. Sets x1 = PC+2 = 0x8000_0002, jumps to PC+4
    bus.write_word(0x8000_0000, 0x2011)
        .expect("operation should succeed");
    assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
    assert_eq!(cpu.x[1], 0x8000_0002); // return addr = PC + 2 (compressed)
    assert_eq!(cpu.pc, 0x8000_0004); // target = PC + 4
}
