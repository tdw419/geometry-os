// riscv/cpu/atomic.rs -- Atomic (A extension) instruction execution
//
// Handles LR.W, SC.W, AMOSWAP.W, AMOADD.W, AMOXOR.W, AMOAND.W,
// AMOOR.W, AMOMIN.W, AMOMAX.W, AMOMINU.W, AMOMAXU.W.
//
// aq/rl flags are ignored in this single-hart emulator (no other harts
// to order against).

use super::super::bus::Bus;
use super::super::csr;
use super::super::decode::Operation;
use super::super::mmu::AccessType;
use super::{RiscvCpu, StepResult};

impl RiscvCpu {
    /// Execute an atomic instruction (A extension).
    pub(super) fn execute_atomic(
        &mut self,
        op: Operation,
        bus: &mut Bus,
        next_pc: u32,
    ) -> StepResult {
        match op {
            Operation::LrW {
                rd,
                rs1,
                aq: _,
                rl: _,
            } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(val) => {
                        self.set_reg(rd, val);
                        self.reservation = Some(pa);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::ScW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Store, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                let store_val = self.get_reg(rs2);
                let success = self.reservation == Some(pa);
                self.reservation = None;
                if success {
                    match bus.write_word(pa, store_val) {
                        Ok(()) => {
                            self.set_reg(rd, 0); // 0 = success
                            self.pc = next_pc;
                            StepResult::Ok
                        }
                        Err(_) => {
                            self.deliver_trap(csr::CAUSE_STORE_ACCESS, va);
                            StepResult::Ok
                        }
                    }
                } else {
                    self.set_reg(rd, 1); // 1 = failure
                    self.pc = next_pc;
                    StepResult::Ok
                }
            }
            Operation::AmoswapW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(old_val) => {
                        let new_val = self.get_reg(rs2);
                        self.set_reg(rd, old_val);
                        let pa_s = match self.translate_va(va, AccessType::Store, bus) {
                            Ok(p) => p,
                            Err(e) => return e,
                        };
                        match bus.write_word(pa_s, new_val) {
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
                    Err(_) => {
                        self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                        StepResult::Ok
                    }
                }
            }
            Operation::AmoaddW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.wrapping_add(new), next_pc),
            Operation::AmoxorW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old ^ new, next_pc),
            Operation::AmoandW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old & new, next_pc),
            Operation::AmoorW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old | new, next_pc),
            Operation::AmominW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(
                rd,
                rs1,
                rs2,
                bus,
                |old, new| {
                    if (old as i32) < (new as i32) {
                        old
                    } else {
                        new
                    }
                },
                next_pc,
            ),
            Operation::AmomaxW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(
                rd,
                rs1,
                rs2,
                bus,
                |old, new| {
                    if (old as i32) > (new as i32) {
                        old
                    } else {
                        new
                    }
                },
                next_pc,
            ),
            Operation::AmominuW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.min(new), next_pc),
            Operation::AmomaxuW {
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.max(new), next_pc),

            _ => unreachable!("execute_atomic called with non-atomic op"),
        }
    }

    /// Shared helper for AMO arithmetic ops.
    /// Reads old value from memory, computes new = f(old, rs2_val), writes new, returns old in rd.
    pub(super) fn exec_amo_arith<F>(
        &mut self,
        rd: u8,
        rs1: u8,
        rs2: u8,
        bus: &mut Bus,
        f: F,
        next_pc: u32,
    ) -> StepResult
    where
        F: FnOnce(u32, u32) -> u32,
    {
        let va = self.get_reg(rs1);
        let pa = match self.translate_va(va, AccessType::Load, bus) {
            Ok(p) => p,
            Err(e) => return e,
        };
        match bus.read_word(pa) {
            Ok(old_val) => {
                let new_val = f(old_val, self.get_reg(rs2));
                self.set_reg(rd, old_val);
                let pa_s = match self.translate_va(va, AccessType::Store, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.write_word(pa_s, new_val) {
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
            Err(_) => {
                self.deliver_trap(csr::CAUSE_LOAD_ACCESS, va);
                StepResult::Ok
            }
        }
    }
}
