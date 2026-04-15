// riscv/decode.rs -- RV32I instruction decode (Phase 34)
//
// Decodes a 32-bit instruction word into a fully-resolved Operation enum.
// Every RV32I base instruction is its own variant -- no funct3/funct7
// dispatch at execute time.
//
// See docs/RISCV_HYPERVISOR.md §Instruction Decode.

/// Fully-decoded RV32I operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    // -- R-type ALU --
    Add { rd: u8, rs1: u8, rs2: u8 },
    Sub { rd: u8, rs1: u8, rs2: u8 },
    Sll { rd: u8, rs1: u8, rs2: u8 },
    Slt { rd: u8, rs1: u8, rs2: u8 },
    Sltu { rd: u8, rs1: u8, rs2: u8 },
    Xor { rd: u8, rs1: u8, rs2: u8 },
    Srl { rd: u8, rs1: u8, rs2: u8 },
    Sra { rd: u8, rs1: u8, rs2: u8 },
    Or { rd: u8, rs1: u8, rs2: u8 },
    And { rd: u8, rs1: u8, rs2: u8 },

    // -- M extension (multiply/divide) --
    Mul { rd: u8, rs1: u8, rs2: u8 },
    Mulh { rd: u8, rs1: u8, rs2: u8 },
    Mulhu { rd: u8, rs1: u8, rs2: u8 },
    Mulhsu { rd: u8, rs1: u8, rs2: u8 },
    Div { rd: u8, rs1: u8, rs2: u8 },
    Divu { rd: u8, rs1: u8, rs2: u8 },
    Rem { rd: u8, rs1: u8, rs2: u8 },
    Remu { rd: u8, rs1: u8, rs2: u8 },

    // -- A extension (atomics) --
    /// LR.W: Load Reserved. Sets reservation on address in rs1.
    LrW { rd: u8, rs1: u8, aq: bool, rl: bool },
    /// SC.W: Store Conditional. Succeeds only if reservation holds.
    ScW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOSWAP.W: Atomically swap rs2 into memory, return old value.
    AmoswapW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOADD.W: Atomically add rs2 to memory, return old value.
    AmoaddW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOXOR.W: Atomically XOR rs2 into memory, return old value.
    AmoxorW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOAND.W: Atomically AND rs2 into memory, return old value.
    AmoandW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOOR.W: Atomically OR rs2 into memory, return old value.
    AmoorW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOMIN.W: Atomically min(rs2, mem) into memory, return old value (signed).
    AmominW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOMAX.W: Atomically max(rs2, mem) into memory, return old value (signed).
    AmomaxW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOMINU.W: Atomically min(rs2, mem) into memory, return old value (unsigned).
    AmominuW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },
    /// AMOMAXU.W: Atomically max(rs2, mem) into memory, return old value (unsigned).
    AmomaxuW { rd: u8, rs1: u8, rs2: u8, aq: bool, rl: bool },

    // -- I-type ALU --
    Addi { rd: u8, rs1: u8, imm: i32 },
    Slti { rd: u8, rs1: u8, imm: i32 },
    Sltiu { rd: u8, rs1: u8, imm: i32 },
    Xori { rd: u8, rs1: u8, imm: i32 },
    Ori { rd: u8, rs1: u8, imm: i32 },
    Andi { rd: u8, rs1: u8, imm: i32 },
    Slli { rd: u8, rs1: u8, shamt: u8 },
    Srli { rd: u8, rs1: u8, shamt: u8 },
    Srai { rd: u8, rs1: u8, shamt: u8 },

    // -- Load --
    Lb { rd: u8, rs1: u8, imm: i32 },
    Lh { rd: u8, rs1: u8, imm: i32 },
    Lw { rd: u8, rs1: u8, imm: i32 },
    Lbu { rd: u8, rs1: u8, imm: i32 },
    Lhu { rd: u8, rs1: u8, imm: i32 },

    // -- Store --
    Sb { rs1: u8, rs2: u8, imm: i32 },
    Sh { rs1: u8, rs2: u8, imm: i32 },
    Sw { rs1: u8, rs2: u8, imm: i32 },

    // -- Branch --
    Beq { rs1: u8, rs2: u8, imm: i32 },
    Bne { rs1: u8, rs2: u8, imm: i32 },
    Blt { rs1: u8, rs2: u8, imm: i32 },
    Bge { rs1: u8, rs2: u8, imm: i32 },
    Bltu { rs1: u8, rs2: u8, imm: i32 },
    Bgeu { rs1: u8, rs2: u8, imm: i32 },

    // -- Upper immediate --
    Lui { rd: u8, imm: u32 },
    Auipc { rd: u8, imm: u32 },

    // -- Jump --
    Jal { rd: u8, imm: i32 },
    Jalr { rd: u8, rs1: u8, imm: i32 },

    // -- System --
    Ecall,
    Ebreak,
    Fence,
    Mret,
    Sret,
    SfenceVma { rs1: u8, rs2: u8 },

    // -- CSR --
    Csrrw { rd: u8, rs1: u8, csr: u32 },
    Csrrs { rd: u8, rs1: u8, csr: u32 },
    Csrrc { rd: u8, rs1: u8, csr: u32 },
    Csrrwi { rd: u8, uimm: u8, csr: u32 },
    Csrrsi { rd: u8, uimm: u8, csr: u32 },
    Csrrci { rd: u8, uimm: u8, csr: u32 },

    // -- Unknown --
    Invalid(u32),
}

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

