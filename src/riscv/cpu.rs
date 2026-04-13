// riscv/cpu.rs -- RV32I CPU state (Phase 34)
//
// Instruction fetch/decode/execute loop will live here.
// See docs/RISCV_HYPERVISOR.md §CPU State.

/// Privilege level constants (RISC-V spec).
pub mod priv_level {
    pub const USER: u8 = 0;
    pub const SUPERVISOR: u8 = 1;
    pub const MACHINE: u8 = 3;
}

/// RV32I CPU state.
pub struct RiscvCpu {
    /// General-purpose registers x[0..32]. x[0] is hardwired to zero.
    pub x: [u32; 32],
    /// Program counter.
    pub pc: u32,
    /// Current privilege level (0=User, 1=Supervisor, 3=Machine).
    pub privilege: u8,
}

impl RiscvCpu {
    /// Create a new CPU in Machine mode with PC at the default entry point.
    pub fn new() -> Self {
        Self {
            x: [0u32; 32],
            pc: 0x8000_0000, // default RAM base
            privilege: priv_level::MACHINE, // M-mode = 3
        }
    }

    /// Write to register `rd`. Writes to x[0] are silently discarded.
    pub fn write_reg(&mut self, rd: u8, val: u32) {
        if rd != 0 {
            self.x[rd as usize] = val;
        }
    }

    /// Read register `rs`. x[0] always returns 0.
    pub fn read_reg(&self, rs: u8) -> u32 {
        if rs == 0 {
            0
        } else {
            self.x[rs as usize]
        }
    }
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
    fn new_initializes_pc() {
        let cpu = RiscvCpu::new();
        assert_eq!(cpu.pc, 0x8000_0000);
    }

    #[test]
    fn new_initializes_privilege_machine() {
        let cpu = RiscvCpu::new();
        assert_eq!(cpu.privilege, 3);
    }

    #[test]
    fn x0_always_reads_zero() {
        let mut cpu = RiscvCpu::new();
        // Direct write to x[0] (should be zeroed after)
        cpu.x[0] = 0xDEAD_BEEF;
        assert_eq!(cpu.read_reg(0), 0);
    }

    #[test]
    fn write_reg_to_x0_is_discarded() {
        let mut cpu = RiscvCpu::new();
        cpu.write_reg(0, 0xDEAD_BEEF);
        assert_eq!(cpu.x[0], 0);
    }

    #[test]
    fn write_reg_to_normal_reg_works() {
        let mut cpu = RiscvCpu::new();
        cpu.write_reg(5, 42);
        assert_eq!(cpu.read_reg(5), 42);
    }

    #[test]
    fn default_matches_new() {
        let cpu = RiscvCpu::default();
        assert_eq!(cpu.pc, 0x8000_0000);
        assert_eq!(cpu.privilege, 3);
        assert_eq!(cpu.read_reg(0), 0);
    }

    #[test]
    fn all_regs_zero_on_init() {
        let cpu = RiscvCpu::new();
        for i in 1..32u8 {
            assert_eq!(cpu.read_reg(i), 0, "x[{}] should be 0", i);
        }
    }
}
