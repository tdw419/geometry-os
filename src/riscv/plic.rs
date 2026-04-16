// riscv/plic.rs -- Platform-Level Interrupt Controller (Phase 36)
//
// Emulates a minimal PLIC for a single hart (hart 0).
// Supports up to 32 interrupt sources with configurable priority,
// enable bits, threshold, and claim/complete mechanism.
//
// MMIO address map (base at 0x0C00_0000, matching QEMU virt):
//   0x0C00_0000 - 0x0C00_00FC  Priority registers (32 x 4 bytes)
//   0x0C00_0200               Interrupt pending bit 0-31
//   0x0C00_2000 - 0x0C00_201C  Enable registers for context 0 (hart 0 S-mode)
//   0x0C00_2000               Enable bits 0-31
//   0x0C20_0000               Priority threshold (context 0)
//   0x0C20_0004               Claim/Complete (context 0)

/// PLIC MMIO base address.
pub const PLIC_BASE: u64 = 0x0C00_0000;
/// PLIC MMIO size (2MB region).
pub const PLIC_SIZE: u64 = 0x0020_0000;

/// Maximum number of interrupt sources.
const MAX_SOURCES: usize = 32;

/// Priority register base.
const PRIORITY_BASE: u64 = 0x0C00_0000;
/// Pending register base.
const PENDING_BASE: u64 = 0x0C00_1000;
/// Enable register base (context 0).
const ENABLE_BASE: u64 = 0x0C00_2000;
/// Context 0 threshold.
const CTX0_THRESHOLD: u64 = 0x0C20_0000;
/// Context 0 claim/complete.
const CTX0_CLAIM: u64 = 0x0C20_0004;

/// External interrupt source IDs.
pub const IRQ_UART: u32 = 10; // UART interrupt (matches QEMU virt)
pub const IRQ_VIRTIO: u32 = 1; // Virtio device interrupt

/// PLIC state for a single hart.
#[derive(Clone, Debug)]
pub struct Plic {
    /// Priority for each interrupt source (0 = disabled, 1-7 = priority).
    pub priority: [u32; MAX_SOURCES],
    /// Pending bits (bit N = source N pending).
    pub pending: u32,
    /// Enable bits for context 0 (bit N = source N enabled).
    pub enable: u32,
    /// Priority threshold for context 0.
    pub threshold: u32,
}

impl Default for Plic {
    fn default() -> Self {
        Self::new()
    }
}

impl Plic {
        /// Create a new PLIC with all interrupts disabled and zero priorities.
    pub fn new() -> Self {
        Self {
            priority: [0u32; MAX_SOURCES],
            pending: 0,
            enable: 0,
            threshold: 0,
        }
    }

    /// Check if address is in PLIC range.
    pub fn contains(addr: u64) -> bool {
        (PLIC_BASE..PLIC_BASE + PLIC_SIZE).contains(&addr)
    }

    /// Read a 32-bit word from PLIC MMIO space.
    /// Takes &mut self because claim reads clear the pending bit.
    pub fn read(&mut self, addr: u64) -> Option<u32> {
        if addr >= PRIORITY_BASE && addr < PRIORITY_BASE + (MAX_SOURCES as u64) * 4 {
            let idx = ((addr - PRIORITY_BASE) / 4) as usize;
            if idx < MAX_SOURCES {
                return Some(self.priority[idx]);
            }
        }
        if addr == PENDING_BASE {
            return Some(self.pending);
        }
        if addr == ENABLE_BASE {
            return Some(self.enable);
        }
        if addr == CTX0_THRESHOLD {
            return Some(self.threshold);
        }
        if addr == CTX0_CLAIM {
            // Claim: return highest-priority enabled pending interrupt.
            return Some(self.claim());
        }
        None
    }

    /// Write a 32-bit word to PLIC MMIO space.
    pub fn write(&mut self, addr: u64, val: u32) -> bool {
        if addr >= PRIORITY_BASE && addr < PRIORITY_BASE + (MAX_SOURCES as u64) * 4 {
            let idx = ((addr - PRIORITY_BASE) / 4) as usize;
            if idx < MAX_SOURCES {
                self.priority[idx] = val & 0x07; // Priority 0-7
                return true;
            }
        }
        if addr == PENDING_BASE {
            // Pending is read-only (set by devices, cleared by claim/complete).
            return true;
        }
        if addr == ENABLE_BASE {
            self.enable = val;
            return true;
        }
        if addr == CTX0_THRESHOLD {
            self.threshold = val & 0x07;
            return true;
        }
        if addr == CTX0_CLAIM {
            // Complete: clear the pending bit for this source.
            self.complete(val);
            return true;
        }
        false
    }

    /// Claim the highest-priority enabled pending interrupt.
    /// Returns 0 if no interrupt is pending or all are below threshold.
    /// Per PLIC spec, claiming clears the pending bit for the claimed IRQ.
    pub fn claim(&mut self) -> u32 {
        let mut best_irq: u32 = 0;
        let mut best_pri: u32 = 0;

        for i in 1..MAX_SOURCES {
            if (self.pending & (1 << i)) != 0 && (self.enable & (1 << i)) != 0 {
                let pri = self.priority[i];
                if pri > 0 && pri > best_pri && pri > self.threshold {
                    best_irq = i as u32;
                    best_pri = pri;
                }
            }
        }

        // Clear the pending bit for the claimed interrupt (PLIC spec requirement).
        if best_irq != 0 {
            self.pending &= !(1 << best_irq);
        }

        best_irq
    }

