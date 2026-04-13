// riscv/cpu.rs -- RV32I CPU state (Phase 34 stub)
//
// Instruction fetch/decode/execute loop will live here.
// See docs/RISCV_HYPERVISOR.md §CPU State.

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
            pc: 0x8000_0000, // default RAM base
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
}

impl Default for RiscvCpu {
    fn default() -> Self {
        Self::new()
    }
}
