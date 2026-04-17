// riscv/csr/mod.rs -- CSR module (Phase 35)
//
// RISC-V privileged CSRs for M-mode and S-mode trap handling.
// sstatus is a restricted view of mstatus per the RISC-V spec.
//
// CSR addresses follow the standard RISC-V privileged specification:
//   0x300  mstatus   Machine status
//   0x305  mtvec     Machine trap vector (base address + mode)
//   0x341  mepc      Machine exception program counter
//   0x342  mcause    Machine trap cause
//   0x100  sstatus   Supervisor status (view of mstatus)
//   0x105  stvec     Supervisor trap vector
//   0x141  sepc      Supervisor exception program counter
//   0x142  scause    Supervisor trap cause
//   0x180  satp       Supervisor address translation and protection

pub mod bank;
pub mod constants;

// Re-export everything so existing `csr::CONSTANT` and `csr::CsrBank` paths work unchanged.
pub use bank::CsrBank;
pub use constants::*;
