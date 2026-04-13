// riscv/decode.rs -- RV32I instruction decode (Phase 34 stub)
//
// Decodes a 32-bit instruction word into an operation.
// See docs/RISCV_HYPERVISOR.md §Instruction Decode.

/// Decoded RISC-V instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
    /// LUI rd, imm
    Lui { rd: u8, imm: u32 },
    /// AUIPC rd, imm
    Auipc { rd: u8, imm: u32 },
    /// JAL rd, imm
    Jal { rd: u8, imm: i32 },
    /// JALR rd, rs1, imm
    Jalr { rd: u8, rs1: u8, imm: i32 },
    /// Branch (BEQ, BNE, BLT, BGE, BLTU, BGEU)
    Branch { rs1: u8, rs2: u8, imm: i32, funct3: u8 },
    /// Load (LB, LH, LW, LBU, LHU)
    Load { rd: u8, rs1: u8, imm: i32, funct3: u8 },
    /// Store (SB, SH, SW)
    Store { rs1: u8, rs2: u8, imm: i32, funct3: u8 },
    /// R-type ALU (ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND)
    RAlu { rd: u8, rs1: u8, rs2: u8, funct3: u8, funct7: u8 },
    /// I-type ALU (ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI)
    IAlu { rd: u8, rs1: u8, imm: i32, funct3: u8 },
    /// FENCE (treated as NOP)
    Fence,
    /// SYSTEM (ECALL, EBREAK, CSR*)
    System { funct12: u16, rs1: u8, rd: u8, funct3: u8 },
    /// Unrecognized instruction
    Invalid(u32),
}

/// Decode a 32-bit instruction word.
pub fn decode(word: u32) -> Instruction {
    let opcode = word & 0x7F;

    let rd = ((word >> 7) & 0x1F) as u8;
    let funct3 = ((word >> 12) & 0x7) as u8;
    let rs1 = ((word >> 15) & 0x1F) as u8;
    let rs2 = ((word >> 20) & 0x1F) as u8;
    let funct7 = ((word >> 25) & 0x7F) as u8;

    match opcode {
        0x37 => {
            // LUI
            let imm = word & 0xFFFF_F000;
            Instruction::Lui { rd, imm }
        }
        0x17 => {
            // AUIPC
            let imm = word & 0xFFFF_F000;
            Instruction::Auipc { rd, imm }
        }
        0x6F => {
            // JAL
            let imm = jal_imm(word);
            Instruction::Jal { rd, imm }
        }
        0x67 => {
            // JALR
            let imm = ((word >> 20) as i32) << 20 >> 20;
            Instruction::Jalr { rd, rs1, imm }
        }
        0x63 => {
            // Branch
            let imm = branch_imm(word);
            Instruction::Branch { rs1, rs2, imm, funct3 }
        }
        0x03 => {
            // Load
            let imm = ((word >> 20) as i32) << 20 >> 20;
            Instruction::Load { rd, rs1, imm, funct3 }
        }
        0x23 => {
            // Store
            let imm = store_imm(word);
            Instruction::Store { rs1, rs2, imm, funct3 }
        }
        0x33 => {
            // R-type ALU
            Instruction::RAlu { rd, rs1, rs2, funct3, funct7 }
        }
        0x13 => {
            // I-type ALU
            let imm = ((word >> 20) as i32) << 20 >> 20;
            Instruction::IAlu { rd, rs1, imm, funct3 }
        }
        0x0F => Instruction::Fence,
        0x73 => {
            let funct12 = ((word >> 20) & 0xFFF) as u16;
            Instruction::System { funct12, rs1, rd, funct3 }
        }
        _ => Instruction::Invalid(word),
    }
}

/// Extract B-type immediate.
fn branch_imm(word: u32) -> i32 {
    let imm12 = (word >> 31) & 1;
    let imm11 = (word >> 7) & 1;
    let imm10_5 = (word >> 25) & 0x3F;
    let imm4_1 = (word >> 8) & 0xF;
    let imm = (imm12 << 12) | (imm11 << 11) | (imm10_5 << 5) | (imm4_1 << 1);
    sign_extend(imm, 13)
}

/// Extract J-type immediate.
fn jal_imm(word: u32) -> i32 {
    let imm20 = (word >> 31) & 1;
    let imm10_1 = (word >> 21) & 0x3FF;
    let imm11 = (word >> 20) & 1;
    let imm19_12 = (word >> 12) & 0xFF;
    let imm = (imm20 << 20) | (imm19_12 << 12) | (imm11 << 11) | (imm10_1 << 1);
    sign_extend(imm, 21)
}

/// Extract S-type immediate.
fn store_imm(word: u32) -> i32 {
    let imm4_0 = (word >> 7) & 0x1F;
    let imm11_5 = (word >> 25) & 0x7F;
    let imm = (imm11_5 << 5) | imm4_0;
    sign_extend(imm, 12)
}

/// Sign-extend a value from `bits` bits to i32.
fn sign_extend(val: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (val << shift) as i32 >> shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_lui() {
        // LUI x5, 0x12345000
        let word = 0x123452B7;
        let instr = decode(word);
        assert!(matches!(instr, Instruction::Lui { rd: 5, .. }));
    }

    #[test]
    fn decode_addi() {
        // ADDI x1, x2, 42
        let word = (42u32 << 20) | (2u32 << 15) | (1u32 << 7) | 0x13;
        let instr = decode(word);
        assert!(matches!(instr, Instruction::IAlu { rd: 1, rs1: 2, imm: 42, .. }));
    }

    #[test]
    fn decode_invalid() {
        let instr = decode(0);
        assert!(matches!(instr, Instruction::Invalid(_)));
    }
}
