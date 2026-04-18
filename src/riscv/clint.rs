// riscv/clint.rs -- Core Local Interruptor (Phase 35)
//
// RISC-V CLINT: mtime register, mtimecmp per hart, msip per hart.
// Provides timer and software interrupts.
// MMIO addresses match SiFive CLINT:
//   0x0200_0000  msip (hart 0) -- write bit[0] to trigger software interrupt
//   0x0200_4000  mtimecmp (hart 0) -- 64-bit compare register
//   0x0200_BFF8  mtime -- 64-bit free-running counter

/// CLINT MMIO address constants.
pub const MSIP_BASE: u64 = 0x0200_0000;
pub const MTIMECMP_BASE: u64 = 0x0200_4000;
pub const MTIME_ADDR: u64 = 0x0200_BFF8;

/// Core Local Interruptor.
#[derive(Clone, Debug)]
pub struct Clint {
    /// mtime: free-running 64-bit counter (increments each tick).
    pub mtime: u64,
    /// mtimecmp per hart (only hart 0 for now).
    pub mtimecmp: u64,
    /// msip per hart: bit 0 triggers machine software interrupt.
    pub msip: u32,
}

impl Default for Clint {
    fn default() -> Self {
        Self::new()
    }
}

impl Clint {
    /// Create a new CLINT with mtime=0 and mtimecmp=max (no timer interrupt).
    pub fn new() -> Self {
        Self {
            mtime: 0,
            mtimecmp: u64::MAX, // No timer interrupt until explicitly set
            msip: 0,
        }
    }

    /// Advance mtime by one tick. Returns true if a timer interrupt is pending.
    pub fn tick(&mut self) -> bool {
        self.mtime = self.mtime.wrapping_add(1);
        self.mtime >= self.mtimecmp
    }

    /// Returns true if the machine timer interrupt should fire.
    pub fn timer_pending(&self) -> bool {
        self.mtime >= self.mtimecmp
    }

    /// Returns true if a machine software interrupt is pending.
    pub fn software_pending(&self) -> bool {
        (self.msip & 1) != 0
    }

    /// Read a 32-bit word from CLINT MMIO space.
    /// Returns None if the address is not in CLINT range.
    pub fn read(&self, addr: u64) -> Option<u32> {
        if addr == MSIP_BASE {
            Some(self.msip)
        } else if (MTIMECMP_BASE..MTIMECMP_BASE + 8).contains(&addr) {
            // mtimecmp is 64-bit, return the appropriate 32-bit half
            let off = (addr - MTIMECMP_BASE) as usize;
            if off < 4 {
                Some((self.mtimecmp & 0xFFFF_FFFF) as u32)
            } else {
                Some((self.mtimecmp >> 32) as u32)
            }
        } else if (MTIME_ADDR..MTIME_ADDR + 8).contains(&addr) {
            // mtime is 64-bit
            let off = (addr - MTIME_ADDR) as usize;
            if off < 4 {
                Some((self.mtime & 0xFFFF_FFFF) as u32)
            } else {
                Some((self.mtime >> 32) as u32)
            }
        } else {
            None
        }
    }

    /// Write a 32-bit word to CLINT MMIO space.
    /// Returns true if the address was handled.
    pub fn write(&mut self, addr: u64, val: u32) -> bool {
        if addr == MSIP_BASE {
            self.msip = val & 1;
            true
        } else if (MTIMECMP_BASE..MTIMECMP_BASE + 8).contains(&addr) {
            let off = (addr - MTIMECMP_BASE) as usize;
            if off < 4 {
                self.mtimecmp = (self.mtimecmp & !0xFFFF_FFFF) | (val as u64);
            } else {
                self.mtimecmp = (self.mtimecmp & 0xFFFF_FFFF) | ((val as u64) << 32);
            }
            true
        } else if (MTIME_ADDR..MTIME_ADDR + 8).contains(&addr) {
            let off = (addr - MTIME_ADDR) as usize;
            if off < 4 {
                self.mtime = (self.mtime & !0xFFFF_FFFF) | (val as u64);
            } else {
                self.mtime = (self.mtime & 0xFFFF_FFFF) | ((val as u64) << 32);
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clint_defaults() {
        let clint = Clint::new();
        assert_eq!(clint.mtime, 0);
        assert_eq!(clint.mtimecmp, u64::MAX);
        assert_eq!(clint.msip, 0);
    }

    #[test]
    fn tick_increments_mtime() {
        let mut clint = Clint::new();
        assert_eq!(clint.mtime, 0);
        assert!(!clint.tick());
        assert_eq!(clint.mtime, 1);
    }

    #[test]
    fn timer_fires_when_mtime_ge_mtimecmp() {
        let mut clint = Clint::new();
        clint.mtimecmp = 5;
        assert!(!clint.tick()); // mtime=1
        assert!(!clint.tick()); // mtime=2
        assert!(!clint.tick()); // mtime=3
        assert!(!clint.tick()); // mtime=4
        assert!(clint.tick()); // mtime=5 >= mtimecmp=5
        assert!(clint.timer_pending());
    }

    #[test]
    fn software_interrupt_pending() {
        let mut clint = Clint::new();
        assert!(!clint.software_pending());
        clint.msip = 1;
        assert!(clint.software_pending());
        clint.msip = 0;
        assert!(!clint.software_pending());
    }

    #[test]
    fn read_write_msip() {
        let mut clint = Clint::new();
        assert_eq!(clint.read(MSIP_BASE), Some(0));
        clint.write(MSIP_BASE, 1);
        assert_eq!(clint.read(MSIP_BASE), Some(1));
    }

    #[test]
    fn read_write_mtimecmp_64bit() {
        let mut clint = Clint::new();
        // Write low 32 bits
        assert!(clint.write(MTIMECMP_BASE, 0x0000_0100));
        // Write high 32 bits
        assert!(clint.write(MTIMECMP_BASE + 4, 0x0000_0002));
        assert_eq!(clint.mtimecmp, 0x0000_0002_0000_0100);
        // Read back
        assert_eq!(clint.read(MTIMECMP_BASE), Some(0x0000_0100));
        assert_eq!(clint.read(MTIMECMP_BASE + 4), Some(0x0000_0002));
    }

    #[test]
    fn read_write_mtime_64bit() {
        let mut clint = Clint::new();
        assert!(clint.write(MTIME_ADDR, 0xABCD_EF01));
        assert!(clint.write(MTIME_ADDR + 4, 0x1234_5678));
        assert_eq!(clint.mtime, 0x1234_5678_ABCD_EF01);
        assert_eq!(clint.read(MTIME_ADDR), Some(0xABCD_EF01));
        assert_eq!(clint.read(MTIME_ADDR + 4), Some(0x1234_5678));
    }

    #[test]
    fn read_unmapped_returns_none() {
        let clint = Clint::new();
        assert_eq!(clint.read(0x0200_1000), None);
    }

    #[test]
    fn write_unmapped_returns_false() {
        let mut clint = Clint::new();
        assert!(!clint.write(0x0200_1000, 42));
    }

    #[test]
    fn msip_ignores_upper_bits() {
        let mut clint = Clint::new();
        clint.write(MSIP_BASE, 0xFFFF_FFFF);
        assert_eq!(clint.msip, 1);
    }
}
