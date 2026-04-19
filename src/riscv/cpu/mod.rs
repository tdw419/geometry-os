// riscv/cpu/mod.rs -- RV32I CPU state + execute engine (Phase 34)
//
// Full RV32I interpreter: fetch, decode, execute.
// 40 base instructions: R-type ALU, I-type ALU, upper immediate,
// jumps, branches, load/store, FENCE, ECALL, EBREAK.
// See docs/RISCV_HYPERVISOR.md §CPU State.

mod alu;
mod atomic;
mod execute;
mod loadstore;
mod system;
#[cfg(test)]
mod tests;

use super::bus::Bus;
use super::csr::{self, CsrBank};
use super::decode::{self, Operation};
use super::mmu::{self, AccessType, Tlb, TranslateResult};

/// Privilege level.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Privilege {
    User = 0,
    Supervisor = 1,
    #[default]
    Machine = 3,
}

/// Result of a single step.
#[derive(Debug, PartialEq, Eq, Clone)]
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

/// Information about the last executed instruction (Phase 41: tracing).
/// Always populated by step() with near-zero overhead. Used by the trace
/// system when tracing is enabled.
#[derive(Debug, Clone)]
pub struct LastStepInfo {
    /// Program counter before execution.
    pub pc: u32,
    /// Raw instruction word fetched.
    pub word: u32,
    /// Decoded operation.
    pub op: Operation,
    /// Instruction length (2 for compressed, 4 for normal).
    pub inst_len: u32,
    /// Registers before execution.
    pub regs_before: [u32; 32],
    /// Registers after execution.
    pub regs_after: [u32; 32],
    /// PC after execution.
    pub pc_after: u32,
    /// Step result.
    pub result: StepResult,
}

/// Scheduler trace event (Phase 41).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedEvent {
    /// Context switch detected (tp register changed).
    ContextSwitch { old_tp: u32, new_tp: u32, pc: u32 },
}

