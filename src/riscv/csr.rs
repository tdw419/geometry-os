// riscv/csr.rs -- CSR (Control and Status Register) bank (Phase 35)
//
// RISC-V privileged CSRs for M-mode and S-mode trap handling.
// sstatus is a restricted view of mstatus per the RISC-V spec.
//
// CSR addresses follow the standard RISC-V privileged specification:
//   0x300  mstatus   Machine status
//   0x305  mtvec     Machine trap vector (base address + mode)
//   0x341  mepc      Machine exception program counter
//   0x342  mcause    Machine trap cause
//   0x100  sstatus   Supervisor status (view of mstatus)
//   0x105  stvec     Supervisor trap vector
//   0x141  sepc      Supervisor exception program counter
//   0x142  scause    Supervisor trap cause
//   0x180  satp       Supervisor address translation and protection

use super::cpu::Privilege;

// ---- CSR address constants ----

pub const MSTATUS: u32 = 0x300;
pub const MTVEC: u32 = 0x305;
pub const MEPC: u32 = 0x341;
pub const MCAUSE: u32 = 0x342;
pub const MTVAL: u32 = 0x343;

pub const SSTATUS: u32 = 0x100;
pub const STVEC: u32 = 0x105;
pub const SEPC: u32 = 0x141;
pub const SSCRATCH: u32 = 0x140;
pub const SCAUSE: u32 = 0x142;
pub const STVAL: u32 = 0x143;
pub const SATP: u32 = 0x180;

// Trap delegation CSRs
pub const MEDELEG: u32 = 0x302;
pub const MIDELEG: u32 = 0x303;

// MIE/MIP interrupt enable/pending registers
pub const MIE: u32 = 0x304;
pub const MIP: u32 = 0x344;
pub const SIE: u32 = 0x104;
pub const SIP: u32 = 0x144;

// ---- mstatus bit masks ----

/// SSTATUS is a restricted view of MSTATUS.
/// Visible bits: SIE (1), SPIE (5), SPP (8), SUM (18), MXR (19).
/// In RV32 the SD bit (31) is read-only and derived.
const SSTATUS_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 8) | (1 << 18) | (1 << 19);

/// SIP is a restricted view of MIP.
/// Visible bits: SSIP (1), STIP (5), SEIP (9).
const SIP_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 9);

/// SIE is a restricted view of MIE.
/// Visible bits: SSIE (1), STIE (5), SEIE (9).
const SIE_MASK: u32 = (1 << 1) | (1 << 5) | (1 << 9);

/// Bit positions in mstatus.
pub const MSTATUS_SIE: u32 = 1; // Supervisor Interrupt Enable
pub const MSTATUS_MIE: u32 = 3; // Machine Interrupt Enable
pub const MSTATUS_SPIE: u32 = 5; // Supervisor Previous IE
pub const MSTATUS_MPIE: u32 = 7; // Machine Previous IE
pub const MSTATUS_SPP: u32 = 8; // Supervisor Previous Privilege (1 bit)
pub const MSTATUS_MPRV: u32 = 17; // Memory Privilege (M-mode uses MPP for loads/stores)
pub const MSTATUS_SUM: u32 = 18; // Supervisor User Memory access
pub const MSTATUS_MXR: u32 = 19; // Make eXecutable Readable
pub const MSTATUS_MPP_LSB: u32 = 11; // Machine Previous Privilege (2 bits: 12:11)
pub const MSTATUS_MPP_MASK: u32 = 0x3 << 11;

/// Trap cause: top bit = interrupt (1) or exception (0), lower 31 bits = code.
pub const MCAUSE_INTERRUPT_BIT: u32 = 1 << 31;

/// Exception codes (lower bits of mcause/scause).
pub const CAUSE_MISALIGNED_FETCH: u32 = 0;
pub const CAUSE_FETCH_ACCESS: u32 = 1;
pub const CAUSE_ILLEGAL_INSTRUCTION: u32 = 2;
pub const CAUSE_BREAKPOINT: u32 = 3;
pub const CAUSE_MISALIGNED_LOAD: u32 = 4;
pub const CAUSE_LOAD_ACCESS: u32 = 5;
pub const CAUSE_MISALIGNED_STORE: u32 = 6;
pub const CAUSE_STORE_ACCESS: u32 = 7;
pub const CAUSE_ECALL_U: u32 = 8;
pub const CAUSE_ECALL_S: u32 = 9;
pub const CAUSE_ECALL_M: u32 = 11;
pub const CAUSE_FETCH_PAGE_FAULT: u32 = 12;
pub const CAUSE_LOAD_PAGE_FAULT: u32 = 13;
pub const CAUSE_STORE_PAGE_FAULT: u32 = 15;

