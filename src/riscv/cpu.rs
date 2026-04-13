// riscv/cpu.rs -- RV32I CPU state + execute engine (Phase 34)
//
// Full RV32I interpreter: fetch, decode, execute.
// 40 base instructions: R-type ALU, I-type ALU, upper immediate,
// jumps, branches, load/store, FENCE, ECALL, EBREAK.
// See docs/RISCV_HYPERVISOR.md §CPU State.

use super::csr::{self, CsrBank};
use super::decode::{self, Operation};
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
    /// Fetch failed (bad PC or unmapped memory).
    FetchFault,
    /// Load from unmapped memory.
    LoadFault,
    /// Store to unmapped memory.
    StoreFault,
}

/// RV32I CPU state.
pub struct RiscvCpu {
    /// General-purpose registers x[0..32]. x[0] is hardwired to zero.
    pub x: [u32; 32],
    /// Program counter.
    pub pc: u32,
    /// Current privilege level.
    pub privilege: Privilege,
    /// Control and Status Registers.
    pub csr: CsrBank,
}

impl RiscvCpu {
    /// Create a new CPU in Machine mode with PC at the default entry point.
    pub fn new() -> Self {
        let mut cpu = Self {
            x: [0u32; 32],
            pc: 0x8000_0000,
            privilege: Privilege::Machine,
            csr: CsrBank::new(),
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
        if rs == 0 {
            0
        } else {
            self.x[rs as usize]
        }
    }

    /// Fetch, decode, and execute one instruction.
    /// Returns StepResult indicating what happened.
    pub fn step(&mut self, mem: &mut GuestMemory) -> StepResult {
        let word = match mem.read_word(self.pc as u64) {
            Ok(w) => w,
            Err(_) => return StepResult::FetchFault,
        };
        let op = decode::decode(word);
        self.execute(op, mem)
    }

    /// Execute a decoded operation. Handles PC advancement internally.
    fn execute(&mut self, op: Operation, mem: &mut GuestMemory) -> StepResult {
        let next_pc = self.pc.wrapping_add(4);

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

            // ---- Loads ----
            Operation::Lb { rd, rs1, imm } => {
                let addr = self.ea(rs1, imm);
                match mem.read_byte(addr) {
                    Ok(b) => {
                        self.set_reg(rd, sign_extend_byte(b) as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::LoadFault,
                }
            }
            Operation::Lh { rd, rs1, imm } => {
                let addr = self.ea(rs1, imm);
                match mem.read_half(addr) {
                    Ok(h) => {
                        self.set_reg(rd, sign_extend_half(h) as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::LoadFault,
                }
            }
            Operation::Lw { rd, rs1, imm } => {
                let addr = self.ea(rs1, imm);
                match mem.read_word(addr) {
                    Ok(w) => {
                        self.set_reg(rd, w);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::LoadFault,
                }
            }
            Operation::Lbu { rd, rs1, imm } => {
                let addr = self.ea(rs1, imm);
                match mem.read_byte(addr) {
                    Ok(b) => {
                        self.set_reg(rd, b as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::LoadFault,
                }
            }
            Operation::Lhu { rd, rs1, imm } => {
                let addr = self.ea(rs1, imm);
                match mem.read_half(addr) {
                    Ok(h) => {
                        self.set_reg(rd, h as u32);
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::LoadFault,
                }
            }

            // ---- Stores ----
            Operation::Sb { rs1, rs2, imm } => {
                let addr = self.ea(rs1, imm);
                let val = self.get_reg(rs2);
                match mem.write_byte(addr, val as u8) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::StoreFault,
                }
            }
            Operation::Sh { rs1, rs2, imm } => {
                let addr = self.ea(rs1, imm);
                let val = self.get_reg(rs2);
                match mem.write_half(addr, val as u16) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::StoreFault,
                }
            }
            Operation::Sw { rs1, rs2, imm } => {
                let addr = self.ea(rs1, imm);
                let val = self.get_reg(rs2);
                match mem.write_word(addr, val) {
                    Ok(()) => {
                        self.pc = next_pc;
                        StepResult::Ok
                    }
                    Err(_) => StepResult::StoreFault,
                }
            }

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

            // ---- System ----
            Operation::Ecall => {
                // Determine trap cause based on current privilege.
                let cause = match self.privilege {
                    Privilege::User => csr::CAUSE_ECALL_U,
                    Privilege::Supervisor => csr::CAUSE_ECALL_S,
                    Privilege::Machine => csr::CAUSE_ECALL_M,
                };
                // ECALL always traps to Machine mode (no delegation yet).
                let trap_priv = Privilege::Machine;
                let vector = self.csr.trap_vector(trap_priv);
                self.csr.trap_enter(trap_priv, self.privilege, self.pc, cause);
                self.privilege = trap_priv;
                self.pc = vector;
                StepResult::Ok
            }
            Operation::Ebreak => {
                self.pc = next_pc;
                StepResult::Ebreak
            }
            Operation::Fence => {
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Mret => {
                let restored = self.csr.trap_return(Privilege::Machine);
                self.pc = self.csr.mepc;
                self.privilege = restored;
                StepResult::Ok
            }
            Operation::Sret => {
                let restored = self.csr.trap_return(Privilege::Supervisor);
                self.pc = self.csr.sepc;
                self.privilege = restored;
                StepResult::Ok
            }

            // ---- CSR ----
            Operation::Csrrw { rd, rs1, csr } => {
                let old = self.csr.read(csr);
                let new_val = self.get_reg(rs1);
                // Write even if rd=x0 (but don't read old into x0).
                self.csr.write(csr, new_val);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrs { rd, rs1, csr } => {
                let old = self.csr.read(csr);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    let _ = self.csr.write(csr, old | mask);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrc { rd, rs1, csr } => {
                let old = self.csr.read(csr);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    let _ = self.csr.write(csr, old & !mask);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrwi { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                self.csr.write(csr, uimm as u32);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrsi { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                let mask = uimm as u32;
                if mask != 0 {
                    let _ = self.csr.write(csr, old | mask);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrci { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                let mask = uimm as u32;
                if mask != 0 {
                    let _ = self.csr.write(csr, old & !mask);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- Invalid ----
            Operation::Invalid(_) => {
                self.pc = next_pc;
                StepResult::Ok
            }
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

    /// R-type ALU helper.
    fn alu_r<F>(&mut self, rd: u8, rs1: u8, rs2: u8, op: F)
    where
        F: Fn(u32, u32) -> u32,
    {
        let v1 = self.get_reg(rs1);
        let v2 = self.get_reg(rs2);
        self.set_reg(rd, op(v1, v2));
    }

    /// Compute effective address: rs1 + imm.
    fn ea(&self, rs1: u8, imm: i32) -> u64 {
        (self.get_reg(rs1) as i64 + imm as i64) as u64
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cpu_defaults() {
        let cpu = RiscvCpu::new();
        assert_eq!(cpu.pc, 0x8000_0000);
        assert_eq!(cpu.privilege, Privilege::Machine);
        assert_eq!(cpu.x[0], 0);
        assert_eq!(cpu.x[10], 0);
        assert_eq!(cpu.x[11], 0);
    }

    #[test]
    fn write_reg_x0_is_noop() {
        let mut cpu = RiscvCpu::new();
        cpu.set_reg(0, 0xDEAD_BEEF);
        assert_eq!(cpu.x[0], 0);
    }

    #[test]
    fn read_reg_x0_is_zero() {
        let cpu = RiscvCpu::new();
        assert_eq!(cpu.get_reg(0), 0);
    }

    #[test]
    fn write_read_reg_roundtrip() {
        let mut cpu = RiscvCpu::new();
        cpu.set_reg(5, 0x1234_5678);
        assert_eq!(cpu.get_reg(5), 0x1234_5678);
    }

    #[test]
    fn fetch_fault_on_bad_pc() {
        let mut cpu = RiscvCpu::new();
        cpu.pc = 0x0000_0000;
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        assert_eq!(cpu.step(&mut mem), StepResult::FetchFault);
    }

    #[test]
    fn step_lui() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let word = 0x1234_52B7;
        mem.write_word(0x8000_0000, word).unwrap();
        let mut cpu = RiscvCpu::new();
        let result = cpu.step(&mut mem);
        assert_eq!(result, StepResult::Ok);
        assert_eq!(cpu.x[5], 0x1234_5000);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    // ---- R-type execution ----

    #[test]
    fn step_add() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 10;
        cpu.x[3] = 20;
        // ADD x1, x2, x3
        let word = (0u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 30);
    }

    #[test]
    fn step_sub() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 30;
        cpu.x[3] = 10;
        // SUB x1, x2, x3
        let word = (0b0100000u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 20);
    }

    #[test]
    fn step_addi() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 100;
        // ADDI x1, x2, 42
        let word = (42u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x13;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 142);
    }

    #[test]
    fn step_jal() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        // JAL x1, +8
        let word = (0u32 << 31) | (4u32 << 21) | (0u32 << 20) | (0u32 << 12) | (1u32 << 7) | 0x6F;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_0004);
        assert_eq!(cpu.pc, 0x8000_0008);
    }

    #[test]
    fn step_ecall() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.csr.mtvec = 0x8000_0200;
        mem.write_word(0x8000_0000, 0x00000073).unwrap(); // ECALL
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        // PC should jump to mtvec
        assert_eq!(cpu.pc, 0x8000_0200);
        // mepc should hold the PC of the ECALL instruction
        assert_eq!(cpu.csr.mepc, 0x8000_0000);
        // mcause should be CAUSE_ECALL_M (we're in M-mode)
        assert_eq!(cpu.csr.mcause, csr::CAUSE_ECALL_M);
    }

    #[test]
    fn step_ebreak() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        mem.write_word(0x8000_0000, 0x00100073).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ebreak);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    #[test]
    fn step_lw_sw() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0xDEAD_BEEF;
        // SW x3, 0(x2)
        let sw = (0u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b010 << 12) | (0u32 << 7) | 0x23;
        mem.write_word(0x8000_0000, sw).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        // LW x1, 0(x2)
        let lw = (0u32 << 20) | (2u32 << 15) | (0b010 << 12) | (1u32 << 7) | 0x03;
        mem.write_word(0x8000_0004, lw).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xDEAD_BEEF);
    }

    #[test]
    fn step_branch_beq_taken() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 42;
        // BEQ x2, x3, +8
        let imm: u32 = 8;
        let bit12 = (imm >> 12) & 1;
        let bit11 = (imm >> 11) & 1;
        let bits10_5 = (imm >> 5) & 0x3F;
        let bits4_1 = (imm >> 1) & 0xF;
        let word = (bit12 << 31) | (bits10_5 << 25) | (3u32 << 20) | (2u32 << 15)
            | (0b000 << 12) | (bits4_1 << 8) | (bit11 << 7) | 0x63;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.pc, 0x8000_0008);
    }

    #[test]
    fn step_branch_bne_not_taken() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 42;
        // BNE x2, x3, +8 (not taken)
        let imm: u32 = 8;
        let bit12 = (imm >> 12) & 1;
        let bit11 = (imm >> 11) & 1;
        let bits10_5 = (imm >> 5) & 0x3F;
        let bits4_1 = (imm >> 1) & 0xF;
        let word = (bit12 << 31) | (bits10_5 << 25) | (3u32 << 20) | (2u32 << 15)
            | (0b001 << 12) | (bits4_1 << 8) | (bit11 << 7) | 0x63;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    #[test]
    fn step_auipc() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        // AUIPC x1, 0x1000  -- imm[31:12] = 0x1
        let word = (0x1u32 << 12) | (1u32 << 7) | 0x17;
        mem.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut mem), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_1000);
    }
}
