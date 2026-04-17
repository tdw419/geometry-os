// riscv/cpu/loadstore.rs -- Load/Store instruction execution
//
// Handles all memory access operations: LB, LH, LW, LBU, LHU, SB, SH, SW.
// Each load translates VA→PA, reads from bus, and sign/zero-extends as needed.
// Each store translates VA→PA and writes to bus.

use super::super::bus::Bus;
use super::super::csr;
use super::super::decode::Operation;
use super::super::mmu::AccessType;
use super::{RiscvCpu, StepResult, sign_extend_byte, sign_extend_half};

impl RiscvCpu {
    /// Execute a load instruction (LB, LH, LW, LBU, LHU).
    pub(super) fn execute_load(
        &mut self,
        op: Operation,
        bus: &mut Bus,
        next_pc: u32,
    ) -> StepResult {
        match op {
            Operation::Lb { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_byte(pa) {
                    Ok(b) => {
                        self.set_reg(rd, sign_extend_byte(b) as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Lh { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_half(pa) {
                    Ok(h) => {
                        self.set_reg(rd, sign_extend_half(h) as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Lw { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(w) => {
                        self.set_reg(rd, w);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Lbu { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_byte(pa) {
                    Ok(b) => {
                        self.set_reg(rd, b as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Lhu { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_half(pa) {
                    Ok(h) => {
                        self.set_reg(rd, h as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            _ => unreachable!("execute_load called with non-load op"),
        }
    }

    /// Execute a store instruction (SB, SH, SW).
    pub(super) fn execute_store(
        &mut self,
        op: Operation,
        bus: &mut Bus,
        next_pc: u32,
    ) -> StepResult {
        match op {
            Operation::Sb { rs1, rs2, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Store, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                let val = self.get_reg(rs2);
                match bus.write_byte(pa, val as u8) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_STORE_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Sh { rs1, rs2, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Store, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                let val = self.get_reg(rs2);
                match bus.write_half(pa, val as u16) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_STORE_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::Sw { rs1, rs2, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Store, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                let val = self.get_reg(rs2);
                match bus.write_word(pa, val) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_STORE_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            _ => unreachable!("execute_store called with non-store op"),
        }
    }
}