/// Interrupt codes.
pub const INT_SSI: u32 = 1; // Supervisor software interrupt
pub const INT_MSI: u32 = 3; // Machine software interrupt
pub const INT_STI: u32 = 5; // Supervisor timer interrupt
pub const INT_MTI: u32 = 7; // Machine timer interrupt
pub const INT_SEI: u32 = 9; // Supervisor external interrupt
pub const INT_MEI: u32 = 11; // Machine external interrupt

// ---- CSR bank ----

/// Bank of RISC-V Control and Status Registers.
///
/// Each field is the raw u32 value. sstatus is *not* stored separately --
/// reads/writes go through mstatus with the SSTATUS_MASK.
#[derive(Clone, Debug)]
pub struct CsrBank {
    /// Machine status register.
    pub mstatus: u32,
    /// Machine trap vector (base | mode). mode: 0=direct, 1=vectored.
    pub mtvec: u32,
    /// Machine exception program counter.
    pub mepc: u32,
    /// Machine trap cause (interrupt bit + exception code).
    pub mcause: u32,
    /// Machine trap value (e.g., faulting address).
    pub mtval: u32,

    /// Supervisor trap vector.
    pub stvec: u32,
    /// Supervisor exception program counter.
    pub sepc: u32,
    /// Supervisor scratch register (per-hart scratch space).
    pub sscratch: u32,
    /// Supervisor trap cause.
    pub scause: u32,
    /// Supervisor trap value.
    pub stval: u32,
    /// Supervisor address translation and protection.
    /// Bits [31:22] = PPN (page table base), [21] = ASID, [0] = MODE.
    pub satp: u32,

    /// Machine interrupt-enable register.
    /// Bit 1 = SSIE, Bit 3 = MSIE, Bit 5 = STIE, Bit 7 = MTIE,
    /// Bit 9 = SEIE, Bit 11 = MEIE.
    pub mie: u32,

    /// Machine interrupt-pending register.
    /// Same bit layout as MIE. SIP is a restricted view of MIP.
    pub mip: u32,

    /// Machine exception delegation register.
    /// Bit N = 1 means exception N is delegated to S-mode.
    pub medeleg: u32,

    /// Machine interrupt delegation register.
    /// Bit N = 1 means interrupt N is delegated to S-mode.
    pub mideleg: u32,
}

impl Default for CsrBank {
    fn default() -> Self {
        Self::new()
    }
}

impl CsrBank {
    /// Create a new CSR bank with power-on defaults.
    ///
    /// Per the RISC-V spec, most CSRs are 0 at reset. mtvec defaults to
    /// 0 (traps go to address 0, which is handled as a fetch fault in
    /// practice). mstatus MPP field defaults to Machine (0b11).
    pub fn new() -> Self {
        Self {
            mstatus: 0,
            mtvec: 0,
            mepc: 0,
            mcause: 0,
            mtval: 0,
            stvec: 0,
            sepc: 0,
            sscratch: 0,
            scause: 0,
            stval: 0,
            satp: 0,
            mie: 0,
            mip: 0,
            medeleg: 0,
            mideleg: 0,
        }
    }

    /// Read a CSR by address.
    ///
    /// Returns 0 for unrecognized CSR addresses (WIRI -- Writes Ignored,
    /// Reads Ignore in spec; we return 0 for simplicity).
    pub fn read(&self, addr: u32) -> u32 {
        match addr {
            MSTATUS => self.mstatus,
            MTVEC => self.mtvec,
            MEPC => self.mepc,
            MCAUSE => self.mcause,
            MTVAL => self.mtval,
            SSTATUS => self.mstatus & SSTATUS_MASK,
            STVEC => self.stvec,
            SEPC => self.sepc,
            SSCRATCH => self.sscratch,
            SCAUSE => self.scause,
            STVAL => self.stval,
            SATP => self.satp,
            MIE => self.mie,
            MIP => self.mip,
            SIE => self.mie & SIE_MASK,
            SIP => self.mip & SIP_MASK,
            MEDELEG => self.medeleg,
            MIDELEG => self.mideleg,
            _ => 0,
        }
    }

