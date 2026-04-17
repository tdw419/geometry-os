// riscv/decode/compressed.rs -- RV32C compressed instruction decode (Phase 38)
//
// See RISC-V Unprivileged ISA v20211203, Chapter 16.
// Compressed instructions are 16 bits. bits[1:0] != 0b11 identifies them.
// decode_c() expands a 16-bit halfword into the same Operation enum.
//
// Dispatch is by bits[1:0] first (the "opcode group"), then funct3 within each group.
// This is critical: the same funct3 value maps to different instructions in different
// bit[1:0] groups.

use super::operation::{Operation, sign_extend};

/// Check if a 16-bit halfword is a compressed instruction.
/// Returns true if bits[1:0] != 0b11 (not a 32-bit instruction).
#[inline]
pub fn is_compressed(halfword: u16) -> bool {
    halfword & 0x3 != 0x3
}

/// Decode a 16-bit compressed instruction into a fully-resolved Operation.
/// Returns an equivalent 32-bit Operation; the caller must advance PC by 2.
pub fn decode_c(halfword: u16) -> Operation {
    let w = halfword as u32;
    let bits01 = w & 0x3;
    let funct3 = ((w >> 13) & 0x7) as u8;

    match bits01 {
        // ---- bits[1:0] = 00: CIW (C.ADDI4SPN), CL (C.LW/C.LD), CS (C.SW/C.SD) ----
        0b00 => match funct3 {
            // C.ADDI4SPN: rd' = sp + nzuimm (nz = non-zero)
            // When nzuimm=0, this is a HINT (NOP), not an illegal instruction.
            // Per RISC-V spec, HINTs execute as NOPs (no side effects).
            0b000 => {
                let rd_p = ((w >> 2) & 0x7) as u8;
                let rd = crd(rd_p);
                let nzuimm = c_addi4spn_imm(w);
                if nzuimm == 0 {
                    // HINT: treat as NOP (write to x0, which is discarded)
                    Operation::Addi { rd: 0, rs1: 0, imm: 0 }
                } else {
                    Operation::Addi { rd, rs1: 2, imm: nzuimm }
                }
            }
            // C.LW: load word from rs1' + offset
            0b010 => {
                let rd_p = ((w >> 2) & 0x7) as u8;
                let rs1_p = ((w >> 7) & 0x7) as u8;
                let rd = crd(rd_p);
                let rs1 = crd(rs1_p);
                let imm = c_lw_imm(w);
                Operation::Lw { rd, rs1, imm }
            }
            // C.SW: store word to rs1' + offset
            0b110 => {
                let rs2_p = ((w >> 2) & 0x7) as u8;
                let rs1_p = ((w >> 7) & 0x7) as u8;
                let rs2 = crd(rs2_p);
                let rs1 = crd(rs1_p);
                let imm = c_sw_imm(w);
                Operation::Sw { rs1, rs2, imm }
            }
            // C.LD (funct3=011), C.SD (funct3=111) are RV64 only
            _ => Operation::Invalid(w),
        },

        // ---- bits[1:0] = 01: CI, CSS, CB, CJ ----
        0b01 => match funct3 {
            // C.NOP (rd=0) / C.ADDI (rd≠0): rd = rd + imm
            0b000 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                let imm = c_addi_imm(w);
                Operation::Addi { rd, rs1: rd, imm }
            }
            // C.JAL: jal x1, offset (RV32C: this is C.JAL, not C.ADDIW)
            0b001 => {
                let imm = c_j_imm(w);
                Operation::Jal { rd: 1, imm }
            }
            // C.LI: rd = imm
            0b010 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                let imm = c_addi_imm(w);
                Operation::Addi { rd, rs1: 0, imm }
            }
            // C.ADDI16SP (rd=2) / C.LUI (rd≠0, rd≠2)
            0b011 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                if rd == 2 {
                    // C.ADDI16SP: sp = sp + imm
                    let imm = c_addi16sp_imm(w);
                    Operation::Addi { rd: 2, rs1: 2, imm }
                } else {
                    // C.LUI: rd = nzimm (must be non-zero)
                    let nzimm = c_lui_imm(w);
                    if nzimm == 0 {
                        Operation::Invalid(w)
                    } else {
                        Operation::Lui { rd, imm: nzimm }
                    }
                }
            }
            // Misc ALU: C.SRLI, C.SRAI, C.ANDI, C.SUB, C.XOR, C.OR, C.AND
            0b100 => {
                let rd_p = ((w >> 7) & 0x7) as u8;
                let rd = crd(rd_p);
                // bits[11:10] determine the operation
                let func = ((w >> 10) & 0x3) as u8;
                match func {
                    0b00 | 0b01 => {
                        // C.SRLI (func=00) or C.SRAI (func=01)
                        let shamt = (((w >> 12) & 1) << 5) | ((w >> 2) & 0x1F);
                        if func == 0b00 {
                            Operation::Srli { rd, rs1: rd, shamt: shamt as u8 }
                        } else {
                            Operation::Srai { rd, rs1: rd, shamt: shamt as u8 }
                        }
                    }
                    0b10 => {
                        // C.ANDI: rd' = rd' & imm
                        let imm = c_alu_imm(w);
                        Operation::Andi { rd, rs1: rd, imm }
                    }
                    0b11 => {
                        // Register ALU: bits[12] is high bit of funct2
                        let rs2_p = ((w >> 2) & 0x7) as u8;
                        let rs2 = crd(rs2_p);
                        let bit12 = ((w >> 12) & 1) as u8;
                        match bit12 {
                            0b0 => {
                                // SUB, XOR, OR, AND
                                let sub_op = ((w >> 5) & 0x3) as u8;
                                match sub_op {
                                    0b00 => Operation::Sub { rd, rs1: rd, rs2 },
                                    0b01 => Operation::Xor { rd, rs1: rd, rs2 },
                                    0b10 => Operation::Or { rd, rs1: rd, rs2 },
                                    0b11 => Operation::And { rd, rs1: rd, rs2 },
                                    _ => Operation::Invalid(w),
                                }
                            }
                            0b1 => {
                                // SUBW, ADDW (RV64/128 only)
                                Operation::Invalid(w)
                            }
                            _ => Operation::Invalid(w),
                        }
                    }
                    _ => Operation::Invalid(w),
                }
            }
            // C.J: jump to offset (unconditional)
            0b101 => {
                let imm = c_j_imm(w);
                Operation::Jal { rd: 0, imm }
            }
            // C.BEQZ: branch if rs1' == zero
            0b110 => {
                let rs1_p = ((w >> 7) & 0x7) as u8;
                let rs1 = crd(rs1_p);
                let imm = c_b_imm(w);
                Operation::Beq { rs1, rs2: 0, imm }
            }
            // C.BNEZ: branch if rs1' != zero
            0b111 => {
                let rs1_p = ((w >> 7) & 0x7) as u8;
                let rs1 = crd(rs1_p);
                let imm = c_b_imm(w);
                Operation::Bne { rs1, rs2: 0, imm }
            }
            _ => Operation::Invalid(w),
        },

        // ---- bits[1:0] = 10: CI, CL, CSS, CR ----
        0b10 => match funct3 {
            // C.SLLI: rd = rd << shamt
            0b000 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                let shamt = (((w >> 12) & 1) << 5) | ((w >> 2) & 0x1F);
                Operation::Slli { rd, rs1: rd, shamt: shamt as u8 }
            }
            // C.LDSP (RV64) / C.LWSP
            0b010 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                let imm = c_lwsp_imm(w);
                Operation::Lw { rd, rs1: 2, imm }
            }
            // C.JR (rs2=0, bit12=0) / C.MV (rs2≠0, bit12=0)
            // C.EBREAK (rd=0, bit12=1) / C.JALR (rd≠0, rs2=0, bit12=1) / C.ADD (rd≠0, rs2≠0, bit12=1)
            0b100 => {
                let rd = ((w >> 7) & 0x1F) as u8;
                let rs2 = ((w >> 2) & 0x1F) as u8;
                let bit12 = ((w >> 12) & 1) as u8;
                if bit12 == 0 {
                    if rs2 == 0 {
                        // C.JR: jalr x0, rd, 0
                        Operation::Jalr { rd: 0, rs1: rd, imm: 0 }
                    } else {
                        // C.MV: add rd, x0, rs2
                        Operation::Add { rd, rs1: 0, rs2 }
                    }
                } else if rd == 0 {
                    // C.EBREAK
                    Operation::Ebreak
                } else if rs2 == 0 {
                    // C.JALR: jalr x1, rd, 0
                    Operation::Jalr { rd: 1, rs1: rd, imm: 0 }
                } else {
                    // C.ADD: add rd, rd, rs2
                    Operation::Add { rd, rs1: rd, rs2 }
                }
            }
            // C.SDSP (RV64) / C.SWSP
            0b110 => {
                let rs2 = ((w >> 2) & 0x1F) as u8;
                let imm = c_swsp_imm(w);
                Operation::Sw { rs1: 2, rs2, imm }
            }
            // funct3=001 (C.LDSP), 011 (C.SQ), 101 (C.FLWSP), 111 (C.FSWSP) are RV64/F only
            _ => Operation::Invalid(w),
        },

        // bits[1:0] = 11: 32-bit instruction, not compressed
        0b11 => Operation::Invalid(w),
        _ => unreachable!(),
    }
}