fn sign_extend(val: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (val << shift) as i32 >> shift
}

// ---- C extension (RV32C compressed instructions) ----
// See RISC-V Unprivileged ISA v20211203, Chapter 16.
// Compressed instructions are 16 bits. bits[1:0] != 0b11 identifies them.
// decode_c() expands a 16-bit halfword into the same Operation enum.
//
// Dispatch is by bits[1:0] first (the "opcode group"), then funct3 within each group.
// This is critical: the same funct3 value maps to different instructions in different
// bit[1:0] groups.

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
            0b000 => {
                let rd_p = ((w >> 2) & 0x7) as u8;
                let rd = crd(rd_p);
                let nzuimm = c_addi4spn_imm(w);
                if nzuimm == 0 {
                    Operation::Invalid(w)
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
                } else {
                    if rd == 0 {
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
    let imm = (((w >> 12) & 0x1) << 17)
        | (((w >> 2) & 0x1F) << 12);
    imm
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

/// C.BEQZ/C.BNEZ immediate: offset[8|4:3|12|2:6|5|1:0|11] (sign-extended 9-bit)
fn c_b_imm(w: u32) -> i32 {
    let raw = (((w >> 12) & 0x1) << 8)
        | (((w >> 10) & 0x3) << 3)
        | (((w >> 5) & 0x3) << 1)
        | (((w >> 3) & 0x3) << 5)
        | (((w >> 2) & 0x1) << 7)
        | (((w >> 7) & 0x1) << 4)
        | (((w >> 6) & 0x1) << 6);
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

#[cfg(test)]
mod tests {
    use super::*;

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
        ((imm >> 12) << 31) | (((imm >> 5) & 0x3F) << 25) | (rs2 << 20) | (rs1 << 15)
            | (funct3 << 12) | (((imm >> 1) & 0xF) << 8) | (((imm >> 11) & 1) << 7) | 0x63
    }

    // R-type
    #[test] fn decode_add() { assert_eq!(decode(encode_r(0,3,1,0,5)), Operation::Add{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_sub() { assert_eq!(decode(encode_r(0x20,3,1,0,5)), Operation::Sub{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_sll() { assert_eq!(decode(encode_r(0,3,1,1,5)), Operation::Sll{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_slt() { assert_eq!(decode(encode_r(0,3,1,2,5)), Operation::Slt{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_sltu() { assert_eq!(decode(encode_r(0,3,1,3,5)), Operation::Sltu{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_xor() { assert_eq!(decode(encode_r(0,3,1,4,5)), Operation::Xor{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_srl() { assert_eq!(decode(encode_r(0,3,1,5,5)), Operation::Srl{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_sra() { assert_eq!(decode(encode_r(0x20,3,1,5,5)), Operation::Sra{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_or()  { assert_eq!(decode(encode_r(0,3,1,6,5)), Operation::Or{rd:5,rs1:1,rs2:3}); }
    #[test] fn decode_and() { assert_eq!(decode(encode_r(0,3,1,7,5)), Operation::And{rd:5,rs1:1,rs2:3}); }

    // I-type ALU
    #[test] fn decode_addi() { assert_eq!(decode(encode_i(42,2,0,1,0x13)), Operation::Addi{rd:1,rs1:2,imm:42}); }
    #[test] fn decode_addi_neg() { assert_eq!(decode(encode_i(0xFFF,2,0,1,0x13)), Operation::Addi{rd:1,rs1:2,imm:-1}); }
    #[test] fn decode_slti() { assert_eq!(decode(encode_i(10,3,2,7,0x13)), Operation::Slti{rd:7,rs1:3,imm:10}); }
    #[test] fn decode_sltiu() { assert_eq!(decode(encode_i(10,3,3,7,0x13)), Operation::Sltiu{rd:7,rs1:3,imm:10}); }
    #[test] fn decode_xori() { assert_eq!(decode(encode_i(0xFF,4,4,8,0x13)), Operation::Xori{rd:8,rs1:4,imm:0xFF}); }
    #[test] fn decode_ori() { assert_eq!(decode(encode_i(0xFF,4,6,8,0x13)), Operation::Ori{rd:8,rs1:4,imm:0xFF}); }
    #[test] fn decode_andi() { assert_eq!(decode(encode_i(0xFF,4,7,8,0x13)), Operation::Andi{rd:8,rs1:4,imm:0xFF}); }
    #[test] fn decode_slli() {
        let w = (0u32<<25)|(5u32<<20)|(2u32<<15)|(1u32<<12)|(1u32<<7)|0x13;
        assert_eq!(decode(w), Operation::Slli{rd:1,rs1:2,shamt:5});
    }
    #[test] fn decode_srli() {
        let w = (0u32<<25)|(7u32<<20)|(3u32<<15)|(5u32<<12)|(1u32<<7)|0x13;
        assert_eq!(decode(w), Operation::Srli{rd:1,rs1:3,shamt:7});
    }
    #[test] fn decode_srai() {
        let w = (0x20u32<<25)|(7u32<<20)|(3u32<<15)|(5u32<<12)|(1u32<<7)|0x13;
        assert_eq!(decode(w), Operation::Srai{rd:1,rs1:3,shamt:7});
    }

    // Load
    #[test] fn decode_lb()  { assert_eq!(decode(encode_i(8,1,0,3,0x03)), Operation::Lb{rd:3,rs1:1,imm:8}); }
    #[test] fn decode_lh()  { assert_eq!(decode(encode_i(8,1,1,3,0x03)), Operation::Lh{rd:3,rs1:1,imm:8}); }
    #[test] fn decode_lw()  { assert_eq!(decode(encode_i(8,1,2,3,0x03)), Operation::Lw{rd:3,rs1:1,imm:8}); }
    #[test] fn decode_lbu() { assert_eq!(decode(encode_i(8,1,4,3,0x03)), Operation::Lbu{rd:3,rs1:1,imm:8}); }
    #[test] fn decode_lhu() { assert_eq!(decode(encode_i(8,1,5,3,0x03)), Operation::Lhu{rd:3,rs1:1,imm:8}); }

    // Store
    #[test] fn decode_sb() { assert_eq!(decode(encode_s(4,5,1,0)), Operation::Sb{rs1:1,rs2:5,imm:4}); }
    #[test] fn decode_sh() { assert_eq!(decode(encode_s(4,5,1,1)), Operation::Sh{rs1:1,rs2:5,imm:4}); }
    #[test] fn decode_sw() { assert_eq!(decode(encode_s(4,5,1,2)), Operation::Sw{rs1:1,rs2:5,imm:4}); }

    // Branch
    #[test] fn decode_beq()  { assert_eq!(decode(encode_b(8,2,1,0)), Operation::Beq{rs1:1,rs2:2,imm:8}); }
    #[test] fn decode_bne()  { assert_eq!(decode(encode_b(8,2,1,1)), Operation::Bne{rs1:1,rs2:2,imm:8}); }
    #[test] fn decode_blt()  { assert_eq!(decode(encode_b(8,2,1,4)), Operation::Blt{rs1:1,rs2:2,imm:8}); }
    #[test] fn decode_bge()  { assert_eq!(decode(encode_b(8,2,1,5)), Operation::Bge{rs1:1,rs2:2,imm:8}); }
    #[test] fn decode_bltu() { assert_eq!(decode(encode_b(8,2,1,6)), Operation::Bltu{rs1:1,rs2:2,imm:8}); }
    #[test] fn decode_bgeu() { assert_eq!(decode(encode_b(8,2,1,7)), Operation::Bgeu{rs1:1,rs2:2,imm:8}); }

    // Upper
    #[test] fn decode_lui() { assert_eq!(decode(0x123452B7), Operation::Lui{rd:5,imm:0x12345000}); }
    #[test] fn decode_auipc() {
        let w = (0x12345u32 << 12) | (5u32 << 7) | 0x17;
        assert_eq!(decode(w), Operation::Auipc{rd:5,imm:0x12345000});
    }

    // Jump
    #[test] fn decode_jal() {
        let w = (0u32<<31)|(4u32<<21)|(0u32<<20)|(0u32<<12)|(1u32<<7)|0x6F;
        assert_eq!(decode(w), Operation::Jal{rd:1,imm:8});
    }
    #[test] fn decode_jalr() { assert_eq!(decode(encode_i(0,1,0,5,0x67)), Operation::Jalr{rd:5,rs1:1,imm:0}); }

    // System
    #[test] fn decode_ecall()  { assert_eq!(decode(0x00000073), Operation::Ecall); }
    #[test] fn decode_ebreak() { assert_eq!(decode(0x00100073), Operation::Ebreak); }
    #[test] fn decode_fence()  { assert_eq!(decode(0x0FF0000F), Operation::Fence); }

    // Invalid
    #[test] fn decode_invalid_zero() { assert_eq!(decode(0), Operation::Invalid(0)); }
    #[test] fn decode_rd_zero_valid() {
        assert_eq!(decode(encode_r(0,2,1,0,0)), Operation::Add{rd:0,rs1:1,rs2:2});
    }
    #[test] fn decode_branch_neg_offset() {
        // BEQ with -16 offset
        let imm_raw: u32 = 0x1FF0; // 13-bit representation of -16
        let w = encode_b(imm_raw, 2, 1, 0);
        assert_eq!(decode(w), Operation::Beq{rs1:1,rs2:2,imm:-16});
    }

    // CSR instructions
    fn encode_csr(funct3: u32, rd: u8, rs1_uimm: u8, csr: u32) -> u32 {
        ((csr & 0xFFF) << 20) | ((rs1_uimm as u32) << 15) | (funct3 << 12) | ((rd as u32) << 7) | 0x73
    }
    #[test] fn decode_csrrw() { assert_eq!(decode(encode_csr(0b001, 3, 5, 0x300)), Operation::Csrrw{rd:3,rs1:5,csr:0x300}); }
    #[test] fn decode_csrrs() { assert_eq!(decode(encode_csr(0b010, 3, 5, 0x305)), Operation::Csrrs{rd:3,rs1:5,csr:0x305}); }
    #[test] fn decode_csrrc() { assert_eq!(decode(encode_csr(0b011, 3, 5, 0x341)), Operation::Csrrc{rd:3,rs1:5,csr:0x341}); }
    #[test] fn decode_csrrwi() { assert_eq!(decode(encode_csr(0b101, 3, 7, 0x342)), Operation::Csrrwi{rd:3,uimm:7,csr:0x342}); }
    #[test] fn decode_csrrsi() { assert_eq!(decode(encode_csr(0b110, 3, 15, 0x100)), Operation::Csrrsi{rd:3,uimm:15,csr:0x100}); }
    #[test] fn decode_csrrci() { assert_eq!(decode(encode_csr(0b111, 3, 31, 0x180)), Operation::Csrrci{rd:3,uimm:31,csr:0x180}); }

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
        assert_eq!(op, Operation::Addi { rd: 0, rs1: 0, imm: 0 });
    }

    #[test]
    fn c_addi() {
        // C.ADDI rd=5, imm=6
        let op = decode_c(0x0299);
        assert_eq!(op, Operation::Addi { rd: 5, rs1: 5, imm: 6 });
    }

    #[test]
    fn c_addi_negative() {
        // C.ADDI rd=5, imm=-1
        let op = decode_c(0x12FD);
        assert_eq!(op, Operation::Addi { rd: 5, rs1: 5, imm: -1 });
    }

    #[test]
    fn c_li() {
        // C.LI rd=3, imm=10
        let op = decode_c(0x41A9);
        assert_eq!(op, Operation::Addi { rd: 3, rs1: 0, imm: 10 });
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
        assert_eq!(op, Operation::Addi { rd: 2, rs1: 2, imm: 16 });
    }

    #[test]
    fn c_mv() {
        // C.MV rd=10, rs2=11
        let op = decode_c(0x852E);
        assert_eq!(op, Operation::Add { rd: 10, rs1: 0, rs2: 11 });
    }

    #[test]
    fn c_add() {
        // C.ADD rd=10, rs2=11
        let op = decode_c(0x952E);
        assert_eq!(op, Operation::Add { rd: 10, rs1: 10, rs2: 11 });
    }

    #[test]
    fn c_jr() {
        // C.JR rs1=5
        let op = decode_c(0x8282);
        assert_eq!(op, Operation::Jalr { rd: 0, rs1: 5, imm: 0 });
    }

    #[test]
    fn c_jalr() {
        // C.JALR rs1=5 -> jalr x1, x5, 0
        let op = decode_c(0x9282);
        assert_eq!(op, Operation::Jalr { rd: 1, rs1: 5, imm: 0 });
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
        assert_eq!(op, Operation::Beq { rs1: 10, rs2: 0, imm: 0 });
    }

    #[test]
    fn c_bnez() {
        // C.BNEZ rs1'=0 (x8), imm=0
        let op = decode_c(0xE001);
        assert_eq!(op, Operation::Bne { rs1: 8, rs2: 0, imm: 0 });
    }

    #[test]
    fn c_slli() {
        // C.SLLI rd=5, shamt=3
        let op = decode_c(0x028E);
        assert_eq!(op, Operation::Slli { rd: 5, rs1: 5, shamt: 3 });
    }

    #[test]
    fn c_srli() {
        // C.SRLI rd'=2 (x10), shamt=5
        let op = decode_c(0x8115);
        assert_eq!(op, Operation::Srli { rd: 10, rs1: 10, shamt: 5 });
    }

    #[test]
    fn c_srai() {
        // C.SRAI rd'=2 (x10), shamt=5
        let op = decode_c(0x8515);
        assert_eq!(op, Operation::Srai { rd: 10, rs1: 10, shamt: 5 });
    }

    #[test]
    fn c_andi() {
        // C.ANDI rd'=2 (x10), imm=7
        let op = decode_c(0x891D);
        assert_eq!(op, Operation::Andi { rd: 10, rs1: 10, imm: 7 });
    }

    #[test]
    fn c_sub() {
        // C.SUB rd'=2 (x10), rs2'=3 (x11)
        let op = decode_c(0x8D0D);
        assert_eq!(op, Operation::Sub { rd: 10, rs1: 10, rs2: 11 });
    }

    #[test]
    fn c_xor() {
        // C.XOR rd'=2 (x10), rs2'=3 (x11)
        let op = decode_c(0x8D2D);
        assert_eq!(op, Operation::Xor { rd: 10, rs1: 10, rs2: 11 });
    }

    #[test]
    fn c_or() {
        // C.OR rd'=2 (x10), rs2'=3 (x11)
        let op = decode_c(0x8D4D);
        assert_eq!(op, Operation::Or { rd: 10, rs1: 10, rs2: 11 });
    }

    #[test]
    fn c_and() {
        // C.AND rd'=2 (x10), rs2'=3 (x11)
        let op = decode_c(0x8D6D);
        assert_eq!(op, Operation::And { rd: 10, rs1: 10, rs2: 11 });
    }

    #[test]
    fn c_lw() {
        // C.LW rd'=1 (x9), rs1'=2 (x10), offset=4
        let op = decode_c(0x4144);
        assert_eq!(op, Operation::Lw { rd: 9, rs1: 10, imm: 4 });
    }

    #[test]
    fn c_sw() {
        // C.SW rs2'=1 (x9), rs1'=2 (x10), offset=4
        let op = decode_c(0xC144);
        assert_eq!(op, Operation::Sw { rs1: 10, rs2: 9, imm: 4 });
    }

    #[test]
    fn c_lwsp() {
        // C.LWSP rd=5, offset=8
        // Encoding verified with riscv64-linux-gnu-as (0x42A2, not 0x4282 which is offset=0)
        let op = decode_c(0x42A2);
        assert_eq!(op, Operation::Lw { rd: 5, rs1: 2, imm: 8 });
    }

    #[test]
    fn c_lwsp_offset_0() {
        // C.LWSP rd=5, offset=0
        let op = decode_c(0x4282);
        assert_eq!(op, Operation::Lw { rd: 5, rs1: 2, imm: 0 });
    }

    #[test]
    fn c_lwsp_offset_60() {
        // C.LWSP rd=5, offset=60 (max for nzuimm[5:2])
        let op = decode_c(0x52F2);
        assert_eq!(op, Operation::Lw { rd: 5, rs1: 2, imm: 60 });
    }

    #[test]
    fn c_swsp() {
        // C.SWSP rs2=5, offset=4
        let op = decode_c(0xC216);
        assert_eq!(op, Operation::Sw { rs1: 2, rs2: 5, imm: 4 });
    }

    #[test]
    fn c_addi4spn() {
        // C.ADDI4SPN rd'=1 (x9), nzuimm=8
        let op = decode_c(0x0024);
        assert_eq!(op, Operation::Addi { rd: 9, rs1: 2, imm: 8 });
    }

    #[test]
    fn c_addi4spn_zero_is_invalid() {
        let op = decode_c(0x0004);
        assert!(matches!(op, Operation::Invalid(_)));
    }

    // ---- C extension integration test: compressed instruction execution ----
    #[test]
    fn c_addi_executes_in_cpu() {
        use super::super::cpu::RiscvCpu;
        use super::super::bus::Bus;
        let mut cpu = RiscvCpu::new();
        let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
        cpu.x[5] = 10;
        // Write C.ADDI x5, 6 as a 16-bit value in the low halfword
        // On little-endian: the 16-bit compressed instruction goes in the low 2 bytes
        bus.write_word(0x8000_0000, 0x0299).unwrap();
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
        bus.write_word(0x8000_0000, 0x852E).unwrap();
        assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
        assert_eq!(cpu.x[10], 42);
        assert_eq!(cpu.pc, 0x8000_0002);
    }

    #[test]
    fn c_32bit_still_works() {
        use super::super::cpu::RiscvCpu;
        use super::super::bus::Bus;
        let mut cpu = RiscvCpu::new();
        let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
        // ADDI x5, x0, 100 (32-bit): opcode=0010011, rd=5, funct3=000, rs1=0, imm=100
        let word = (100 << 20) | (0 << 15) | (0b000 << 12) | (5 << 7) | 0x13;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
        assert_eq!(cpu.x[5], 100);
        assert_eq!(cpu.pc, 0x8000_0004); // PC advanced by 4
    }

    #[test]
    fn c_jal_sets_return_address_pc_plus_2() {
        use super::super::cpu::RiscvCpu;
        use super::super::bus::Bus;
        let mut cpu = RiscvCpu::new();
        let mut bus = Bus::new(0x8000_0000, 1024 * 1024);
        cpu.pc = 0x8000_0000;
        // C.JAL imm=4 -> jal x1, 4. Sets x1 = PC+2 = 0x8000_0002, jumps to PC+4
        bus.write_word(0x8000_0000, 0x2011).unwrap();
        assert_eq!(cpu.step(&mut bus), super::super::cpu::StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_0002); // return addr = PC + 2 (compressed)
        assert_eq!(cpu.pc, 0x8000_0004); // target = PC + 4
    }
}
