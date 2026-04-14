// riscv/cpu.rs -- RV32I CPU state + execute engine (Phase 34)
//
// Full RV32I interpreter: fetch, decode, execute.
// 40 base instructions: R-type ALU, I-type ALU, upper immediate,
// jumps, branches, load/store, FENCE, ECALL, EBREAK.
// See docs/RISCV_HYPERVISOR.md §CPU State.

use super::bus::Bus;
use super::csr::{self, CsrBank};
use super::decode::{self, Operation};
use super::mmu::{self, AccessType, Tlb, TranslateResult};

/// Privilege level.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum Privilege {
    User = 0,
    Supervisor = 1,
    #[default]
    Machine = 3,
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
    /// Translation Lookaside Buffer for Sv32 MMU.
    pub tlb: Tlb,
    /// Reservation address for LR.W/SC.W (A extension).
    /// Set by LR.W, checked by SC.W. None means no reservation.
    pub reservation: Option<u64>,
}

impl RiscvCpu {
    /// Create a new CPU in Machine mode with PC at the default entry point.
    pub fn new() -> Self {
        let mut cpu = Self {
            x: [0u32; 32],
            pc: 0x8000_0000,
            privilege: Privilege::Machine,
            csr: CsrBank::new(),
            tlb: Tlb::new(),
            reservation: None,
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

    /// Translate a virtual address through the Sv32 MMU.
    /// Returns the physical address or triggers a page fault trap.
    fn translate_va(&mut self, va: u32, access: AccessType, bus: &Bus) -> Result<u64, StepResult> {
        let is_user = self.privilege == Privilege::User;
        let satp = self.csr.satp;
        match mmu::translate(va, access, is_user, satp, bus, &mut self.tlb) {
            TranslateResult::Ok(pa) => Ok(pa),
            TranslateResult::FetchFault
            | TranslateResult::LoadFault
            | TranslateResult::StoreFault => {
                let cause = match access {
                    AccessType::Fetch => csr::CAUSE_FETCH_PAGE_FAULT,
                    AccessType::Load => csr::CAUSE_LOAD_PAGE_FAULT,
                    AccessType::Store => csr::CAUSE_STORE_PAGE_FAULT,
                };
                self.deliver_trap(cause, va);
                let fault = match access {
                    AccessType::Fetch => StepResult::FetchFault,
                    AccessType::Load => StepResult::LoadFault,
                    AccessType::Store => StepResult::StoreFault,
                };
                Err(fault)
            }
        }
    }

    /// Deliver a trap: set cause/epc/tval CSRs, update privilege, jump to vector.
    fn deliver_trap(&mut self, cause: u32, tval: u32) {
        let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
        let vector = self.csr.trap_vector(trap_priv);
        self.csr.trap_enter(trap_priv, self.privilege, self.pc, cause);
        match trap_priv {
            Privilege::Machine => self.csr.mtval = tval,
            Privilege::Supervisor => self.csr.stval = tval,
            Privilege::User => {}
        }
        self.privilege = trap_priv;
        self.pc = vector;
    }

    /// Fetch, decode, and execute one instruction.
    /// Returns StepResult indicating what happened.
    ///
    /// Before fetching, checks for pending interrupts and delivers them
    /// as traps if enabled.
    pub fn step(&mut self, bus: &mut Bus) -> StepResult {
        // Check for pending interrupts before fetching.
        if let Some(cause) = self.csr.pending_interrupt(self.privilege) {
            let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
            let vector = self.csr.trap_vector(trap_priv);
            self.csr.trap_enter(trap_priv, self.privilege, self.pc, cause);
            self.privilege = trap_priv;
            self.pc = vector;
            return StepResult::Ok;
        }

        // Translate PC through MMU for instruction fetch.
        let fetch_pa = match self.translate_va(self.pc, AccessType::Fetch, &*bus) {
            Ok(pa) => pa,
            Err(e) => return e,
        };
        let word = match bus.read_word(fetch_pa) {
            Ok(w) => w,
            Err(_) => {
                self.deliver_trap(csr::CAUSE_FETCH_ACCESS, self.pc);
                return StepResult::Ok;
            }
        };

        // RISC-V C extension: check if low 16 bits are a compressed instruction.
        // Compressed instructions have bits[1:0] != 0b11.
        // On little-endian, the low halfword is at the lower address.
        let halfword = (word & 0xFFFF) as u16;
        let (op, inst_len) = if decode::is_compressed(halfword) {
            (decode::decode_c(halfword), 2u32)
        } else {
            (decode::decode(word), 4u32)
        };
        self.execute(op, bus, inst_len)
    }

    /// Execute a decoded operation. Handles PC advancement internally.
    fn execute(&mut self, op: Operation, bus: &mut Bus, inst_len: u32) -> StepResult {
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

            // ---- Loads ----
            Operation::Lb { rd, rs1, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
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

            // ---- Stores ----
            Operation::Sb { rs1, rs2, imm } => {
                let va = self.ea(rs1, imm) as u32;
                let pa = match self.translate_va(va, AccessType::Store, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Store, &*bus) {
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
                let pa = match self.translate_va(va, AccessType::Store, &*bus) {
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
                // Upper 32 bits of signed 64-bit product
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
                // rs1 sign-extended, rs2 zero-extended into i64
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

            // ---- A extension (atomics) ----
            // AMO instructions: opcode 0x2F, funct3=010
            // Address = x[rs1], value in x[rs2], result in x[rd]
            // aq/rl flags ignored in single-hart emulator (no other harts to order against).
            Operation::LrW { rd, rs1, aq: _, rl: _ } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(val) => {
                        self.set_reg(rd, val);
                        // Set reservation on this address.
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
            Operation::ScW { rd, rs1, rs2, aq: _, rl: _ } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Store, &*bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                // Check reservation: succeeds only if reservation matches this address.
                // Per RISC-V spec, SC.W always clears the reservation.
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
            Operation::AmoswapW { rd, rs1, rs2, aq: _, rl: _ } => {
                let va = self.get_reg(rs1);
                let pa = match self.translate_va(va, AccessType::Load, &*bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(old_val) => {
                        let new_val = self.get_reg(rs2);
                        self.set_reg(rd, old_val);
                        // AMO also needs store permission.
                        let pa_s = match self.translate_va(va, AccessType::Store, &*bus) {
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
            Operation::AmoaddW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.wrapping_add(new), next_pc)
            }
            Operation::AmoxorW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old ^ new, next_pc)
            }
            Operation::AmoandW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old & new, next_pc)
            }
            Operation::AmoorW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old | new, next_pc)
            }
            Operation::AmominW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| {
                    if (old as i32) < (new as i32) { old } else { new }
                }, next_pc)
            }
            Operation::AmomaxW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| {
                    if (old as i32) > (new as i32) { old } else { new }
                }, next_pc)
            }
            Operation::AmominuW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.min(new), next_pc)
            }
            Operation::AmomaxuW { rd, rs1, rs2, aq: _, rl: _ } => {
                self.exec_amo_arith(rd, rs1, rs2, bus, |old, new| old.max(new), next_pc)
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

                // SBI interception: when an ECALL from S-mode would trap to M-mode,
                // check if it's an SBI call (a7 = SBI extension ID).
                // If handled by SBI, set results in a0/a1 and advance PC (no trap).
                // This is how real firmware (OpenSBI/BBL) handles SBI calls.
                if self.privilege == Privilege::Supervisor {
                    let a7 = self.x[17]; // extension ID
                    let a6 = self.x[16]; // function ID
                    let a0 = self.x[10];
                    let a1 = self.x[11];
                    let a2 = self.x[12];
                    let a3 = self.x[13];
                    let a4 = self.x[14];
                    let a5 = self.x[15];

                    let sbi_result =
                        bus.sbi
                            .handle_ecall(a7, a6, a0, a1, a2, a3, a4, a5, &mut bus.uart);

                    if let Some((ret_a0, ret_a1)) = sbi_result {
                        // SBI handled the call. Set results and advance PC.
                        self.x[10] = ret_a0;
                        self.x[11] = ret_a1;
                        self.pc = next_pc;

                        // Check if SBI requested shutdown
                        if bus.sbi.shutdown_requested {
                            return StepResult::Ebreak;
                        }
                        return StepResult::Ok;
                    }
                }

                // Not an SBI call -- deliver as a normal trap.
                // Check medeleg to see if this exception is delegated to S-mode.
                let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
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
            Operation::SfenceVma { rs1, rs2 } => {
                if rs1 == 0 && rs2 == 0 {
                    // Flush all entries.
                    self.tlb.flush_all();
                } else if rs1 == 0 {
                    // rs2 specifies ASID: flush non-global entries for that ASID.
                    let asid = self.get_reg(rs2) as u16;
                    self.tlb.flush_asid(asid);
                } else if rs2 == 0 {
                    // rs1 specifies VPN: flush entries for that virtual address.
                    let vpn = mmu::va_to_vpn(self.get_reg(rs1));
                    self.tlb.flush_va(vpn);
                } else {
                    // Both specified: flush entries matching both VPN and ASID.
                    let vpn = mmu::va_to_vpn(self.get_reg(rs1));
                    let asid = self.get_reg(rs2) as u16;
                    self.tlb.flush_va_asid(vpn, asid);
                }
                self.pc = next_pc;
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

            // ---- Invalid / Illegal instruction ----
            Operation::Invalid(_) => {
                self.deliver_trap(csr::CAUSE_ILLEGAL_INSTRUCTION, self.pc);
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

    /// Shared helper for AMO arithmetic ops (AMOADD, AMOXOR, AMOAND, AMOOR, AMOMIN/MAX).
    /// Reads old value from memory, computes new = f(old, rs2_val), writes new, returns old in rd.
    fn exec_amo_arith<F>(
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
        let pa = match self.translate_va(va, AccessType::Load, &*bus) {
            Ok(p) => p,
            Err(e) => return e,
        };
        match bus.read_word(pa) {
            Ok(old_val) => {
                let new_val = f(old_val, self.get_reg(rs2));
                self.set_reg(rd, old_val);
                let pa_s = match self.translate_va(va, AccessType::Store, &*bus) {
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
    use crate::riscv::bus::Bus;

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
        let mut bus = Bus::new(0x8000_0000, 4096);
        cpu.csr.mtvec = 0x8000_0200;
        let result = cpu.step(&mut bus);
        // Low addresses return 0 (boot ROM), so instruction 0x00000000 is fetched.
        // 0x00000000 decodes as compressed C.ADDI4SPN with nzuimm=0, which is
        // an illegal instruction (mcause=2). CPU traps to mtvec.
        assert_eq!(result, StepResult::Ok);
        assert_eq!(cpu.pc, 0x8000_0200);
        assert_eq!(cpu.csr.mepc, 0x0000_0000);
        assert_eq!(cpu.csr.mcause, csr::CAUSE_ILLEGAL_INSTRUCTION);
    }

    #[test]
    fn step_lui() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let word = 0x1234_52B7;
        bus.write_word(0x8000_0000, word).unwrap();
        let mut cpu = RiscvCpu::new();
        let result = cpu.step(&mut bus);
        assert_eq!(result, StepResult::Ok);
        assert_eq!(cpu.x[5], 0x1234_5000);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    // ---- R-type execution ----

    #[test]
    fn step_add() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 10;
        cpu.x[3] = 20;
        // ADD x1, x2, x3
        let word = (0u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 30);
    }

    #[test]
    fn step_sub() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 30;
        cpu.x[3] = 10;
        // SUB x1, x2, x3
        let word = (0b0100000u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 20);
    }

    #[test]
    fn step_addi() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 100;
        // ADDI x1, x2, 42
        let word = (42u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x13;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 142);
    }

    #[test]
    fn step_jal() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        // JAL x1, +8
        let word = (0u32 << 31) | (4u32 << 21) | (0u32 << 20) | (0u32 << 12) | (1u32 << 7) | 0x6F;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_0004);
        assert_eq!(cpu.pc, 0x8000_0008);
    }

    #[test]
    fn step_ecall() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.csr.mtvec = 0x8000_0200;
        bus.write_word(0x8000_0000, 0x00000073).unwrap(); // ECALL
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        // PC should jump to mtvec
        assert_eq!(cpu.pc, 0x8000_0200);
        // mepc should hold the PC of the ECALL instruction
        assert_eq!(cpu.csr.mepc, 0x8000_0000);
        // mcause should be CAUSE_ECALL_M (we're in M-mode)
        assert_eq!(cpu.csr.mcause, csr::CAUSE_ECALL_M);
    }

    #[test]
    fn step_ebreak() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        bus.write_word(0x8000_0000, 0x00100073).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ebreak);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    #[test]
    fn step_lw_sw() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0xDEAD_BEEF;
        // SW x3, 0(x2)
        let sw = (0u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b010 << 12) | (0u32 << 7) | 0x23;
        bus.write_word(0x8000_0000, sw).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        // LW x1, 0(x2)
        let lw = (0u32 << 20) | (2u32 << 15) | (0b010 << 12) | (1u32 << 7) | 0x03;
        bus.write_word(0x8000_0004, lw).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xDEAD_BEEF);
    }

    #[test]
    fn step_branch_beq_taken() {
        let mut bus = Bus::new(0x8000_0000, 4096);
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
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.pc, 0x8000_0008);
    }

    #[test]
    fn step_branch_bne_not_taken() {
        let mut bus = Bus::new(0x8000_0000, 4096);
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
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.pc, 0x8000_0004);
    }

    #[test]
    fn step_auipc() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        // AUIPC x1, 0x1000  -- imm[31:12] = 0x1
        let word = (0x1u32 << 12) | (1u32 << 7) | 0x17;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_1000);
    }

    // ---- M extension execution ----

    #[test]
    fn step_mul() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 6;
        cpu.x[3] = 7;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42);
    }

    #[test]
    fn step_mul_overflow() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFF;
        cpu.x[3] = 2;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b000 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFE);
    }

    #[test]
    fn step_mulh_positive() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 1;
        cpu.x[3] = 1;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b001 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0);
    }

    #[test]
    fn step_mulh_negative_times_negative() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFF; // -1
        cpu.x[3] = 0xFFFF_FFFF; // -1
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b001 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0); // (-1)*(-1)=1, high 32 = 0
    }

    #[test]
    fn step_mulh_large() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x0001_0000; // 65536
        cpu.x[3] = 0x0001_0000; // 65536
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b001 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 1); // 65536*65536 = 0x1_0000_0000, high 32 = 1
    }

    #[test]
    fn step_mulhu() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFF;
        cpu.x[3] = 0xFFFF_FFFF;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b011 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFE);
    }

    #[test]
    fn step_mulhsu() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFF; // -1 signed
        cpu.x[3] = 2;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b010 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // (-1)*2 = -2 as i64, high 32 = 0xFFFFFFFF
    }

    #[test]
    fn step_div() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 7;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b100 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 6);
    }

    #[test]
    fn step_div_negative() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFE; // -2
        cpu.x[3] = 2;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b100 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // -1
    }

    #[test]
    fn step_div_by_zero() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 0;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b100 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // -1
    }

    #[test]
    fn step_div_overflow() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0000u32; // INT_MIN
        cpu.x[3] = 0xFFFF_FFFF; // -1
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b100 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x8000_0000); // INT_MIN
    }

    #[test]
    fn step_divu() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 7;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b101 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 6);
    }

    #[test]
    fn step_divu_by_zero() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 0;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b101 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], u32::MAX);
    }

    #[test]
    fn step_rem() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 7;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b110 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0);
    }

    #[test]
    fn step_rem_negative() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0xFFFF_FFFE; // -2
        cpu.x[3] = 3;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b110 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFE); // -2 (Rust truncation toward zero)
    }

    #[test]
    fn step_rem_by_zero() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 0;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b110 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42);
    }

    #[test]
    fn step_rem_overflow() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0000u32; // INT_MIN
        cpu.x[3] = 0xFFFF_FFFF; // -1
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b110 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0);
    }

    #[test]
    fn step_remu() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 7;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b111 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0);
    }

    #[test]
    fn step_remu_by_zero() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 42;
        cpu.x[3] = 0;
        let word = (0x01u32 << 25) | (3u32 << 20) | (2u32 << 15) | (0b111 << 12) | (1u32 << 7) | 0x33;
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42);
    }

    // ---- A extension (atomics) tests ----
    // AMO encoding: (funct5 << 27) | (aq << 26) | (rl << 25) | (rs2 << 20) | (rs1 << 15) | (0b010 << 12) | (rd << 7) | 0x2F

    /// Helper to encode an AMO instruction word.
    fn amo_encode(funct5: u32, rd: u32, rs1: u32, rs2: u32, aq: bool, rl: bool) -> u32 {
        (funct5 << 27)
            | ((aq as u32) << 26)
            | ((rl as u32) << 25)
            | (rs2 << 20)
            | (rs1 << 15)
            | (0b010 << 12)
            | (rd << 7)
            | 0x2F
    }

    #[test]
    fn step_lr_w() {
        // LR.W x1, (x2) -- funct5=00010, aq=0, rl=0
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100; // address in bus range
        bus.write_word(0x8000_0100, 0xDEADBEEF).unwrap();
        let word = amo_encode(0b00010, 1, 2, 0, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xDEADBEEF);
        // Reservation should be set
        assert!(cpu.reservation.is_some());
    }

    #[test]
    fn step_sc_w_success() {
        // First LR.W x1, (x2) then SC.W x3, x1, (x2)
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        bus.write_word(0x8000_0100, 0x11111111).unwrap();

        // LR.W x1, (x2)
        let lr = amo_encode(0b00010, 1, 2, 0, false, false);
        bus.write_word(0x8000_0000, lr).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x11111111);

        // SC.W x3, x4, (x2) -- store x4=0xCAFEBABE to address in x2
        cpu.x[4] = 0xCAFEBABE;
        let sc = amo_encode(0b00011, 3, 2, 4, false, false);
        bus.write_word(0x8000_0004, sc).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[3], 0); // 0 = success
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xCAFEBABE
        );
    }

    #[test]
    fn step_sc_w_fail() {
        // SC.W without prior LR.W should fail (no reservation)
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[4] = 0xCAFEBABE;
        bus.write_word(0x8000_0100, 0x11111111).unwrap();

        let sc = amo_encode(0b00011, 3, 2, 4, false, false);
        bus.write_word(0x8000_0000, sc).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[3], 1); // 1 = failure
        // Memory should be unchanged
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0x11111111
        );
    }

    #[test]
    fn step_amoswap_w() {
        // AMOSWAP.W x1, x3, (x2) -- funct5=00001
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0x12345678;
        bus.write_word(0x8000_0100, 0xABCDEF00).unwrap();
        let word = amo_encode(0b00001, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xABCDEF00); // old value
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0x12345678
        ); // swapped
    }

    #[test]
    fn step_amoadd_w() {
        // AMOADD.W x1, x3, (x2) -- funct5=00000
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 100;
        bus.write_word(0x8000_0100, 42).unwrap();
        let word = amo_encode(0b00000, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42); // old value
        assert_eq!(bus.read_word(0x8000_0100).unwrap(), 142); // 42+100
    }

    #[test]
    fn step_amoxor_w() {
        // AMOXOR.W x1, x3, (x2) -- funct5=00101
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0xFF00FF00;
        bus.write_word(0x8000_0100, 0x0F0F0F0F).unwrap();
        let word = amo_encode(0b00101, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0x0F0F0F0F);
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xF00FF00F
        );
    }

    #[test]
    fn step_amoand_w() {
        // AMOAND.W x1, x3, (x2) -- funct5=01101
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0x0F0F0F0F;
        bus.write_word(0x8000_0100, 0xFF00FF00).unwrap();
        let word = amo_encode(0b01101, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFF00FF00);
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0x0F000F00
        );
    }

    #[test]
    fn step_amoor_w() {
        // AMOOR.W x1, x3, (x2) -- funct5=01001
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0x0F0F0F0F;
        bus.write_word(0x8000_0100, 0xF0F0F0F0).unwrap();
        let word = amo_encode(0b01001, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xF0F0F0F0);
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xFFFFFFFF
        );
    }

    #[test]
    fn step_amomin_w() {
        // AMOMIN.W x1, x3, (x2) -- funct5=10000 (signed min)
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 10u32; // small positive
        bus.write_word(0x8000_0100, 0xFFFF_FFFF).unwrap(); // -1 signed
        let word = amo_encode(0b10000, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // old value
        // if (old as i32) < (new as i32) { old } else { new }
        // old = -1, new = 10. -1 < 10, so result = old = -1 = 0xFFFF_FFFF
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xFFFF_FFFF
        ); // stays as -1
    }

    #[test]
    fn step_amomin_w_signed() {
        // AMOMIN.W: signed comparison. mem=-1, rs2=10 -> -1 is min, stays
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 10u32;
        bus.write_word(0x8000_0100, 0xFFFF_FFFF).unwrap(); // -1 signed
        let word = amo_encode(0b10000, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // old
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xFFFF_FFFF
        ); // -1 < 10, stays
    }

    #[test]
    fn step_amomax_w_signed() {
        // AMOMAX.W: signed comparison. mem=-1, rs2=10 -> 10 is max
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 10u32;
        bus.write_word(0x8000_0100, 0xFFFF_FFFF).unwrap(); // -1 signed
        let word = amo_encode(0b10100, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF); // old
        assert_eq!(bus.read_word(0x8000_0100).unwrap(), 10); // max(-1, 10) = 10
    }

    #[test]
    fn step_amominu_w() {
        // AMOMINU.W x1, x3, (x2) -- funct5=11000 (unsigned min)
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0xFFFF_FFFF; // large unsigned
        bus.write_word(0x8000_0100, 42).unwrap();
        let word = amo_encode(0b11000, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42); // old
        assert_eq!(bus.read_word(0x8000_0100).unwrap(), 42); // min(42, 0xFFFF_FFFF) = 42
    }

    #[test]
    fn step_amomaxu_w() {
        // AMOMAXU.W x1, x3, (x2) -- funct5=11100 (unsigned max)
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 0xFFFF_FFFF; // large unsigned
        bus.write_word(0x8000_0100, 42).unwrap();
        let word = amo_encode(0b11100, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 42); // old
        assert_eq!(
            bus.read_word(0x8000_0100).unwrap(),
            0xFFFF_FFFF
        ); // max(42, 0xFFFF_FFFF)
    }

    #[test]
    fn step_amoadd_w_overflow() {
        // Test wrapping add behavior
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        cpu.x[3] = 1;
        bus.write_word(0x8000_0100, 0xFFFF_FFFF).unwrap();
        let word = amo_encode(0b00000, 1, 2, 3, false, false);
        bus.write_word(0x8000_0000, word).unwrap();
        assert_eq!(cpu.step(&mut bus), StepResult::Ok);
        assert_eq!(cpu.x[1], 0xFFFF_FFFF);
        assert_eq!(bus.read_word(0x8000_0100).unwrap(), 0); // wraps to 0
    }

    #[test]
    fn step_sc_w_clears_reservation() {
        // SC.W should clear the reservation regardless of success
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut cpu = RiscvCpu::new();
        cpu.x[2] = 0x8000_0100;
        bus.write_word(0x8000_0100, 0).unwrap();

        // LR.W x1, (x2)
        let lr = amo_encode(0b00010, 1, 2, 0, false, false);
        bus.write_word(0x8000_0000, lr).unwrap();
        cpu.step(&mut bus);
        assert!(cpu.reservation.is_some());

        // SC.W x3, x4, (x2) -- different address
        cpu.x[3] = 0x8000_0200;
        cpu.x[4] = 42;
        let sc = amo_encode(0b00011, 5, 3, 4, false, false);
        bus.write_word(0x8000_0200, 0).unwrap();
        bus.write_word(0x8000_0004, sc).unwrap();
        cpu.step(&mut bus);
        assert_eq!(cpu.x[5], 1); // fail (no reservation for 0x8000_0200)

        // Second SC.W on original address should also fail (reservation cleared)
        cpu.x[5] = 0; // reset rd
        let sc2 = amo_encode(0b00011, 5, 2, 4, false, false);
        bus.write_word(0x8000_0008, sc2).unwrap();
        cpu.step(&mut bus);
        assert_eq!(cpu.x[5], 1); // fail -- reservation was cleared
    }

}