    /// Write a CSR by address.
    ///
    /// Returns false if the CSR address is unrecognized or not writable.
    pub fn write(&mut self, addr: u32, val: u32) -> bool {
        match addr {
            MSTATUS => {
                self.mstatus = val;
                true
            }
            MTVEC => {
                // Only modes 0 (direct) and 1 (vectored) are valid.
                // Mask: base is bits [31:2], mode is bits [1:0].
                self.mtvec = val & !1; // Force direct mode for now
                true
            }
            MEPC => {
                // Must be aligned to instruction boundary (clear low bit).
                self.mepc = val & !1;
                true
            }
            MCAUSE => {
                self.mcause = val;
                true
            }
            MTVAL => {
                self.mtval = val;
                true
            }
            SSTATUS => {
                // Write only the sstatus-visible bits in mstatus.
                self.mstatus = (self.mstatus & !SSTATUS_MASK) | (val & SSTATUS_MASK);
                true
            }
            STVEC => {
                self.stvec = val & !1;
                true
            }
            SEPC => {
                self.sepc = val & !1;
                true
            }
            SSCRATCH => {
                self.sscratch = val;
                true
            }
            SCAUSE => {
                self.scause = val;
                true
            }
            STVAL => {
                self.stval = val;
                true
            }
            SATP => {
                self.satp = val;
                true
            }
            MIE => {
                self.mie = val;
                true
            }
            MIP => {
                // MIP is mostly read-only; only SSIP (bit 1) is writable by S-mode.
                // For simplicity, allow writing (guest firmware often needs this).
                self.mip = val;
                true
            }
            SIE => {
                // Write only the SIE-visible bits in mie.
                self.mie = (self.mie & !SIE_MASK) | (val & SIE_MASK);
                true
            }
            SIP => {
                // Write only the SIP-visible bits in mip.
                self.mip = (self.mip & !SIP_MASK) | (val & SIP_MASK);
                true
            }
            MEDELEG => {
                self.medeleg = val;
                true
            }
            MIDELEG => {
                self.mideleg = val;
                true
            }
            _ => false,
        }
    }

    /// Set bits in a CSR (CSRRS semantics: set bits that are 1 in mask).
    /// Returns the old value. If mask is 0, this is a read-only operation
    /// (no bits changed) -- but we still return the value.
    pub fn set_bits(&mut self, addr: u32, mask: u32) -> u32 {
        let old = self.read(addr);
        if mask != 0 {
            let _ = self.write(addr, old | mask);
        }
        old
    }

    /// Clear bits in a CSR (CSRRC semantics: clear bits that are 1 in mask).
    /// Returns the old value. If mask is 0, this is a read-only operation.
    pub fn clear_bits(&mut self, addr: u32, mask: u32) -> u32 {
        let old = self.read(addr);
        if mask != 0 {
            let _ = self.write(addr, old & !mask);
        }
        old
    }

    /// Read mcause as an exception code (strips interrupt bit).
    pub fn mcause_exception_code(&self) -> u32 {
        self.mcause & !MCAUSE_INTERRUPT_BIT
    }

    /// Read mcause interrupt bit.
    pub fn mcause_is_interrupt(&self) -> bool {
        (self.mcause & MCAUSE_INTERRUPT_BIT) != 0
    }

    /// Determine which privilege level handles a trap.
    /// If the exception/interrupt is delegated to S-mode via medeleg/mideleg,
    /// and the trap comes from U-mode, route to S. Otherwise M.
    pub fn trap_target_priv(&self, cause: u32, current_priv: Privilege) -> Privilege {
        let is_interrupt = (cause & MCAUSE_INTERRUPT_BIT) != 0;
        let code = cause & !MCAUSE_INTERRUPT_BIT;
        if is_interrupt {
            // Check mideleg for this interrupt code
            if (self.mideleg >> code) & 1 != 0 && current_priv != Privilege::Machine {
                Privilege::Supervisor
            } else {
                Privilege::Machine
            }
        } else {
            // Check medeleg for this exception code
            if (self.medeleg >> code) & 1 != 0 && current_priv != Privilege::Machine {
                Privilege::Supervisor
            } else {
                Privilege::Machine
            }
        }
    }