/// RV32I CPU state.
pub struct RiscvCpu {
    /// General-purpose registers x0-x31. x0 is hardwired to zero.
    pub x: [u32; 32],
    /// Program counter.
    pub pc: u32,
    /// Current privilege level.
    pub privilege: Privilege,
    /// Control and Status Registers.
    pub csr: CsrBank,
    /// Translation Lookaside Buffer for Sv32 MMU.
    pub tlb: Tlb,
    /// Delayed TLB flush: when SATP is written, the flush is deferred
    /// by one instruction to emulate the pipeline effect. On real hardware,
    /// the instruction after CSRW SATP is already fetched before the MMU
    /// change takes effect. Without this, the kernel's MMU-enable trampoline
    /// (which relies on fetching the next instruction from a physical address
    /// before the virtual mapping is active) immediately page-faults.
    pub satp_flush_pending: bool,
    /// Reservation address for LR.W/SC.W (A extension).
    /// Set by LR.W, checked by SC.W. None means no reservation.
    pub reservation: Option<u64>,
    /// Last step info, populated every step for tracing (Phase 41).
    pub last_step: Option<LastStepInfo>,
    /// Count of ECALL instructions executed (diagnostic).
    pub ecall_count: u64,
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
            satp_flush_pending: false,
            reservation: None,
            last_step: None,
            ecall_count: 0,
        };
        cpu.x[10] = 0; // a0 = 0 (no Hart ID)
        cpu.x[11] = 0; // a1 = 0 (no DTB)
        cpu
    }

    /// Write to register rd, enforcing x[0] = 0.
    pub(crate) fn set_reg(&mut self, rd: u8, val: u32) {
        if rd != 0 {
            self.x[rd as usize] = val;
        }
    }

    /// Read register rs (x[0] always returns 0).
    pub(crate) fn get_reg(&self, rs: u8) -> u32 {
        if rs == 0 {
            0
        } else {
            self.x[rs as usize]
        }
    }

    /// Translate a virtual address through the Sv32 MMU.
    /// Returns the physical address or triggers a page fault trap.
    ///
    /// Per RISC-V spec: M-mode instruction fetches always use bare mode
    /// (no translation). For loads/stores in M-mode, translation depends on
    /// MPRV: when MPRV=0 (default), bare mode; when MPRV=1, uses MPP privilege.
    fn translate_va(
        &mut self,
        va: u32,
        access: AccessType,
        bus: &mut Bus,
    ) -> Result<u64, StepResult> {
        // Determine the effective privilege for address translation.
        let effective_priv = if self.privilege == Privilege::Machine {
            if access == AccessType::Fetch {
                // M-mode instruction fetches always use bare mode.
                return Ok(va as u64);
            }
            // For loads/stores in M-mode, check MPRV.
            let mprv = (self.csr.mstatus >> csr::MSTATUS_MPRV) & 1;
            if mprv == 0 {
                // MPRV=0: bare mode, no translation.
                return Ok(va as u64);
            }
            // MPRV=1: use MPP for effective privilege.
            let mpp = ((self.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB) as u8;
            match mpp {
                0 => Privilege::User,
                1 => Privilege::Supervisor,
                _ => Privilege::Machine,
            }
        } else {
            self.privilege
        };

        let sum = (self.csr.mstatus >> csr::MSTATUS_SUM) & 1 != 0;
        let mxr = (self.csr.mstatus >> csr::MSTATUS_MXR) & 1 != 0;
        let satp = self.csr.satp;
        match mmu::translate(
            va,
            access,
            effective_priv,
            sum,
            mxr,
            satp,
            bus,
            &mut self.tlb,
        ) {
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

    /// Read a CSR, intercepting hardware-mapped CSRs like TIME/TIMEH.
    ///
    /// The TIME (0xC01) and TIMEH (0xC81) CSRs map to the CLINT mtime register.
    /// Since the CSR bank has no access to the bus/CLINT, we intercept them here.
    pub(crate) fn read_csr_with_time(&self, addr: u32, bus: &Bus) -> u32 {
        match addr {
            csr::TIME => bus.clint.mtime as u32,
            csr::TIMEH => (bus.clint.mtime >> 32) as u32,
            _ => self.csr.read(addr),
        }
    }

    /// Write to a CSR, intercepting side effects like SATP logging (Phase 41).
    pub(crate) fn write_csr(&mut self, addr: u32, val: u32, bus: &mut Bus) {
        if addr == csr::SATP {
            // Virtual SATP fixup for Linux boot: the kernel's relocate_enable_mmu
            // writes the virtual PPN of trampoline_pg_dir into SATP (e.g., 0xC1484
            // instead of physical 0x1484). On real hardware, OpenSBI handles this;
            // we translate virtual PPNs to physical PPNs here.
            // PAGE_OFFSET for RV32 Linux = 0xC0000000, PPN offset = 0xC0000.
            let mut fixed_val = val;
            let page_offset_ppn: u32 = 0xC000_0000 >> 12; // 0xC0000
            let ppn = val & 0x003F_FFFF;
            if ppn >= page_offset_ppn {
                let phys_ppn = ppn - page_offset_ppn;
                fixed_val = (val & !0x003F_FFFF) | phys_ppn;
            }
            let old = self.csr.satp;
            if old != fixed_val {
                bus.mmu_log.push(mmu::MmuEvent::SatpWrite {
                    old,
                    new: fixed_val,
                });
                // Flush TLB on SATP change to prevent stale translations.
                // While software should SFENCE.VMA, many implementations flush
                // on SATP write to avoid a window of stale entries.
                self.tlb.flush_all();
            }
            self.csr.write(addr, fixed_val);

            // Auto-fixup: scan and fix virtual PPNs in the new page table.
            // This must happen BEFORE any instruction executes with the new SATP,
            // otherwise the kernel's writes go to wrong physical addresses.
            if bus.auto_pte_fixup && (old != fixed_val) {
                let new_ppn = fixed_val & 0x003F_FFFF;
                let pg_dir_phys = (new_ppn as u64) * 4096;
                eprintln!("[cpu] write_csr SATP: auto PTE fixup at pg_dir PA 0x{:08X} (SATP 0x{:08X} -> 0x{:08X})", pg_dir_phys, old, fixed_val);
                bus.fixup_kernel_page_table(pg_dir_phys);
                // Flush TLB again since we modified page table entries.
                self.tlb.flush_all();
            }
            return;
        }
        self.csr.write(addr, val);
    }

    /// Deliver a trap: set cause/epc/tval CSRs, update privilege, jump to vector.
    pub(crate) fn deliver_trap(&mut self, cause: u32, tval: u32) {
        let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
        let vector = self.csr.trap_vector(trap_priv);
        self.csr
            .trap_enter(trap_priv, self.privilege, self.pc, cause);
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
        // Snapshot register state before execution (Phase 41: tracing).
        let regs_before = self.x;
        let pc_before = self.pc;
        let tp_before = self.x[4];

        // Check for pending interrupts before fetching.
        if let Some(cause) = self.csr.pending_interrupt(self.privilege) {
            let trap_priv = self.csr.trap_target_priv(cause, self.privilege);
            let vector = self.csr.trap_vector(trap_priv);
            self.csr
                .trap_enter(trap_priv, self.privilege, self.pc, cause);
            self.privilege = trap_priv;
            self.pc = vector;
            self.last_step = Some(LastStepInfo {
                pc: pc_before,
                word: 0,
                op: Operation::Invalid(0),
                inst_len: 0,
                regs_before,
                regs_after: self.x,
                pc_after: self.pc,
                result: StepResult::Ok,
            });
            return StepResult::Ok;
        }

        // Translate PC through MMU for instruction fetch.
        let fetch_pa = match self.translate_va(self.pc, AccessType::Fetch, bus) {
            Ok(pa) => pa,
            Err(e) => {
                self.last_step = Some(LastStepInfo {
                    pc: pc_before,
                    word: 0,
                    op: Operation::Invalid(0),
                    inst_len: 0,
                    regs_before,
                    regs_after: self.x,
                    pc_after: self.pc,
                    result: e.clone(),
                });
                return e;
            }
        };
        let word = match bus.read_word(fetch_pa) {
            Ok(w) => w,
            Err(_) => {
                self.deliver_trap(csr::CAUSE_FETCH_ACCESS, self.pc);
                self.last_step = Some(LastStepInfo {
                    pc: pc_before,
                    word: 0,
                    op: Operation::Invalid(0),
                    inst_len: 0,
                    regs_before,
                    regs_after: self.x,
                    pc_after: self.pc,
                    result: StepResult::Ok,
                });
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
        let result = self.execute(op, bus, inst_len);

        // Phase 41: Scheduler tracing (infer context switch from tp register change).
        let tp_after = self.x[4];
        if tp_before != tp_after && self.privilege != Privilege::User {
            bus.sched_log.push(SchedEvent::ContextSwitch {
                old_tp: tp_before,
                new_tp: tp_after,
                pc: pc_before,
            });
        }

        self.last_step = Some(LastStepInfo {
            pc: pc_before,
            word,
            op,
            inst_len,
            regs_before,
            regs_after: self.x,
            pc_after: self.pc,
            result: result.clone(),
        });
        result
    }
}

/// Sign-extend a byte to i32.
pub(crate) fn sign_extend_byte(b: u8) -> i32 {
    b as i8 as i32
}

/// Sign-extend a half-word to i32.
pub(crate) fn sign_extend_half(h: u16) -> i32 {
    h as i16 as i32
}

impl Default for RiscvCpu {
    fn default() -> Self {
        Self::new()
    }
}
