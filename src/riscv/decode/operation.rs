// riscv/decode/operation.rs -- RV32I operation enum (Phase 34)
//
// Fully-decoded RV32I operation. Every base instruction is its own variant --
// no funct3/funct7 dispatch at execute time.

/// Fully-decoded RV32I operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    // -- R-type ALU --
    Add {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Sub {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Sll {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Slt {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Sltu {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Xor {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Srl {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Sra {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Or {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    And {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },

    // -- M extension (multiply/divide) --
    Mul {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Mulh {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Mulhu {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Mulhsu {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Div {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Divu {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Rem {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },
    Remu {
        rd: u8,
        rs1: u8,
        rs2: u8,
    },

    // -- A extension (atomics) --
    /// LR.W: Load Reserved. Sets reservation on address in rs1.
    LrW {
        rd: u8,
        rs1: u8,
        aq: bool,
        rl: bool,
    },
    /// SC.W: Store Conditional. Succeeds only if reservation holds.
    ScW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOSWAP.W: Atomically swap rs2 into memory, return old value.
    AmoswapW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOADD.W: Atomically add rs2 to memory, return old value.
    AmoaddW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOXOR.W: Atomically XOR rs2 into memory, return old value.
    AmoxorW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOAND.W: Atomically AND rs2 into memory, return old value.
    AmoandW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOOR.W: Atomically OR rs2 into memory, return old value.
    AmoorW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOMIN.W: Atomically min(rs2, mem) into memory, return old value (signed).
    AmominW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOMAX.W: Atomically max(rs2, mem) into memory, return old value (signed).
    AmomaxW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOMINU.W: Atomically min(rs2, mem) into memory, return old value (unsigned).
    AmominuW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },
    /// AMOMAXU.W: Atomically max(rs2, mem) into memory, return old value (unsigned).
    AmomaxuW {
        rd: u8,
        rs1: u8,
        rs2: u8,
        aq: bool,
        rl: bool,
    },

    // -- I-type ALU --
    Addi {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Slti {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Sltiu {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Xori {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Ori {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Andi {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Slli {
        rd: u8,
        rs1: u8,
        shamt: u8,
    },
    Srli {
        rd: u8,
        rs1: u8,
        shamt: u8,
    },
    Srai {
        rd: u8,
        rs1: u8,
        shamt: u8,
    },

    // -- Load --
    Lb {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Lh {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Lw {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Lbu {
        rd: u8,
        rs1: u8,
        imm: i32,
    },
    Lhu {
        rd: u8,
        rs1: u8,
        imm: i32,
    },

    // -- Store --
    Sb {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Sh {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Sw {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },

    // -- Branch --
    Beq {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Bne {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Blt {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Bge {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Bltu {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },
    Bgeu {
        rs1: u8,
        rs2: u8,
        imm: i32,
    },

    // -- Upper immediate --
    Lui {
        rd: u8,
        imm: u32,
    },
    Auipc {
        rd: u8,
        imm: u32,
    },

    // -- Jump --
    Jal {
        rd: u8,
        imm: i32,
    },
    Jalr {
        rd: u8,
        rs1: u8,
        imm: i32,
    },

    // -- System --
    Ecall,
    Ebreak,
    Fence,

    // -- NOP (e.g., floating-point instructions treated as no-ops) --
    Nop,
    Mret,
    Sret,
    SfenceVma {
        rs1: u8,
        rs2: u8,
    },

    // -- CSR --
    Csrrw {
        rd: u8,
        rs1: u8,
        csr: u32,
    },
    Csrrs {
        rd: u8,
        rs1: u8,
        csr: u32,
    },
    Csrrc {
        rd: u8,
        rs1: u8,
        csr: u32,
    },
    Csrrwi {
        rd: u8,
        uimm: u8,
        csr: u32,
    },
    Csrrsi {
        rd: u8,
        uimm: u8,
        csr: u32,
    },
    Csrrci {
        rd: u8,
        uimm: u8,
        csr: u32,
    },

    // -- Unknown --
    Invalid(u32),
}

/// Sign-extend a value from `bits` width to full i32.
pub(crate) fn sign_extend(val: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (val << shift) as i32 >> shift
}