    /// Check if a timer interrupt should be delivered.
    /// Returns Some(interrupt_cause) if an interrupt is pending and enabled,
    /// prioritized: MTI > STI > MSI > SSI.
    pub fn pending_interrupt(&self, current_priv: Privilege) -> Option<u32> {
        let mie_enabled = (self.mstatus >> MSTATUS_MIE) & 1 != 0;
        let sie_enabled = (self.mstatus >> MSTATUS_SIE) & 1 != 0;

        // Machine timer interrupt: MTIP pending, MTIE enabled, MIE enabled
        if (self.mip >> INT_MTI) & 1 != 0
            && (self.mie >> INT_MTI) & 1 != 0
            && mie_enabled
        {
            return Some(MCAUSE_INTERRUPT_BIT | INT_MTI);
        }

        // Supervisor timer interrupt (only if not in M-mode, or if delegated)
        if (self.mip >> INT_STI) & 1 != 0
            && (self.mie >> INT_STI) & 1 != 0
            && sie_enabled
            && current_priv != Privilege::Machine
        {
            return Some(MCAUSE_INTERRUPT_BIT | INT_STI);
        }

        // Machine software interrupt
        if (self.mip >> INT_MSI) & 1 != 0
            && (self.mie >> INT_MSI) & 1 != 0
            && mie_enabled
        {
            return Some(MCAUSE_INTERRUPT_BIT | INT_MSI);
        }

        // Supervisor software interrupt
        if (self.mip >> INT_SSI) & 1 != 0
            && (self.mie >> INT_SSI) & 1 != 0
            && sie_enabled
            && current_priv != Privilege::Machine
        {
            return Some(MCAUSE_INTERRUPT_BIT | INT_SSI);
        }

        // Machine external interrupt (from PLIC)
        if (self.mip >> INT_MEI) & 1 != 0 && (self.mie >> INT_MEI) & 1 != 0 && mie_enabled {
            return Some(MCAUSE_INTERRUPT_BIT | INT_MEI);
        }

        // Supervisor external interrupt (from PLIC, delegated)
        if (self.mip >> INT_SEI) & 1 != 0
            && (self.mie >> INT_SEI) & 1 != 0
            && sie_enabled
            && current_priv != Privilege::Machine
        {
            return Some(MCAUSE_INTERRUPT_BIT | INT_SEI);
        }

        None
    }

    /// Get the trap vector PC for the current privilege level.
    /// For direct mode, all traps go to BASE.
    /// Returns (vector_pc, isvectored).
    pub fn trap_vector(&self, priv_level: Privilege) -> u32 {
        let (base, mode) = match priv_level {
            Privilege::Machine => (self.mtvec & !0x3, self.mtvec & 0x3),
            Privilege::Supervisor => (self.stvec & !0x3, self.stvec & 0x3),
            Privilege::User => (self.mtvec & !0x3, self.mtvec & 0x3), // U traps to M
        };
        let _ = mode; // Only direct mode supported; always return base
        base
    }

    /// Push privilege state for a trap (xPP <- current, xPIE <- xIE, xIE <- 0).
    /// Used by ECALL/trap entry.
    pub fn trap_enter(&mut self, trap_priv: Privilege, current_priv: Privilege, pc: u32, cause: u32) {
        match trap_priv {
            Privilege::Machine => {
                // MPP <- current_priv, MPIE <- MIE, MIE <- 0
                let mpp = (current_priv as u32 & 0x3) << MSTATUS_MPP_LSB;
                self.mstatus = (self.mstatus & !MSTATUS_MPP_MASK) | mpp;
                let mie = (self.mstatus >> MSTATUS_MIE) & 1;
                self.mstatus = (self.mstatus & !(1 << MSTATUS_MPIE))
                    | (mie << MSTATUS_MPIE);
                self.mstatus &= !(1 << MSTATUS_MIE);
                self.mepc = pc & !1;
                self.mcause = cause;
            }
            Privilege::Supervisor => {
                // SPP <- current_priv (0=U, 1=S), SPIE <- SIE, SIE <- 0
                let spp = if current_priv == Privilege::Supervisor { 1 } else { 0 };
                self.mstatus = (self.mstatus & !(1 << MSTATUS_SPP))
                    | (spp << MSTATUS_SPP);
                let sie = (self.mstatus >> MSTATUS_SIE) & 1;
                self.mstatus = (self.mstatus & !(1 << MSTATUS_SPIE))
                    | (sie << MSTATUS_SPIE);
                self.mstatus &= !(1 << MSTATUS_SIE);
                self.sepc = pc & !1;
                self.scause = cause;
            }
            Privilege::User => {
                // U-mode doesn't handle traps; no-op.
            }
        }
    }

