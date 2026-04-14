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
}
