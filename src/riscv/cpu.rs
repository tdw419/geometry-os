// riscv/cpu.rs -- RV32I CPU state + execute engine (Phase 34)
//
// Full RV32I interpreter: fetch, decode, execute.
// 40 base instructions: R-type ALU, I-type ALU, upper immediate,
// jumps, branches, load/store, FENCE, ECALL, EBREAK.
// See docs/RISCV_HYPERVISOR.md §CPU State.

use super::decode::{self, Instruction};
use super::memory::GuestMemory;

/// Privilege level.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privilege {
    User = 0,
    Supervisor = 1,
    Machine = 3,
}

impl Default for Privilege {
    fn default() -> Self {
        Privilege::Machine
    }
}

/// Result of a single step.
#[derive(Debug, PartialEq, Eq)]
pub enum StepResult {
    /// Executed one instruction normally.
    Ok,
    /// ECALL was executed (trap to higher privilege).
    Ecall,
    /// EBREAK was executed (breakpoint).
    Ebreak,
    /// Fetch failed (bad PC).
    FetchFault,
}

/// RV32I CPU state.
pub struct RiscvCpu {
    /// General-purpose registers x[0..32]. x[0] is hardwired to zero.
    pub x: [u32; 32],
    /// Program counter.
    pub pc: u32,
    /// Current privilege level.
    pub privilege: Privilege,
}

impl RiscvCpu {
    /// Create a new CPU in Machine mode with PC at the default entry point.
    pub fn new() -> Self {
        let mut cpu = Self {
            x: [0u32; 32],
            pc: 0x8000_0000,
            privilege: Privilege::Machine,
        };
        cpu.x[10] = 0; // a0 = 0 (no Hart ID)
        cpu.x[11] = 0; // a1 = 0 (no DTB)
        cpu
    }

    /// Write to register rd, enforcing x[0] = 0.
    fn set_reg(&mut self, rd: u8, val: u32) {
        if rd != 0 {
            self.x[rd as usize] = val;
        }
    }

    /// Read register rs (x[0] always returns 0).
    fn get_reg(&self, rs: u8) -> u32 {
        if rs == 0 { 0 } else { self.x[rs as usize] }
    }

    /// Fetch, decode, and execute one instruction.
    /// Returns StepResult indicating what happened.
    pub fn step(&mut self, mem: &mut GuestMemory) -> StepResult {
        let word = mem.read_word(self.pc as u64);
        let instr = decode::decode(word);
        self.execute(instr, mem)
    }