    /// Pop privilege state for MRET/SRET (restore xIE from xPIE, privilege from xPP).
    /// Returns the restored privilege level.
    pub fn trap_return(&mut self, from_priv: Privilege) -> Privilege {
        match from_priv {
            Privilege::Machine => {
                // Restore MIE from MPIE, set MPIE=1, restore MPP
                let mpie = (self.mstatus >> MSTATUS_MPIE) & 1;
                self.mstatus = (self.mstatus & !(1 << MSTATUS_MIE))
                    | (mpie << MSTATUS_MIE);
                self.mstatus |= 1 << MSTATUS_MPIE; // MPIE = 1
                let mpp = ((self.mstatus & MSTATUS_MPP_MASK) >> MSTATUS_MPP_LSB) as u8;
                self.mstatus &= !MSTATUS_MPP_MASK; // MPP = U (00)
                match mpp {
                    0 => Privilege::User,
                    1 => Privilege::Supervisor,
                    3 => Privilege::Machine,
                    _ => Privilege::User, // Reserved, treat as U
                }
            }
            Privilege::Supervisor => {
                // Restore SIE from SPIE, set SPIE=1, restore SPP
                let spie = (self.mstatus >> MSTATUS_SPIE) & 1;
                self.mstatus = (self.mstatus & !(1 << MSTATUS_SIE))
                    | (spie << MSTATUS_SIE);
                self.mstatus |= 1 << MSTATUS_SPIE; // SPIE = 1
                let spp = (self.mstatus >> MSTATUS_SPP) & 1;
                self.mstatus &= !(1 << MSTATUS_SPP); // SPP = 0 (U)
                if spp == 1 {
                    Privilege::Supervisor
                } else {
                    Privilege::User
                }
            }
            Privilege::User => Privilege::User, // U-mode can't return
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_csr_bank_defaults() {
        let csr = CsrBank::new();
        assert_eq!(csr.mstatus, 0);
        assert_eq!(csr.mtvec, 0);
        assert_eq!(csr.mepc, 0);
        assert_eq!(csr.mcause, 0);
        assert_eq!(csr.mtval, 0);
        assert_eq!(csr.stvec, 0);
        assert_eq!(csr.sepc, 0);
        assert_eq!(csr.scause, 0);
        assert_eq!(csr.stval, 0);
        assert_eq!(csr.satp, 0);
    }

    #[test]
    fn read_write_mstatus() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MSTATUS, 0xFF));
        assert_eq!(csr.read(MSTATUS), 0xFF);
    }

    #[test]
    fn read_write_mtvec() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MTVEC, 0x80000100));
        assert_eq!(csr.read(MTVEC), 0x80000100); // direct mode
    }

    #[test]
    fn read_write_mepc() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MEPC, 0x80000101));
        assert_eq!(csr.read(MEPC), 0x80000100); // LSB cleared
    }

    #[test]
    fn read_write_mcause() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MCAUSE, 0x80000008)); // interrupt + ecall-u
        assert_eq!(csr.read(MCAUSE), 0x80000008);
        assert!(csr.mcause_is_interrupt());
        assert_eq!(csr.mcause_exception_code(), 8);
    }

    #[test]
    fn sstatus_is_view_of_mstatus() {
        let mut csr = CsrBank::new();
        // Write to sstatus should only affect sstatus-visible bits
        csr.write(SSTATUS, 0xFFFFFFFF);
        // mstatus should have only SSTATUS_MASK bits set
        assert_eq!(csr.mstatus, SSTATUS_MASK);
        // sstatus reads back the same
        assert_eq!(csr.read(SSTATUS), SSTATUS_MASK);

        // Write to mstatus sets bits outside sstatus view
        csr.mstatus = 0xFFFFFFFF;
        // sstatus should still only show the masked bits
        assert_eq!(csr.read(SSTATUS), SSTATUS_MASK);
    }

    #[test]
    fn set_bits_csrrs() {
        let mut csr = CsrBank::new();
        csr.write(MSTATUS, 0);
        let old = csr.set_bits(MSTATUS, 1 << MSTATUS_MIE);
        assert_eq!(old, 0);
        assert_eq!(csr.read(MSTATUS), 1 << MSTATUS_MIE);
    }

    #[test]
    fn set_bits_mask_zero_is_read_only() {
        let mut csr = CsrBank::new();
        csr.write(MSTATUS, 0xAB);
        let old = csr.set_bits(MSTATUS, 0);
        assert_eq!(old, 0xAB);
        assert_eq!(csr.read(MSTATUS), 0xAB); // unchanged
    }

    #[test]
    fn clear_bits_csrrc() {
        let mut csr = CsrBank::new();
        csr.write(MSTATUS, 0xFF);
        let old = csr.clear_bits(MSTATUS, 1 << MSTATUS_MIE);
        assert_eq!(old, 0xFF);
        assert_eq!(csr.read(MSTATUS) & (1 << MSTATUS_MIE), 0);
    }

    #[test]
    fn trap_enter_saves_state() {
        let mut csr = CsrBank::new();
        csr.write(MSTATUS, 1 << MSTATUS_MIE); // MIE=1
        csr.trap_enter(Privilege::Machine, Privilege::User, 0x80001000, CAUSE_ECALL_U);

        assert_eq!(csr.mepc, 0x80001000);
        assert_eq!(csr.mcause, CAUSE_ECALL_U);
        // MPP should be User (0)
        assert_eq!((csr.mstatus & MSTATUS_MPP_MASK) >> MSTATUS_MPP_LSB, 0);
        // MPIE should have old MIE (1)
        assert_eq!((csr.mstatus >> MSTATUS_MPIE) & 1, 1);
        // MIE should be 0
        assert_eq!((csr.mstatus >> MSTATUS_MIE) & 1, 0);
    }

    #[test]
    fn trap_return_mrestores_state() {
        let mut csr = CsrBank::new();
        // Simulate trap entry: M-mode from S-mode
        csr.mstatus = 0;
        csr.mstatus |= (Privilege::Supervisor as u32) << MSTATUS_MPP_LSB; // MPP=S
        csr.mstatus |= 1 << MSTATUS_MPIE; // MPIE=1

        let restored = csr.trap_return(Privilege::Machine);
        assert_eq!(restored, Privilege::Supervisor);
        // MIE should be restored from MPIE (1)
        assert_eq!((csr.mstatus >> MSTATUS_MIE) & 1, 1);
        // MPIE should be 1
        assert_eq!((csr.mstatus >> MSTATUS_MPIE) & 1, 1);
        // MPP should be 0 (U)
        assert_eq!((csr.mstatus & MSTATUS_MPP_MASK) >> MSTATUS_MPP_LSB, 0);
    }

    #[test]
    fn trap_enter_supervisor() {
        let mut csr = CsrBank::new();
        csr.mstatus = 1 << MSTATUS_SIE; // SIE=1
        csr.trap_enter(Privilege::Supervisor, Privilege::User, 0x80002000, CAUSE_ECALL_U);

        assert_eq!(csr.sepc, 0x80002000);
        assert_eq!(csr.scause, CAUSE_ECALL_U);
        // SPP should be 0 (User)
        assert_eq!((csr.mstatus >> MSTATUS_SPP) & 1, 0);
        // SPIE should have old SIE (1)
        assert_eq!((csr.mstatus >> MSTATUS_SPIE) & 1, 1);
        // SIE should be 0
        assert_eq!((csr.mstatus >> MSTATUS_SIE) & 1, 0);
    }

    #[test]
    fn trap_return_sret() {
        let mut csr = CsrBank::new();
        csr.mstatus = 0;
        csr.mstatus |= 1 << MSTATUS_SPP; // SPP=S
        csr.mstatus |= 1 << MSTATUS_SPIE; // SPIE=1

        let restored = csr.trap_return(Privilege::Supervisor);
        assert_eq!(restored, Privilege::Supervisor);
        // SIE restored from SPIE
        assert_eq!((csr.mstatus >> MSTATUS_SIE) & 1, 1);
        // SPP cleared to 0
        assert_eq!((csr.mstatus >> MSTATUS_SPP) & 1, 0);
    }

    #[test]
    fn unrecognized_csr_returns_zero() {
        let csr = CsrBank::new();
        assert_eq!(csr.read(0x999), 0);
    }

    #[test]
    fn unrecognized_csr_write_fails() {
        let mut csr = CsrBank::new();
        assert!(!csr.write(0x999, 42));
    }

    #[test]
    fn trap_vector_machine() {
        let mut csr = CsrBank::new();
        csr.mtvec = 0x80000200;
        assert_eq!(csr.trap_vector(Privilege::Machine), 0x80000200);
    }

    #[test]
    fn trap_vector_user_goes_to_machine() {
        let mut csr = CsrBank::new();
        csr.mtvec = 0x80000400;
        assert_eq!(csr.trap_vector(Privilege::User), 0x80000400);
    }

    #[test]
    fn trap_vector_supervisor() {
        let mut csr = CsrBank::new();
        csr.stvec = 0x80000600;
        assert_eq!(csr.trap_vector(Privilege::Supervisor), 0x80000600);
    }

    #[test]
    fn satp_read_write() {
        let mut csr = CsrBank::new();
        assert!(csr.write(SATP, 0x80000000)); // MODE=SV32, PPN=0
        assert_eq!(csr.read(SATP), 0x80000000);
    }

    #[test]
    fn new_csr_bank_includes_interrupt_regs() {
        let csr = CsrBank::new();
        assert_eq!(csr.mie, 0);
        assert_eq!(csr.mip, 0);
        assert_eq!(csr.medeleg, 0);
        assert_eq!(csr.mideleg, 0);
    }

    #[test]
    fn read_write_mie() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MIE, 1 << INT_MTI)); // Enable machine timer interrupt
        assert_eq!(csr.read(MIE), 1 << INT_MTI);
    }

    #[test]
    fn read_write_mip() {
        let mut csr = CsrBank::new();
        assert!(csr.write(MIP, 1 << INT_MTI)); // Machine timer interrupt pending
        assert_eq!(csr.read(MIP), 1 << INT_MTI);
    }

    #[test]
    fn sie_is_view_of_mie() {
        let mut csr = CsrBank::new();
        // Write to MIE with both M-mode and S-mode bits
        csr.mie = (1 << INT_MTI) | (1 << INT_STI);
        // SIE should only show S-mode bits (STI at bit 5)
        assert_eq!(csr.read(SIE), 1 << INT_STI);

        // Write to SIE should only affect S-mode bits
        assert!(csr.write(SIE, 1 << INT_SSI)); // Enable SSIE
        // MIE should now have STI cleared and SSI set, MTI unchanged
        assert_eq!(csr.mie, (1 << INT_MTI) | (1 << INT_SSI));
    }

    #[test]
    fn sip_is_view_of_mip() {
        let mut csr = CsrBank::new();
        csr.mip = (1 << INT_MSI) | (1 << INT_SSI);
        // SIP should only show S-mode bits (SSI at bit 1)
        assert_eq!(csr.read(SIP), 1 << INT_SSI);

        // Write to SIP should only affect S-mode bits
        assert!(csr.write(SIP, 1 << INT_STI)); // Set STIP
        // MIP should now have MSI unchanged, SSI cleared, STI set
        assert_eq!(csr.mip, (1 << INT_MSI) | (1 << INT_STI));
    }

    #[test]
    fn read_write_medeleg() {
        let mut csr = CsrBank::new();
        // Delegate ECALL-U (cause 8) to S-mode
        assert!(csr.write(MEDELEG, 1 << CAUSE_ECALL_U));
        assert_eq!(csr.read(MEDELEG), 1 << CAUSE_ECALL_U);
    }

    #[test]
    fn read_write_mideleg() {
        let mut csr = CsrBank::new();
        // Delegate supervisor timer interrupt to S-mode
        assert!(csr.write(MIDELEG, 1 << INT_STI));
        assert_eq!(csr.read(MIDELEG), 1 << INT_STI);
    }

    #[test]
    fn trap_target_priv_no_delegation() {
        let csr = CsrBank::new(); // No delegation set
        // ECALL from U goes to M (no delegation)
        assert_eq!(
            csr.trap_target_priv(CAUSE_ECALL_U, Privilege::User),
            Privilege::Machine
        );
    }

    #[test]
    fn trap_target_priv_delegated_exception() {
        let mut csr = CsrBank::new();
        // Delegate ECALL-U to S-mode
        csr.medeleg = 1 << CAUSE_ECALL_U;
        // ECALL from U goes to S (delegated)
        assert_eq!(
            csr.trap_target_priv(CAUSE_ECALL_U, Privilege::User),
            Privilege::Supervisor
        );
        // ECALL from S still goes to M (not delegated for S-mode)
        assert_eq!(
            csr.trap_target_priv(CAUSE_ECALL_S, Privilege::Supervisor),
            Privilege::Machine
        );
    }

    #[test]
    fn trap_target_priv_delegated_interrupt() {
        let mut csr = CsrBank::new();
        csr.mideleg = 1 << INT_STI;
        let cause = MCAUSE_INTERRUPT_BIT | INT_STI;
        // Timer interrupt from U delegated to S
        assert_eq!(
            csr.trap_target_priv(cause, Privilege::User),
            Privilege::Supervisor
        );
    }

    #[test]
    fn trap_target_priv_m_mode_always_traps_to_m() {
        let mut csr = CsrBank::new();
        csr.medeleg = 0xFFFF; // Delegate everything
        // M-mode exception still goes to M
        assert_eq!(
            csr.trap_target_priv(CAUSE_ECALL_U, Privilege::Machine),
            Privilege::Machine
        );
    }

    #[test]
    fn pending_interrupt_none_when_disabled() {
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_MTI; // Timer pending
        csr.mie = 1 << INT_MTI; // Timer enabled
        // But MIE bit in mstatus is 0
        assert!(csr.pending_interrupt(Privilege::Machine).is_none());
    }

    #[test]
    fn pending_interrupt_timer_fires() {
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_MTI; // Timer pending
        csr.mie = 1 << INT_MTI; // Timer enabled
        csr.mstatus = 1 << MSTATUS_MIE; // Global MIE enabled
        let cause = csr.pending_interrupt(Privilege::Machine).unwrap();
        assert_eq!(cause, MCAUSE_INTERRUPT_BIT | INT_MTI);
    }

    #[test]
    fn pending_interrupt_software_fires() {
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_SSI; // Software pending
        csr.mie = 1 << INT_SSI; // Software enabled
        csr.mstatus = 1 << MSTATUS_SIE; // SIE enabled
        let cause = csr.pending_interrupt(Privilege::User).unwrap();
        assert_eq!(cause, MCAUSE_INTERRUPT_BIT | INT_SSI);
    }

    #[test]
    fn pending_interrupt_nothing_pending() {
        let csr = CsrBank::new();
        assert!(csr.pending_interrupt(Privilege::Machine).is_none());
    }

    #[test]
    fn pending_interrupt_mei_fires() {
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_MEI; // External interrupt pending
        csr.mie = 1 << INT_MEI; // MEIE enabled
        csr.mstatus = 1 << MSTATUS_MIE; // Global MIE enabled
        let cause = csr.pending_interrupt(Privilege::Machine).unwrap();
        assert_eq!(cause, MCAUSE_INTERRUPT_BIT | INT_MEI);
    }

    #[test]
    fn pending_interrupt_sei_fires() {
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_SEI; // Supervisor external pending
        csr.mie = 1 << INT_SEI; // SEIE enabled
        csr.mstatus = 1 << MSTATUS_SIE; // SIE enabled
        let cause = csr.pending_interrupt(Privilege::User).unwrap();
        assert_eq!(cause, MCAUSE_INTERRUPT_BIT | INT_SEI);
    }

    #[test]
    fn pending_interrupt_mei_not_in_s_mode() {
        // SEI should not fire when in M-mode
        let mut csr = CsrBank::new();
        csr.mip = 1 << INT_SEI;
        csr.mie = 1 << INT_SEI;
        csr.mstatus = 1 << MSTATUS_SIE;
        assert!(csr.pending_interrupt(Privilege::Machine).is_none());
    }

    #[test]
    fn pending_interrupt_priority_mti_over_mei() {
        // MTI should be returned before MEI (higher priority in our check order)
        let mut csr = CsrBank::new();
        csr.mip = (1 << INT_MTI) | (1 << INT_MEI);
        csr.mie = (1 << INT_MTI) | (1 << INT_MEI);
        csr.mstatus = 1 << MSTATUS_MIE;
        let cause = csr.pending_interrupt(Privilege::Machine).unwrap();
        assert_eq!(cause, MCAUSE_INTERRUPT_BIT | INT_MTI);
    }
}
