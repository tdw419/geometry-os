// riscv/cpu/alu.rs -- ALU instruction execution
//
// Handles R-type ALU (ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND),
// I-type ALU (ADDI, SLTI, SLTIU, XORI, ORI, ANDI, SLLI, SRLI, SRAI),
// and M extension (MUL, MULH, MULHU, MULHSU, DIV, DIVU, REM, REMU).

use super::super::decode::Operation;
use super::{RiscvCpu, StepResult};

impl RiscvCpu {
    /// Execute an ALU instruction (R-type, I-type, or M extension).
    pub(super) fn execute_alu(&mut self, op: Operation, next_pc: u32) -> StepResult {
        match op {
            // ---- R-type ALU ----
            Operation::Add { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a.wrapping_add(b));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Sub { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a.wrapping_sub(b));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Sll { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a << (b & 0x1F));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Slt { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| if (a as i32) < (b as i32) { 1 } else { 0 });
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Sltu { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| if a < b { 1 } else { 0 });
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Xor { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a ^ b);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Srl { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a >> (b & 0x1F));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Sra { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| ((a as i32) >> (b & 0x1F)) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Or { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a | b);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::And { rd, rs1, rs2 } => {
                self.alu_r(rd, rs1, rs2, |a, b| a & b);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- M extension (multiply/divide) ----
            Operation::Mul { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1) as u64;
                let v2 = self.get_reg(rs2) as u64;
                self.set_reg(rd, (v1.wrapping_mul(v2)) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Mulh { rd, rs1, rs2 } => {
                let v1 = (self.get_reg(rs1) as i32) as i64;
                let v2 = (self.get_reg(rs2) as i32) as i64;
                let product = v1.wrapping_mul(v2);
                self.set_reg(rd, (product >> 32) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Mulhu { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1) as u64;
                let v2 = self.get_reg(rs2) as u64;
                self.set_reg(rd, (v1.wrapping_mul(v2) >> 32) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Mulhsu { rd, rs1, rs2 } => {
                let v1 = (self.get_reg(rs1) as i32) as i64;
                let v2 = self.get_reg(rs2) as i64;
                let product = v1.wrapping_mul(v2);
                self.set_reg(rd, (product >> 32) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Div { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1) as i32;
                let v2 = self.get_reg(rs2) as i32;
                let result = if v2 == 0 {
                    -1i32 as u32
                } else if v1 == i32::MIN && v2 == -1 {
                    i32::MIN as u32
                } else {
                    v1.wrapping_div(v2) as u32
                };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Divu { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let result = if v2 == 0 { u32::MAX } else { v1 / v2 };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Rem { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1) as i32;
                let v2 = self.get_reg(rs2) as i32;
                let result = if v2 == 0 {
                    self.get_reg(rs1)
                } else if v1 == i32::MIN && v2 == -1 {
                    0
                } else {
                    v1.wrapping_rem(v2) as u32
                };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Remu { rd, rs1, rs2 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let result = if v2 == 0 { v1 } else { v1 % v2 };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- I-type ALU ----
            Operation::Addi { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1.wrapping_add(imm as u32));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Slti { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, if (v1 as i32) < imm { 1 } else { 0 });
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Sltiu { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, if v1 < (imm as u32) { 1 } else { 0 });
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Xori { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1 ^ (imm as u32));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Ori { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1 | (imm as u32));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Andi { rd, rs1, imm } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1 & (imm as u32));
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Slli { rd, rs1, shamt } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1 << shamt);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Srli { rd, rs1, shamt } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, v1 >> shamt);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Srai { rd, rs1, shamt } => {
                let v1 = self.get_reg(rs1);
                self.set_reg(rd, ((v1 as i32) >> shamt) as u32);
                self.pc = next_pc;
                StepResult::Ok
            }

            _ => unreachable!("execute_alu called with non-ALU op"),
        }
    }

    /// R-type ALU helper.
    pub(super) fn alu_r<F>(&mut self, rd: u8, rs1: u8, rs2: u8, op: F)
    where
        F: Fn(u32, u32) -> u32,
    {
        let v1 = self.get_reg(rs1);
        let v2 = self.get_reg(rs2);
        self.set_reg(rd, op(v1, v2));
    }

    /// Compute effective address: rs1 + imm.
    pub(super) fn ea(&self, rs1: u8, imm: i32) -> u64 {
        (self.get_reg(rs1) as i64 + imm as i64) as u64
    }
}
