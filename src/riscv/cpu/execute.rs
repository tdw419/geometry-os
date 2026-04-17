// riscv/cpu/execute.rs -- RV32I execute engine (split from cpu/mod.rs)
//
// The execute() method and its helper functions.
// Separated from the main cpu module to keep the match arm manageable.

use super::super::bus::Bus;
use super::super::csr;
use super::super::decode::Operation;
use super::super::mmu::AccessType;
use super::super::mmu;
use super::{Privilege, RiscvCpu, StepResult, sign_extend_byte, sign_extend_half};

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

            // ---- Loads ----
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

            // ---- Stores ----
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
                let pa = match self.translate_va(va, AccessType::Load, bus) {
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
                let pa = match self.translate_va(va, AccessType::Store, bus) {
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
                let pa = match self.translate_va(va, AccessType::Load, bus) {
                    Ok(p) => p,
                    Err(e) => return e,
                };
                match bus.read_word(pa) {
                    Ok(old_val) => {
                        let new_val = self.get_reg(rs2);
                        self.set_reg(rd, old_val);
                        // AMO also needs store permission.
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
                self.ecall_count += 1;
                // Determine trap cause based on current privilege.
                let cause = match self.privilege {
                    Privilege::User => csr::CAUSE_ECALL_U,
                    Privilege::Supervisor => csr::CAUSE_ECALL_S,
                    Privilege::Machine => csr::CAUSE_ECALL_M,
                };

                // Phase 41: Intercept User-mode ECALL as Linux syscall.
                // a7 (x[17]) = syscall number, a0-a5 (x[10]-x[15]) = args.
                if self.privilege == Privilege::User {
                    let nr = self.x[17];
                    let name = super::super::syscall::syscall_name(nr);
                    let event = super::super::syscall::SyscallEvent {
                        nr,
                        name,
                        args: [self.x[10], self.x[11], self.x[12],
                               self.x[13], self.x[14], self.x[15]],
                        ret: None,
                        pc: self.pc,
                    };
                    let idx = bus.syscall_log.len();
                    bus.syscall_log.push(event);
                    bus.pending_syscall_idx = Some(idx);
                }

                // SBI interception: when an ECALL from S-mode or M-mode would
                // trap, check if it's an SBI call (a7 = SBI extension ID).
                // If handled by SBI, set results in a0/a1 and advance PC (no trap).
                // This is how real firmware (OpenSBI/BBL) handles SBI calls.
                //
                // M-mode ECALL is the standard way the kernel calls SBI during
                // early boot (before transitioning to S-mode). Without this,
                // M-mode ECALLs go through the trap forwarding path which just
                // skips the instruction without setting results.
                if self.privilege == Privilege::Supervisor
                    || self.privilege == Privilege::Machine
                {
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
                            .handle_ecall(a7, a6, a0, a1, a2, a3, a4, a5, &mut bus.uart, &mut bus.clint);

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
            Operation::Nop => {
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

                // Phase 41: capture syscall return value.
                // If we had a pending U-mode syscall and SRET returns to U-mode,
                // a0 (x[10]) holds the syscall return value.
                if restored == Privilege::User {
                    if let Some(idx) = bus.pending_syscall_idx.take() {
                        if let Some(event) = bus.syscall_log.get_mut(idx) {
                            event.ret = Some(self.x[10]);
                        }
                    }
                }

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
                self.write_csr(csr, new_val, bus);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrs { rd, rs1, csr } => {
                let old = self.csr.read(csr);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    self.write_csr(csr, old | mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrc { rd, rs1, csr } => {
                let old = self.csr.read(csr);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    self.write_csr(csr, old & !mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrwi { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                self.write_csr(csr, uimm as u32, bus);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrsi { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                let mask = uimm as u32;
                if mask != 0 {
                    self.write_csr(csr, old | mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrci { rd, uimm, csr } => {
                let old = self.csr.read(csr);
                let mask = uimm as u32;
                if mask != 0 {
                    self.write_csr(csr, old & !mask, bus);
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
