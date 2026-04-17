// riscv/csr/constants.rs -- CSR address constants, bit masks, and exception/interrupt codes
//
// RISC-V privileged CSR addresses and bit definitions per the RISC-V spec.
// Split from csr.rs for maintainability.

// ---- CSR address constants ----

pub const MSTATUS: u32 = 0x300;
pub const MTVEC: u32 = 0x305;
pub const MEPC: u32 = 0x341;
pub const MCAUSE: u32 = 0x342;
pub const MTVAL: u32 = 0x343;

pub const SSTATUS: u32 = 0x100;
pub const STVEC: u32 = 0x105;
pub const SEPC: u32 = 0x141;
pub const SSCRATCH: u32 = 0x140;
pub const SCAUSE: u32 = 0x142;
pub const STVAL: u32 = 0x143;
pub const SATP: u32 = 0x180;

// Trap delegation CSRs
pub const MEDELEG: u32 = 0x302;
pub const MIDELEG: u32 = 0x303;

// MIE/MIP interrupt enable/pending registers
pub const MIE: u32 = 0x304;
pub const MIP: u32 = 0x344;
pub const SIE: u32 = 0x104;
pub const SIP: u32 = 0x144;

// ---- mstatus bit masks ----

/// SSTATUS is a restricted view of MSTATUS.
/// Visible bits: SIE (1), SPIE (5), SPP (8), SUM (18), MXR (19).
/// In RV32 the SD bit (31) is read-only and derived.
pub(crate) const SSTATUS_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 8) | (1 << 18) | (1 << 19);

/// SIP is a restricted view of MIP.
/// Visible bits: SSIP (1), STIP (5), SEIP (9).
pub(crate) const SIP_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 9);

/// SIE is a restricted view of MIE.
/// Visible bits: SSIE (1), STIE (5), SEIE (9).
pub(crate) const SIE_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 9);

/// Bit positions in mstatus.
pub const MSTATUS_SIE: u32 = 1; // Supervisor Interrupt Enable
pub const MSTATUS_MIE: u32 = 3; // Machine Interrupt Enable
pub const MSTATUS_SPIE: u32 = 5; // Supervisor Previous IE
pub const MSTATUS_MPIE: u32 = 7; // Machine Previous IE
pub const MSTATUS_SPP: u32 = 8; // Supervisor Previous Privilege (1 bit)
pub const MSTATUS_MPRV: u32 = 17; // Memory Privilege (M-mode uses MPP for loads/stores)
pub const MSTATUS_SUM: u32 = 18; // Supervisor User Memory access
pub const MSTATUS_MXR: u32 = 19; // Make eXecutable Readable
pub const MSTATUS_MPP_LSB: u32 = 11; // Machine Previous Privilege (2 bits: 12:11)
pub const MSTATUS_MPP_MASK: u32 = 0x3 << 11;

/// Trap cause: top bit = interrupt (1) or exception (0), lower 31 bits = code.
pub const MCAUSE_INTERRUPT_BIT: u32 = 1 << 31;

/// Exception codes (lower bits of mcause/scause).
pub const CAUSE_MISALIGNED_FETCH: u32 = 0;
pub const CAUSE_FETCH_ACCESS: u32 = 1;
pub const CAUSE_ILLEGAL_INSTRUCTION: u32 = 2;
pub const CAUSE_BREAKPOINT: u32 = 3;
pub const CAUSE_MISALIGNED_LOAD: u32 = 4;
pub const CAUSE_LOAD_ACCESS: u32 = 5;
pub const CAUSE_MISALIGNED_STORE: u32 = 6;
pub const CAUSE_STORE_ACCESS: u32 = 7;
pub const CAUSE_ECALL_U: u32 = 8;
pub const CAUSE_ECALL_S: u32 = 9;
pub const CAUSE_ECALL_M: u32 = 11;
pub const CAUSE_FETCH_PAGE_FAULT: u32 = 12;
pub const CAUSE_LOAD_PAGE_FAULT: u32 = 13;
pub const CAUSE_STORE_PAGE_FAULT: u32 = 15;

/// Interrupt codes.
pub const INT_SSI: u32 = 1; // Supervisor software interrupt
pub const INT_MSI: u32 = 3; // Machine software interrupt
pub const INT_STI: u32 = 5; // Supervisor timer interrupt
pub const INT_MTI: u32 = 7; // Machine timer interrupt
pub const INT_SEI: u32 = 9; // Supervisor external interrupt
pub const INT_MEI: u32 = 11; // Machine external interrupt