    /// Execute a decoded instruction. Handles PC advancement internally.
    fn execute(&mut self, instr: Instruction, mem: &mut GuestMemory) -> StepResult {
        let next_pc = self.pc.wrapping_add(4);

        match instr {
            // ---- Upper immediate ----
            Instruction::Lui { rd, imm } => {
                self.set_reg(rd, imm);
                self.pc = next_pc;
                StepResult::Ok
            }
            Instruction::Auipc { rd, imm } => {
                self.set_reg(rd, self.pc.wrapping_add(imm));
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- Jumps ----
            Instruction::Jal { rd, imm } => {
                self.set_reg(rd, next_pc);
                self.pc = (self.pc as i64 + imm as i64) as u32;
                StepResult::Ok
            }
            Instruction::Jalr { rd, rs1, imm } => {
                let target = (self.get_reg(rs1) as i64 + imm as i64) as u32 & !1u32;
                self.set_reg(rd, next_pc);
                self.pc = target;
                StepResult::Ok
            }

            // ---- Branches ----
            Instruction::Branch { rs1, rs2, imm, funct3 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let taken = match funct3 {
                    0b000 => v1 == v2,                    // BEQ
                    0b001 => v1 != v2,                    // BNE
                    0b100 => (v1 as i32) < (v2 as i32),   // BLT
                    0b101 => (v1 as i32) >= (v2 as i32),  // BGE
                    0b110 => v1 < v2,                     // BLTU
                    0b111 => v1 >= v2,                    // BGEU
                    _ => false,
                };
                if taken {
                    self.pc = (self.pc as i64 + imm as i64) as u32;
                } else {
                    self.pc = next_pc;
                }
                StepResult::Ok
            }

            // ---- Loads ----
            Instruction::Load { rd, rs1, imm, funct3 } => {
                let addr = (self.get_reg(rs1) as i64 + imm as i64) as u64;
                let val = match funct3 {
                    0b000 => sign_extend_byte(mem.read_byte(addr)) as u32,  // LB
                    0b001 => sign_extend_half(mem.read_half(addr)) as u32,  // LH
                    0b010 => mem.read_word(addr),                            // LW
                    0b100 => mem.read_byte(addr) as u32,                     // LBU
                    0b101 => mem.read_half(addr) as u32,                     // LHU
                    _ => 0,
                };
                self.set_reg(rd, val);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- Stores ----
            Instruction::Store { rs1, rs2, imm, funct3 } => {
                let addr = (self.get_reg(rs1) as i64 + imm as i64) as u64;
                let val = self.get_reg(rs2);
                match funct3 {
                    0b000 => mem.write_byte(addr, val as u8),   // SB
                    0b001 => mem.write_half(addr, val as u16),  // SH
                    0b010 => mem.write_word(addr, val),         // SW
                    _ => {}
                }
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- R-type ALU ----
            Instruction::RAlu { rd, rs1, rs2, funct3, funct7 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let result = match (funct3, funct7) {
                    (0b000, 0b0000000) => v1.wrapping_add(v2),  // ADD
                    (0b000, 0b0100000) => v1.wrapping_sub(v2),  // SUB
                    (0b001, _) => v1 << (v2 & 0x1F),           // SLL
                    (0b010, _) => {
                        if (v1 as i32) < (v2 as i32) { 1 } else { 0 }
                    } // SLT
                    (0b011, _) => if v1 < v2 { 1 } else { 0 }, // SLTU
                    (0b100, _) => v1 ^ v2,                     // XOR
                    (0b101, 0b0000000) => v1 >> (v2 & 0x1F),   // SRL
                    (0b101, 0b0100000) => {
                        ((v1 as i32) >> (v2 & 0x1F)) as u32    // SRA
                    }
                    (0b110, _) => v1 | v2,                     // OR
                    (0b111, _) => v1 & v2,                     // AND
                    _ => 0,
                };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- I-type ALU ----
            Instruction::IAlu { rd, rs1, imm, funct3 } => {
                let v1 = self.get_reg(rs1);
                let shamt = (imm as u32) & 0x1F;
                let result = match funct3 {
                    0b000 => v1.wrapping_add(imm as u32),       // ADDI
                    0b010 => {
                        if (v1 as i32) < imm { 1 } else { 0 }
                    } // SLTI
                    0b011 => {
                        if v1 < (imm as u32) { 1 } else { 0 }
                    } // SLTIU
                    0b100 => v1 ^ (imm as u32),                 // XORI
                    0b110 => v1 | (imm as u32),                 // ORI
                    0b111 => v1 & (imm as u32),                 // ANDI
                    0b001 => v1 << shamt,                       // SLLI
                    0b101 => {
                        let funct7 = ((imm as u32) >> 5) & 0x7F;
                        if funct7 == 0 {
                            v1 >> shamt                         // SRLI
                        } else {
                            ((v1 as i32) >> shamt) as u32      // SRAI
                        }
                    }
                    _ => 0,
                };
                self.set_reg(rd, result);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- Misc ----
            Instruction::Fence => {
                self.pc = next_pc;
                StepResult::Ok
            }
            Instruction::System { funct12, rs1: _, rd: _, funct3 } => {
                match (funct3, funct12) {
                    (0b000, 0x000) => {
                        self.pc = next_pc;
                        StepResult::Ecall
                    }
                    (0b000, 0x001) => {
                        self.pc = next_pc;
                        StepResult::Ebreak
                    }
                    _ => {
                        self.pc = next_pc;
                        StepResult::Ok // CSR ops: NOP for Phase 35
                    }
                }
            }
            Instruction::Invalid(_) => {
                self.pc = next_pc;
                StepResult::Ok
            }
        }
    }
}

/// Sign-extend a byte to i32.
fn sign_extend_byte(b: u8) -> i32 {
    b as i8 as i32
}

/// Sign-extend a half-word to i32.
fn sign_extend_half(h: u16) -> i32 {
    h as i16 as i32
}

impl Default for RiscvCpu {
    fn default() -> Self {
        Self::new()
    }
}
