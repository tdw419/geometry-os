// riscv/cpu/execute.rs -- RV32I execute dispatcher (split from cpu/mod.rs)
//
// The execute() method dispatches to sub-modules by instruction category.
// Sub-modules: loadstore.rs, alu.rs, atomic.rs, system.rs.

use super::super::bus::Bus;
use super::super::decode::Operation;
use super::{RiscvCpu, StepResult};

impl RiscvCpu {
    /// Execute a decoded operation. Handles PC advancement internally.
    pub(crate) fn execute(&mut self, op: Operation, bus: &mut Bus, inst_len: u32) -> StepResult {
        let next_pc = self.pc.wrapping_add(inst_len);

        match op {
            // ---- Upper immediate ----
            Operation::Lui { rd, imm } => {
                self.set_reg(rd, imm);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Auipc { rd, imm } => {
                self.set_reg(rd, self.pc.wrapping_add(imm));
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- Jumps ----
            Operation::Jal { rd, imm } => {
                self.set_reg(rd, next_pc);
                self.pc = (self.pc as i64 + imm as i64) as u32;
                StepResult::Ok
            }
            Operation::Jalr { rd, rs1, imm } => {
                let target = (self.get_reg(rs1) as i64 + imm as i64) as u32 & !1u32;
                self.set_reg(rd, next_pc);
                self.pc = target;
                StepResult::Ok
            }

            // ---- Branches ----
            Operation::Beq { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| a == b, imm, next_pc)
            }
            Operation::Bne { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| a != b, imm, next_pc)
            }
            Operation::Blt { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| (a as i32) < (b as i32), imm, next_pc)
            }
            Operation::Bge { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| (a as i32) >= (b as i32), imm, next_pc)
            }
            Operation::Bltu { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| a < b, imm, next_pc)
            }
            Operation::Bgeu { rs1, rs2, imm } => {
                self.exec_branch(rs1, rs2, |a, b| a >= b, imm, next_pc)
            }

            // ---- Loads (delegate to loadstore.rs) ----
            Operation::Lb { .. }
            | Operation::Lh { .. }
            | Operation::Lw { .. }
            | Operation::Lbu { .. }
            | Operation::Lhu { .. } => self.execute_load(op, bus, next_pc),

            // ---- Stores (delegate to loadstore.rs) ----
            Operation::Sb { .. }
            | Operation::Sh { .. }
            | Operation::Sw { .. } => self.execute_store(op, bus, next_pc),

            // ---- ALU: R-type + I-type + M extension (delegate to alu.rs) ----
            Operation::Add { .. }
            | Operation::Sub { .. }
            | Operation::Sll { .. }
            | Operation::Slt { .. }
            | Operation::Sltu { .. }
            | Operation::Xor { .. }
            | Operation::Srl { .. }
            | Operation::Sra { .. }
            | Operation::Or { .. }
            | Operation::And { .. }
            | Operation::Mul { .. }
            | Operation::Mulh { .. }
            | Operation::Mulhu { .. }
            | Operation::Mulhsu { .. }
            | Operation::Div { .. }
            | Operation::Divu { .. }
            | Operation::Rem { .. }
            | Operation::Remu { .. }
            | Operation::Addi { .. }
            | Operation::Slti { .. }
            | Operation::Sltiu { .. }
            | Operation::Xori { .. }
            | Operation::Ori { .. }
            | Operation::Andi { .. }
            | Operation::Slli { .. }
            | Operation::Srli { .. }
            | Operation::Srai { .. } => self.execute_alu(op, next_pc),

            // ---- A extension / atomics (delegate to atomic.rs) ----
            Operation::LrW { .. }
            | Operation::ScW { .. }
            | Operation::AmoswapW { .. }
            | Operation::AmoaddW { .. }
            | Operation::AmoxorW { .. }
            | Operation::AmoandW { .. }
            | Operation::AmoorW { .. }
            | Operation::AmominW { .. }
            | Operation::AmomaxW { .. }
            | Operation::AmominuW { .. }
            | Operation::AmomaxuW { .. } => self.execute_atomic(op, bus, next_pc),

            // ---- System (delegate to system.rs) ----
            Operation::Ecall
            | Operation::Ebreak
            | Operation::Fence
            | Operation::Nop
            | Operation::Mret
            | Operation::Sret
            | Operation::SfenceVma { .. }
            | Operation::Csrrw { .. }
            | Operation::Csrrs { .. }
            | Operation::Csrrc { .. }
            | Operation::Csrrwi { .. }
            | Operation::Csrrsi { .. }
            | Operation::Csrrci { .. }
            | Operation::Invalid(_) => self.execute_system(op, bus, next_pc),
        }
    }

    /// Branch helper.
    fn exec_branch<F>(&mut self, rs1: u8, rs2: u8, cond: F, imm: i32, next_pc: u32) -> StepResult
    where
        F: Fn(u32, u32) -> bool,
    {
        let v1 = self.get_reg(rs1);
        let v2 = self.get_reg(rs2);
        if cond(v1, v2) {
            self.pc = (self.pc as i64 + imm as i64) as u32;
        } else {
            self.pc = next_pc;
        }
        StepResult::Ok
    }
}