// ---- C extension helpers ----

/// Map 3-bit compressed register prime to full register number (x8-x15).
fn crd(prime: u8) -> u8 {
    8 + prime
}

/// C.ADDI4SPN immediate: nzimm[5:4|9:6|2|3|8]
fn c_addi4spn_imm(w: u32) -> i32 {
    let imm = (((w >> 5) & 0x1) << 3)
        | (((w >> 6) & 0x1) << 2)
        | (((w >> 7) & 0x1) << 8)
        | (((w >> 10) & 0x7) << 5)
        | (((w >> 11) & 0x1) << 4);
    imm as i32
}

/// C.ADDI / C.LI / C.ADDIW immediate: imm[5:4|12|2:6|3] (sign-extended 6-bit)
fn c_addi_imm(w: u32) -> i32 {
    let raw = (((w >> 12) & 0x1) << 5)
        | ((w >> 2) & 0x1F);
    sign_extend(raw, 6)
}

/// C.ADDI16SP immediate: nzimm[9|4|6|8|7|5] (sign-extended 10-bit)
/// Encoding per RISC-V Unprivileged ISA v20240411 Table 16.3:
///   bit[12] → nzimm[9]
///   bit[6]  → nzimm[4]
///   bit[5]  → nzimm[6]
///   bit[4]  → nzimm[8]
///   bit[3]  → nzimm[7]
///   bit[2]  → nzimm[5]
fn c_addi16sp_imm(w: u32) -> i32 {
    let raw = (((w >> 12) & 0x1) << 9)
        | (((w >> 6) & 0x1) << 4)
        | (((w >> 5) & 0x1) << 6)
        | (((w >> 4) & 0x1) << 8)
        | (((w >> 3) & 0x1) << 7)
        | (((w >> 2) & 0x1) << 5);
    sign_extend(raw, 10)
}

