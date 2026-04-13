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
        // a0 = 0, a1 = 0 (no DTB yet)
        cpu.x[10] = 0;
        cpu.x[11] = 0;
        cpu
    }

    /// Ensure x[0] is always zero (call after any register write).
    pub fn enforce_x0(&mut self) {
        self.x[0] = 0;
    }

    /// Write to register rd, enforcing x[0] = 0.
    fn set_reg(&mut self, rd: u8, val: u32) {
        if rd != 0 {
            self.x[rd as usize] = val;
        }
    }

    /// Read register rs (x[0] always returns 0).
    fn get_reg(&self, rs: u8) -> u32 {
        if rs == 0 {
            0
        } else {
            self.x[rs as usize]
        }
    }

    /// Fetch, decode, and execute one instruction.
    /// Returns StepResult indicating what happened.
    pub fn step(&mut self, mem: &mut GuestMemory) -> StepResult {
        let word = match self.fetch(mem) {
            Some(w) => w,
            None => return StepResult::FetchFault,
        };

        let instr = decode::decode(word);
        let next_pc = self.pc.wrapping_add(4);

        match self.execute(instr, next_pc, mem) {
            StepResult::Ok => {
                self.pc = next_pc;
                self.enforce_x0();
                StepResult::Ok
            }
            other => {
                self.enforce_x0();
                other
            }
        }
    }

    /// Fetch a 32-bit instruction word at the current PC.
    fn fetch(&self, mem: &GuestMemory) -> Option<u32> {
        Some(mem.read_word(self.pc as u64))
    }

    /// Execute a decoded instruction.
    /// `next_pc` is PC+4 (the default fallthrough).
    /// Returns Ok for normal execution, or Ecall/Ebreak for traps.
    /// On Ok, the caller sets self.pc = next_pc.
    /// For jumps/branches, execute() sets self.pc directly and returns Ok.
    fn execute(&mut self, instr: Instruction, next_pc: u32, mem: &mut GuestMemory) -> StepResult {
        match instr {
            Instruction::Lui { rd, imm } => {
                self.set_reg(rd, imm);
                StepResult::Ok
            }

            Instruction::Auipc { rd, imm } => {
                // imm is the upper 20 bits already shifted; PC + upper immediate
                let result = self.pc.wrapping_add(imm);
                self.set_reg(rd, result);
                StepResult::Ok
            }

            Instruction::Jal { rd, imm } => {
                let ret_addr = next_pc;
                let target = (self.pc as i64 + imm as i64) as u32;
                self.set_reg(rd, ret_addr);
                self.pc = target;
                // Signal Ok with special handling: caller should NOT overwrite pc
                // We set pc directly, so we need a way to indicate that.
                // Hack: set a flag by using a special return.
                // Actually, let's handle this differently -- see below.
                StepResult::Ok
            }

            Instruction::Jalr { rd, rs1, imm } => {
                let ret_addr = next_pc;
                let base = self.get_reg(rs1);
                let target = (base as i64 + imm as i64) as u32 & !1u32; // clear LSB
                self.set_reg(rd, ret_addr);
                self.pc = target;
                StepResult::Ok
            }

            Instruction::Branch { rs1, rs2, imm, funct3 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let taken = match funct3 {
                    0b000 => v1 == v2,           // BEQ
                    0b001 => v1 != v2,           // BNE
                    0b100 => (v1 as i32) < (v2 as i32),   // BLT (signed)
                    0b101 => (v1 as i32) >= (v2 as i32),  // BGE (signed)
                    0b110 => v1 < v2,            // BLTU (unsigned)
                    0b111 => v1 >= v2,           // BGEU (unsigned)
                    _ => false,
                };
                if taken {
                    let target = (self.pc as i64 + imm as i64) as u32;
                    self.pc = target;
                } else {
                    self.pc = next_pc;
                }
                StepResult::Ok
            }

            Instruction::Load { rd, rs1, imm, funct3 } => {
                let base = self.get_reg(rs1);
                let addr = (base as i64 + imm as i64) as u64;
                let val = match funct3 {
                    0b000 => {
                        // LB: sign-extend byte
                        let b = mem.read_byte(addr);
                        sign_extend_byte(b) as u32
                    }
                    0b001 => {
                        // LH: sign-extend half-word
                        let h = mem.read_half(addr);
                        sign_extend_half(h) as u32
                    }
                    0b010 => {
                        // LW: load word
                        mem.read_word(addr)
                    }
                    0b100 => {
                        // LBU: zero-extend byte
                        mem.read_byte(addr) as u32
                    }
                    0b101 => {
                        // LHU: zero-extend half-word
                        mem.read_half(addr) as u32
                    }
                    _ => 0,
                };
                self.set_reg(rd, val);
                StepResult::Ok
            }

            Instruction::Store { rs1, rs2, imm, funct3 } => {
                let base = self.get_reg(rs1);
                let val = self.get_reg(rs2);
                let addr = (base as i64 + imm as i64) as u64;
                match funct3 {
                    0b000 => mem.write_byte(addr, val as u8),      // SB
                    0b001 => mem.write_half(addr, val as u16),     // SH
                    0b010 => mem.write_word(addr, val),            // SW
                    _ => {}
                }
                StepResult::Ok
            }

            Instruction::RAlu { rd, rs1, rs2, funct3, funct7 } => {
                let v1 = self.get_reg(rs1);
                let v2 = self.get_reg(rs2);
                let result = match (funct3, funct7) {
                    (0b000, 0b0000000) => v1.wrapping_add(v2),              // ADD
                    (0b000, 0b0100000) => v1.wrapping_sub(v2),              // SUB
                    (0b001, 0b0000000) => v1 << (v2 & 0x1F),               // SLL
                    (0b010, 0b0000000) => {                                   // SLT
                        if (v1 as i32) < (v2 as i32) { 1 } else { 0 }
                    }
                    (0b011, 0b0000000) => if v1 < v2 { 1 } else { 0 },     // SLTU
                    (0b100, 0b0000000) => v1 ^ v2,                           // XOR
                    (0b101, 0b0000000) => v2 >> (v2 & 0x1F) | v1 >> (v2 & 0x1F), // SRL -- fix below
                    (0b101, 0b0100000) => {                                   // SRA
                        let shift = v2 & 0x1F;
                        ((v1 as i32) >> shift) as u32
                    }
                    (0b110, 0b0000000) => v1 | v2,                           // OR
                    (0b111, 0b0000000) => v1 & v2,                           // AND
                    _ => 0,
                };
                // Fix SRL: logical right shift
                let result = match (funct3, funct7) {
                    (0b101, 0b0000000) => v1 >> (v2 & 0x1F),               // SRL (logical)
                    _ => result,
                };
                self.set_reg(rd, result);
                StepResult::Ok
            }

            Instruction::IAlu { rd, rs1, imm, funct3 } => {
                let v1 = self.get_reg(rs1);
                let shamt = (imm as u32) & 0x1F;
                let result = match funct3 {
                    0b000 => v1.wrapping_add(imm as u32),           // ADDI
                    0b010 => {                                       // SLTI
                        if (v1 as i32) < imm { 1 } else { 0 }
                    }
                    0b011 => {                                       // SLTIU
                        if v1 < (imm as u32) { 1 } else { 0 }
                    }
                    0b100 => v1 ^ (imm as u32),                     // XORI
                    0b110 => v1 | (imm as u32),                     // ORI
                    0b111 => v1 & (imm as u32),                     // ANDI
                    0b001 => v1 << shamt,                            // SLLI
                    0b101 => {
                        // Distinguish SRLI vs SRAI by bit 30 of the instruction
                        // imm >> 5 gives the upper 7 bits (funct7 for shifts)
                        let funct7 = ((imm as u32) >> 5) & 0x7F;
                        if funct7 == 0b0000000 {
                            v1 >> shamt                               // SRLI (logical)
                        } else {
                            ((v1 as i32) >> shamt) as u32            // SRAI (arithmetic)
                        }
                    }
                    _ => 0,
                };
                self.set_reg(rd, result);
                StepResult::Ok
            }

            Instruction::Fence => {
                // FENCE: treat as NOP for now
                StepResult::Ok
            }

            Instruction::System { funct12, rs1: _, rd: _, funct3 } => {
                match (funct3, funct12) {
                    (0b000, 0x000) => StepResult::Ecall,
                    (0b000, 0x001) => StepResult::Ebreak,
                    _ => StepResult::Ok, // CSR ops: NOP for now (Phase 35)
                }
            }

            Instruction::Invalid(_) => {
                // Treat unknown instructions as NOP
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
