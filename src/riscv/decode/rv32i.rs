// riscv/decode/rv32i.rs -- RV32I 32-bit instruction decode (Phase 34)
//
// Decodes a 32-bit instruction word into a fully-resolved Operation enum.
// Every RV32I base instruction is its own variant -- no funct3/funct7
// dispatch at execute time.
//
// See docs/RISCV_HYPERVISOR.md §Instruction Decode.

use super::operation::{Operation, sign_extend};

/// Decode a 32-bit instruction word into a fully-resolved Operation.
pub fn decode(word: u32) -> Operation {
    let opcode = word & 0x7F;
    let rd = ((word >> 7) & 0x1F) as u8;
    let funct3 = ((word >> 12) & 0x7) as u8;
    let rs1 = ((word >> 15) & 0x1F) as u8;
    let rs2 = ((word >> 20) & 0x1F) as u8;
    let funct7 = ((word >> 25) & 0x7F) as u8;

    match opcode {
        0x37 => {
            let imm = word & 0xFFFF_F000;
            Operation::Lui { rd, imm }
        }
        0x17 => {
            let imm = word & 0xFFFF_F000;
            Operation::Auipc { rd, imm }
        }
        0x6F => {
            let imm = jal_imm(word);
            Operation::Jal { rd, imm }
        }
        0x67 => {
            let imm = i_imm(word);
            Operation::Jalr { rd, rs1, imm }
        }
        0x63 => {
            let imm = branch_imm(word);
            match funct3 {
                0b000 => Operation::Beq { rs1, rs2, imm },
                0b001 => Operation::Bne { rs1, rs2, imm },
                0b100 => Operation::Blt { rs1, rs2, imm },
                0b101 => Operation::Bge { rs1, rs2, imm },
                0b110 => Operation::Bltu { rs1, rs2, imm },
                0b111 => Operation::Bgeu { rs1, rs2, imm },
                _ => Operation::Invalid(word),
            }
        }
        0x03 => {
            let imm = i_imm(word);
            match funct3 {
                0b000 => Operation::Lb { rd, rs1, imm },
                0b001 => Operation::Lh { rd, rs1, imm },
                0b010 => Operation::Lw { rd, rs1, imm },
                0b100 => Operation::Lbu { rd, rs1, imm },
                0b101 => Operation::Lhu { rd, rs1, imm },
                _ => Operation::Invalid(word),
            }
        }
        0x23 => {
            let imm = store_imm(word);
            match funct3 {
                0b000 => Operation::Sb { rs1, rs2, imm },
                0b001 => Operation::Sh { rs1, rs2, imm },
                0b010 => Operation::Sw { rs1, rs2, imm },
                _ => Operation::Invalid(word),
            }
        }
        0x33 => match (funct3, funct7) {
            (0b000, 0b0000000) => Operation::Add { rd, rs1, rs2 },
            (0b000, 0b0100000) => Operation::Sub { rd, rs1, rs2 },
            (0b001, 0b0000000) => Operation::Sll { rd, rs1, rs2 },
            (0b010, 0b0000000) => Operation::Slt { rd, rs1, rs2 },
            (0b011, 0b0000000) => Operation::Sltu { rd, rs1, rs2 },
            (0b100, 0b0000000) => Operation::Xor { rd, rs1, rs2 },
            (0b101, 0b0000000) => Operation::Srl { rd, rs1, rs2 },
            (0b101, 0b0100000) => Operation::Sra { rd, rs1, rs2 },
            (0b110, 0b0000000) => Operation::Or { rd, rs1, rs2 },
            (0b111, 0b0000000) => Operation::And { rd, rs1, rs2 },
            // M extension: funct7 = 0b0000001
            (0b000, 0b0000001) => Operation::Mul { rd, rs1, rs2 },
            (0b001, 0b0000001) => Operation::Mulh { rd, rs1, rs2 },
            (0b010, 0b0000001) => Operation::Mulhsu { rd, rs1, rs2 },
            (0b011, 0b0000001) => Operation::Mulhu { rd, rs1, rs2 },
            (0b100, 0b0000001) => Operation::Div { rd, rs1, rs2 },
            (0b101, 0b0000001) => Operation::Divu { rd, rs1, rs2 },
            (0b110, 0b0000001) => Operation::Rem { rd, rs1, rs2 },
            (0b111, 0b0000001) => Operation::Remu { rd, rs1, rs2 },
            _ => Operation::Invalid(word),
        },
        0x13 => {
            let imm = i_imm(word);
            let shamt = ((word >> 20) & 0x1F) as u8;
            let funct7_hi = ((word >> 25) & 0x7F) as u8;
            match funct3 {
                0b000 => Operation::Addi { rd, rs1, imm },
                0b001 => Operation::Slli { rd, rs1, shamt },
                0b010 => Operation::Slti { rd, rs1, imm },
                0b011 => Operation::Sltiu { rd, rs1, imm },
                0b100 => Operation::Xori { rd, rs1, imm },
                0b101 => {
                    if funct7_hi == 0 {
                        Operation::Srli { rd, rs1, shamt }
                    } else {
                        Operation::Srai { rd, rs1, shamt }
                    }
                }
                0b110 => Operation::Ori { rd, rs1, imm },
                0b111 => Operation::Andi { rd, rs1, imm },
                _ => Operation::Invalid(word),
            }
        }
        0x0F => Operation::Fence,
        0x73 => {
            let funct12 = ((word >> 20) & 0xFFF) as u16;
            let csr_addr = (word >> 20) & 0xFFF;
            let uimm = rs1; // for I-type CSR, rs1 field holds uimm
            // SFENCE.VMA: funct3=000, rd=0, funct7=0001001
            if funct3 == 0 && rd == 0 && funct7 == 0b0001001 {
                Operation::SfenceVma { rs1, rs2 }
            } else {
                match (funct3, funct12) {
                (0b000, 0x000) => Operation::Ecall,
                (0b000, 0x001) => Operation::Ebreak,
                (0b000, 0x302) => Operation::Mret,
                (0b000, 0x102) => Operation::Sret,
                // CSR register-register
                (0b001, _) => Operation::Csrrw { rd, rs1, csr: csr_addr },
                (0b010, _) => Operation::Csrrs { rd, rs1, csr: csr_addr },
                (0b011, _) => Operation::Csrrc { rd, rs1, csr: csr_addr },
                // CSR register-immediate
                (0b101, _) => Operation::Csrrwi { rd, uimm, csr: csr_addr },
                (0b110, _) => Operation::Csrrsi { rd, uimm, csr: csr_addr },
                (0b111, _) => Operation::Csrrci { rd, uimm, csr: csr_addr },
                    _ => Operation::Invalid(word),
                }
            }
        }
        // A extension: AMO (opcode 0x2F)
        // Format: bits[31:27]=funct5, bit[26]=aq, bit[25]=rl, rs2, rs1, funct3=010, rd
        0x2F => {
            let aq = (word >> 26) & 1 != 0;
            let rl = (word >> 25) & 1 != 0;
            let funct5 = (funct7 >> 2) & 0x1F; // bits[31:27]
            match funct3 {
                0b010 => {
                    // RV32W atomics -- match on funct5
                    match funct5 {
                        0b00010 => Operation::LrW { rd, rs1, aq, rl },
                        0b00011 => Operation::ScW { rd, rs1, rs2, aq, rl },
                        0b00001 => Operation::AmoswapW { rd, rs1, rs2, aq, rl },
                        0b00000 => Operation::AmoaddW { rd, rs1, rs2, aq, rl },
                        0b00100 => Operation::AmoxorW { rd, rs1, rs2, aq, rl },
                        0b01100 => Operation::AmoandW { rd, rs1, rs2, aq, rl },
                        0b01000 => Operation::AmoorW { rd, rs1, rs2, aq, rl },
                        0b10000 => Operation::AmominW { rd, rs1, rs2, aq, rl },
                        0b10100 => Operation::AmomaxW { rd, rs1, rs2, aq, rl },
                        0b11000 => Operation::AmominuW { rd, rs1, rs2, aq, rl },
                        0b11100 => Operation::AmomaxuW { rd, rs1, rs2, aq, rl },
                        _ => Operation::Invalid(word),
                    }
                }
                _ => Operation::Invalid(word),
            }
        }
        // Floating-point load/store/convert instructions: treat as NOP.
        // The RV32IMAC kernel may contain FP instructions in generic code
        // (e.g., printk context save/restore). Since we don't implement FP,
        // these would trap as illegal instructions and corrupt the stack via
        // the S-mode exception handler. Treating them as NOPs lets the
        // kernel boot past such code.
        0x07 | // FLW, FLD, FLQ (FP load)
        0x27 | // FSW, FSD, FSQ (FP store)
        0x43 | // FMADD
        0x47 | // FMSUB
        0x53 | // FP compute (FADD, FSUB, FMUL, FDIV, etc.)
        0x5B   // FP compute (FSGNJ, FEQ, FLT, FLE, etc.)
        => Operation::Nop,

        _ => Operation::Invalid(word),
    }
}

fn i_imm(word: u32) -> i32 {
    sign_extend(word >> 20, 12)
}

fn branch_imm(word: u32) -> i32 {
    let imm12 = (word >> 31) & 1;
    let imm11 = (word >> 7) & 1;
    let imm10_5 = (word >> 25) & 0x3F;
    let imm4_1 = (word >> 8) & 0xF;
    let imm = (imm12 << 12) | (imm11 << 11) | (imm10_5 << 5) | (imm4_1 << 1);
    sign_extend(imm, 13)
}

fn jal_imm(word: u32) -> i32 {
    let imm20 = (word >> 31) & 1;
    let imm10_1 = (word >> 21) & 0x3FF;
    let imm11 = (word >> 20) & 1;
    let imm19_12 = (word >> 12) & 0xFF;
    let imm = (imm20 << 20) | (imm19_12 << 12) | (imm11 << 11) | (imm10_1 << 1);
    sign_extend(imm, 21)
}

fn store_imm(word: u32) -> i32 {
    let imm4_0 = (word >> 7) & 0x1F;
    let imm11_5 = (word >> 25) & 0x7F;
    let imm = (imm11_5 << 5) | imm4_0;
    sign_extend(imm, 12)
}