/// C.LUI immediate: nzimm[17|12:2] (not sign-extended, 32-bit)
fn c_lui_imm(w: u32) -> u32 {
    
    (((w >> 12) & 0x1) << 17)
        | (((w >> 2) & 0x1F) << 12)
}

/// C.ANDI immediate: imm[5:4|12|2:6|3] (same encoding as C.ADDI, sign-extended 6-bit)
fn c_alu_imm(w: u32) -> i32 {
    c_addi_imm(w)
}

/// C.LW immediate: offset[6]=inst[5], offset[5]=inst[12], offset[4]=inst[11], offset[3]=inst[10], offset[2]=inst[6]
/// Derived from GNU assembler reference encodings. Range: 0-124, word-aligned.
fn c_lw_imm(w: u32) -> i32 {
    let imm = (((w >> 5) & 0x1) << 6)
        | (((w >> 12) & 0x1) << 5)
        | (((w >> 11) & 0x1) << 4)
        | (((w >> 10) & 0x1) << 3)
        | (((w >> 6) & 0x1) << 2);
    imm as i32
}

/// C.SW immediate: same encoding as C.LW
fn c_sw_imm(w: u32) -> i32 {
    c_lw_imm(w)
}

/// C.BEQZ/C.BNEZ immediate (sign-extended 9-bit).
/// RISC-V spec Table 16.4:
///   imm[8]   = inst[12]
///   imm[7:6] = inst[6:5]
///   imm[5]   = inst[2]
///   imm[4:3] = inst[11:10]
///   imm[2:1] = inst[4:3]
fn c_b_imm(w: u32) -> i32 {
    let raw = (((w >> 12) & 0x1) << 8)
        | (((w >> 5) & 0x3) << 6)
        | (((w >> 2) & 0x1) << 5)
        | (((w >> 10) & 0x3) << 3)
        | (((w >> 3) & 0x3) << 1);
    sign_extend(raw, 9)
}

/// C.J / C.JAL immediate: imm[11|4|9:8|10|6|7|3:1|5] (sign-extended 12-bit)
fn c_j_imm(w: u32) -> i32 {
    let raw = (((w >> 12) & 0x1) << 11)
        | (((w >> 11) & 0x1) << 4)
        | (((w >> 9) & 0x3) << 8)
        | (((w >> 8) & 0x1) << 10)
        | (((w >> 7) & 0x1) << 6)
        | (((w >> 6) & 0x1) << 7)
        | (((w >> 3) & 0x7) << 1)
        | (((w >> 2) & 0x1) << 5);
    sign_extend(raw, 12)
}

/// C.LWSP immediate: offset[7]=inst[3], offset[6]=inst[2], offset[5]=inst[12],
/// offset[4]=inst[6], offset[3]=inst[5], offset[2]=inst[4]
/// Derived from GNU assembler reference encodings. Range: 0-252, word-aligned.
fn c_lwsp_imm(w: u32) -> i32 {
    let imm = (((w >> 3) & 0x1) << 7)
        | (((w >> 2) & 0x1) << 6)
        | (((w >> 12) & 0x1) << 5)
        | (((w >> 6) & 0x1) << 4)
        | (((w >> 5) & 0x1) << 3)
        | (((w >> 4) & 0x1) << 2);
    imm as i32
}

/// C.SWSP immediate: offset[7]=inst[8], offset[6]=inst[7], offset[5]=inst[12],
/// offset[4]=inst[11], offset[3]=inst[10], offset[2]=inst[9]
/// Derived from GNU assembler reference encodings. Range: 0-252, word-aligned.
fn c_swsp_imm(w: u32) -> i32 {
    let imm = (((w >> 8) & 0x1) << 7)
        | (((w >> 7) & 0x1) << 6)
        | (((w >> 12) & 0x1) << 5)
        | (((w >> 11) & 0x1) << 4)
        | (((w >> 10) & 0x1) << 3)
        | (((w >> 9) & 0x1) << 2);
    imm as i32
}
