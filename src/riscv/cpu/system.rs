// riscv/cpu/system.rs -- System instruction execution
//
// Handles ECALL, EBREAK, FENCE, NOP, MRET, SRET, SFENCE.VMA,
// CSR operations (CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI),
// and Invalid (illegal instruction trap).

use super::super::bus::Bus;
use super::super::csr;
use super::super::decode::Operation;
use super::super::mmu;
use super::{Privilege, RiscvCpu, StepResult};

impl RiscvCpu {
    /// Execute a system instruction.
    pub(super) fn execute_system(
        &mut self,
        op: Operation,
        bus: &mut Bus,
        next_pc: u32,
    ) -> StepResult {
        match op {
            Operation::Ecall => {
                self.ecall_count += 1;
                let cause = match self.privilege {
                    Privilege::User => csr::CAUSE_ECALL_U,
                    Privilege::Supervisor => csr::CAUSE_ECALL_S,
                    Privilege::Machine => csr::CAUSE_ECALL_M,
                };

                // Phase 41: Intercept User-mode ECALL as Linux syscall.
                if self.privilege == Privilege::User {
                    let nr = self.x[17];
                    let name = super::super::syscall::syscall_name(nr);
                    let event = super::super::syscall::SyscallEvent {
                        nr,
                        name,
                        args: [
                            self.x[10], self.x[11], self.x[12], self.x[13], self.x[14], self.x[15],
                        ],
                        ret: None,
                        pc: self.pc,
                    };
                    let idx = bus.syscall_log.len();
                    bus.syscall_log.push(event);
                    bus.pending_syscall_idx = Some(idx);
                }

                // SBI interception: when an ECALL from S-mode or M-mode would
                // trap, check if it's an SBI call (a7 = SBI extension ID).
                if self.privilege == Privilege::Supervisor || self.privilege == Privilege::Machine {
                    let a7 = self.x[17];
                    let a6 = self.x[16];
                    let a0 = self.x[10];
                    let a1 = self.x[11];
                    let a2 = self.x[12];
                    let a3 = self.x[13];
                    let a4 = self.x[14];
                    let a5 = self.x[15];

                    // Debug: log all SBI calls
                    if self.ecall_count <= 30 || a7 == 0x4442434E {
                        eprintln!("[sbi] ECALL #{}: a7=0x{:08X} a6={} a0=0x{:08X} a1=0x{:08X}",
                            self.ecall_count, a7, a6, a0, a1);
                    }

                    let sbi_result = bus.sbi.handle_ecall(
                        a7,
                        a6,
                        a0,
                        a1,
                        a2,
                        a3,
                        a4,
                        a5,
                        &mut bus.uart,
                        &mut bus.clint,
                    );

                    if let Some((ret_a0, ret_a1)) = sbi_result {
                        self.x[10] = ret_a0;
                        self.x[11] = ret_a1;

                        // Handle DBCN pending write: read from guest memory
                        if let Some((phys_addr, num_bytes)) = bus.sbi.dbcn_pending_write.take() {
                            for i in 0..num_bytes {
                                if let Ok(b) = bus.read_byte(phys_addr + i as u64) {
                                    if b != 0 {
                                        bus.uart.write_byte(0, b);
                                        bus.sbi.console_output.push(b);
                                    }
                                }
                            }
                        }

                        self.pc = next_pc;

                        if bus.sbi.shutdown_requested {
                            return StepResult::Ebreak;
                        }
                        return StepResult::Ok;
                    }
                }

                // Not an SBI call -- deliver as a normal trap.
                let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
                let vector = self.csr.trap_vector(trap_priv);
                self.csr
                    .trap_enter(trap_priv, self.privilege, self.pc, cause);
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
                    self.tlb.flush_all();
                } else if rs1 == 0 {
                    let asid = self.get_reg(rs2) as u16;
                    self.tlb.flush_asid(asid);
                } else if rs2 == 0 {
                    let vpn = mmu::va_to_vpn(self.get_reg(rs1));
                    self.tlb.flush_va(vpn);
                } else {
                    let vpn = mmu::va_to_vpn(self.get_reg(rs1));
                    let asid = self.get_reg(rs2) as u16;
                    self.tlb.flush_va_asid(vpn, asid);
                }
                self.pc = next_pc;
                StepResult::Ok
            }

            // ---- CSR ----
            // Note: TIME/TIMEH CSRs (0xC01/0xC81) are intercepted here because
            // they map to CLINT mtime hardware, not to the CSR bank. The csr.read()
            // function has no access to the bus/CLINT, so we handle them here.
            Operation::Csrrw { rd, rs1, csr } => {
                let old = self.read_csr_with_time(csr, bus);
                let new_val = self.get_reg(rs1);
                self.write_csr(csr, new_val, bus);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrs { rd, rs1, csr } => {
                let old = self.read_csr_with_time(csr, bus);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    self.write_csr(csr, old | mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrc { rd, rs1, csr } => {
                let old = self.read_csr_with_time(csr, bus);
                let mask = self.get_reg(rs1);
                if mask != 0 {
                    self.write_csr(csr, old & !mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrwi { rd, uimm, csr } => {
                let old = self.read_csr_with_time(csr, bus);
                self.write_csr(csr, uimm as u32, bus);
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrsi { rd, uimm, csr } => {
                let old = self.read_csr_with_time(csr, bus);
                let mask = uimm as u32;
                if mask != 0 {
                    self.write_csr(csr, old | mask, bus);
                }
                self.set_reg(rd, old);
                self.pc = next_pc;
                StepResult::Ok
            }
            Operation::Csrrci { rd, uimm, csr } => {
                let old = self.read_csr_with_time(csr, bus);
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

            _ => unreachable!("execute_system called with non-system op"),
        }
    }
}