    /// Complete (acknowledge) an interrupt by clearing its pending bit.
    pub fn complete(&mut self, irq: u32) {
        if irq > 0 && (irq as usize) < MAX_SOURCES {
            self.pending &= !(1 << irq);
        }
    }

    /// Signal an interrupt from a device.
    /// Sets the pending bit for the given source.
    pub fn signal(&mut self, irq: u32) {
        if irq > 0 && (irq as usize) < MAX_SOURCES {
            self.pending |= 1 << irq;
        }
    }

    /// Check if there is an enabled, pending interrupt above threshold.
    /// Returns Some(irq) if so. Does NOT claim or clear pending.
    pub fn pending_interrupt(&self) -> Option<u32> {
        let irq = self.find_best_irq();
        if irq > 0 {
            Some(irq)
        } else {
            None
        }
    }

    /// Find the highest-priority enabled pending interrupt (no side effects).
    fn find_best_irq(&self) -> u32 {
        let mut best_irq: u32 = 0;
        let mut best_pri: u32 = 0;
        for i in 1..MAX_SOURCES {
            if (self.pending & (1 << i)) != 0 && (self.enable & (1 << i)) != 0 {
                let pri = self.priority[i];
                if pri > 0 && pri > best_pri && pri > self.threshold {
                    best_irq = i as u32;
                    best_pri = pri;
                }
            }
        }
        best_irq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_plic_defaults() {
        let plic = Plic::new();
        assert_eq!(plic.priority, [0u32; MAX_SOURCES]);
        assert_eq!(plic.pending, 0);
        assert_eq!(plic.enable, 0);
        assert_eq!(plic.threshold, 0);
    }

    #[test]
    fn signal_sets_pending() {
        let mut plic = Plic::new();
        plic.signal(IRQ_UART);
        assert_ne!(plic.pending & (1 << IRQ_UART), 0);
    }

    #[test]
    fn claim_returns_highest_priority() {
        let mut plic = Plic::new();
        plic.priority[1] = 2;
        plic.priority[2] = 5;
        plic.enable = (1 << 1) | (1 << 2);
        plic.signal(1);
        plic.signal(2);
        assert_eq!(plic.claim(), 2); // Higher priority
    }

    #[test]
    fn claim_returns_zero_if_disabled() {
        let mut plic = Plic::new();
        plic.priority[1] = 5;
        plic.signal(1);
        assert_eq!(plic.claim(), 0); // Not enabled
    }

    #[test]
    fn claim_respects_threshold() {
        let mut plic = Plic::new();
        plic.priority[1] = 3;
        plic.enable = 1 << 1;
        plic.threshold = 4; // Threshold higher than priority
        plic.signal(1);
        assert_eq!(plic.claim(), 0);
    }

    #[test]
    fn complete_clears_pending() {
        let mut plic = Plic::new();
        plic.signal(1);
        plic.complete(1);
        assert_eq!(plic.pending & (1 << 1), 0);
    }

    #[test]
    fn read_write_priority() {
        let mut plic = Plic::new();
        plic.write(PRIORITY_BASE + 4, 5); // Source 1 priority
        assert_eq!(plic.read(PRIORITY_BASE + 4), Some(5));
    }

    #[test]
    fn read_write_enable() {
        let mut plic = Plic::new();
        plic.write(ENABLE_BASE, 0xFFFF);
        assert_eq!(plic.read(ENABLE_BASE), Some(0xFFFF));
    }

    #[test]
    fn read_write_threshold() {
        let mut plic = Plic::new();
        plic.write(CTX0_THRESHOLD, 3);
        assert_eq!(plic.read(CTX0_THRESHOLD), Some(3));
    }

    #[test]
    fn claim_via_mmio() {
        let mut plic = Plic::new();
plic.priority[IRQ_UART as usize] = 5;
        plic.enable = 1 << IRQ_UART;
        plic.signal(IRQ_UART);
        assert_eq!(plic.read(CTX0_CLAIM), Some(IRQ_UART));
    }

    #[test]
    fn complete_via_mmio() {
        let mut plic = Plic::new();
        plic.signal(IRQ_UART);
        plic.write(CTX0_CLAIM, IRQ_UART);
        assert_eq!(plic.pending & (1 << IRQ_UART), 0);
    }

    #[test]
    fn pending_interrupt_convenience() {
        let mut plic = Plic::new();
        assert!(plic.pending_interrupt().is_none());
        plic.priority[1] = 5;
        plic.enable = 1 << 1;
        plic.signal(1);
        assert_eq!(plic.pending_interrupt(), Some(1));
    }

    #[test]
    fn priority_masked_to_3_bits() {
        let mut plic = Plic::new();
        plic.write(PRIORITY_BASE + 4, 0xFF);
        assert_eq!(plic.read(PRIORITY_BASE + 4), Some(0x07));
    }

    #[test]
    fn source_0_not_interruptible() {
        let mut plic = Plic::new();
        plic.priority[0] = 7;
        plic.enable = 1;
        plic.signal(0);
        // Source 0 is reserved (no interrupt), claim should skip it
        assert_eq!(plic.claim(), 0);
    }
}
