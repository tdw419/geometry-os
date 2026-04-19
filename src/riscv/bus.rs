// riscv/bus.rs -- Memory-mapped IO bus (Phase 35)
//
// Routes memory accesses to RAM or device MMIO regions.
// Currently handles: CLINT (timer + software interrupts).
// Phase 36 will add: UART, PLIC, virtio-blk.

use super::clint::Clint;
use super::memory::{GuestMemory, MemoryError};
use super::plic::Plic;
use super::sbi::Sbi;
use super::uart::Uart;
use super::virtio_blk::VirtioBlk;
use std::collections::HashSet;

/// CLINT MMIO address range.
const CLINT_START: u64 = 0x0200_0000;
const CLINT_END: u64 = 0x0201_0000;

/// The system bus: owns RAM and devices, routes accesses.
pub struct Bus {
    /// Guest RAM.
    pub mem: GuestMemory,
    /// Core Local Interruptor (timer + software interrupts).
    pub clint: Clint,
    /// UART 16550 serial port.
    pub uart: Uart,
    /// Platform-Level Interrupt Controller.
    pub plic: Plic,
    /// Virtio block device.
    pub virtio_blk: VirtioBlk,
    /// SBI (Supervisor Binary Interface) handler.
    /// Intercepts SBI ECALLs from the kernel before they reach the trap vector.
    pub sbi: Sbi,
    /// Syscall trace log: records User-mode ECALLs (Linux syscalls).
    /// Populated by the CPU when it detects a U-mode ECALL.
    pub syscall_log: Vec<super::syscall::SyscallEvent>,
    /// MMU trace log: records page table walks and faults.
    pub mmu_log: Vec<super::mmu::MmuEvent>,
    /// Scheduler trace log: records context switches.
    pub sched_log: Vec<super::cpu::SchedEvent>,
    /// Index into syscall_log of the last U-mode ECALL awaiting its return value.
    /// Set when a U-mode ECALL is captured; cleared when SRET returns to U-mode.
    pub pending_syscall_idx: Option<usize>,
    /// Debug write watchpoint: physical address to watch, None = disabled.
    /// When a write hits this address, records the value and last_pc.
    pub write_watch_addr: Option<u64>,
    /// Debug: value written to the watchpoint address.
    pub write_watch_val: u32,
    /// Debug: PC when the watchpoint was hit.
    pub write_watch_pc: u32,
    /// Debug: whether the watchpoint was hit.
    pub write_watch_hit: bool,
    /// When true, the MMU identity-maps virtual addresses below 4MB
    /// (VA == PA) when the page table walk fails. This emulates OpenSBI
    /// firmware behavior for Linux boot, where per-CPU data, device
    /// probes, and early boot structures live in low memory.
    /// Default: false. Set to true by boot_linux_setup().
    pub low_addr_identity_map: bool,
    /// When true, automatically fix virtual PPNs in kernel page tables on SATP write.
    /// Linux's setup_vm() creates PTEs with virtual PPNs (because __pa() is a no-op
    /// without real SBI firmware). This flag triggers a full page table scan on each
    /// SATP write, translating PPNs >= 0xC0000 (virtual) to physical PPNs.
    /// Default: false. Set to true by boot_linux_setup().
    pub auto_pte_fixup: bool,
    /// Known page table physical page addresses (4KB-aligned).
    /// When auto_pte_fixup is true, writes to these pages are intercepted:
    /// PPNs >= 0xC0000 (virtual kernel addresses) are translated to physical PPNs
    /// by subtracting PAGE_OFFSET/4096. New L2 table pages are discovered when
    /// non-leaf PTEs are written and automatically registered.
    /// Populated by fixup_kernel_page_table() and dynamically during writes.
    pub known_pt_pages: HashSet<u64>,
    /// Write-protected physical addresses. Writes to these addresses are silently
    /// dropped. Used to protect DTB early pointers (_dtb_early_va, _dtb_early_pa)
    /// from being clobbered by setup_vm()'s pt_ops write.
    /// Each entry is (physical_address, protected_value). Reads return the
    /// protected value regardless of what's in RAM.
    pub protected_addrs: Vec<(u64, u32)>,
}

impl Bus {
    /// Create a new bus with the given RAM base address and size.
    pub fn new(ram_base: u64, ram_size: usize) -> Self {
        Self {
            mem: GuestMemory::new(ram_base, ram_size),
            clint: Clint::new(),
            uart: Uart::new(),
            plic: Plic::new(),
            virtio_blk: VirtioBlk::new(),
            sbi: Sbi::new(),
            syscall_log: Vec::new(),
            mmu_log: Vec::new(),
            sched_log: Vec::new(),
            pending_syscall_idx: None,
            write_watch_addr: None,
            write_watch_val: 0,
            write_watch_pc: 0,
            write_watch_hit: false,
            low_addr_identity_map: false,
            auto_pte_fixup: false,
            known_pt_pages: HashSet::new(),
            protected_addrs: Vec::new(),
        }
    }

    /// Read a 32-bit word. Routes to device MMIO or RAM.
    /// Takes &mut self because device reads can have side effects
    /// (e.g., UART RBR read consumes the byte and clears Data Ready).
    pub fn read_word(&mut self, addr: u64) -> Result<u32, MemoryError> {
        // Write-protected address check: return protected value for reads.
        for &(pa, protected_val) in &self.protected_addrs {
            if addr == pa {
                return Ok(protected_val);
            }
        }
        if Self::in_clint(addr) {
            self.clint.read(addr).ok_or(MemoryError { addr, size: 4 })
        } else if super::uart::Uart::contains(addr) {
            self.uart
                .read_word(addr)
                .ok_or(MemoryError { addr, size: 4 })
        } else if super::plic::Plic::contains(addr) {
            self.plic.read(addr).ok_or(MemoryError { addr, size: 4 })
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            self.virtio_blk
                .read(addr)
                .ok_or(MemoryError { addr, size: 4 })
        } else if addr < self.mem.ram_base {
            // Return 0 for reads from unmapped addresses below RAM
            Ok(0)
        } else {
            self.mem.read_word(addr)
        }
    }

    /// Write a 32-bit word. Routes to device MMIO or RAM.
    pub fn write_word(&mut self, addr: u64, val: u32) -> Result<(), MemoryError> {
        // Write-protected address check: silently drop writes to protected addresses.
        for &(pa, _protected_val) in &self.protected_addrs {
            if addr == pa {
                return Ok(());
            }
        }
        // Debug write watchpoint
        if let Some(watch) = self.write_watch_addr {
            if addr >= watch && addr < watch + 4 && !self.write_watch_hit {
                self.write_watch_hit = true;
                self.write_watch_val = val;
            }
        }
        if Self::in_clint(addr) {
            if self.clint.write(addr, val) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 4 })
            }
        } else if super::uart::Uart::contains(addr) {
            self.uart.write_word(addr, val);
            Ok(())
        } else if super::plic::Plic::contains(addr) {
            if self.plic.write(addr, val) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 4 })
            }
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            self.virtio_blk.write(addr, val);
            Ok(())
        } else if addr < self.mem.ram_base {
            // Silently accept writes to unmapped addresses below RAM
            Ok(())
        } else {
            // Real-time PTE write interception: if this address is in a known
            // page table page, fix virtual PPNs before storing.
            let fixed_val = self.intercept_pte_write(addr, val);
            self.mem.write_word(addr, fixed_val)
        }
    }

    /// Read a byte. Routes to device MMIO or RAM.
    /// Takes &mut self because device reads can have side effects.
    pub fn read_byte(&mut self, addr: u64) -> Result<u8, MemoryError> {
        if Self::in_clint(addr) {
            let word = self
                .clint
                .read(addr & !3)
                .ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if super::uart::Uart::contains(addr) {
            Ok(self.uart.read_byte(addr - super::uart::UART_BASE))
        } else if super::plic::Plic::contains(addr) {
            let word = self
                .plic
                .read(addr & !3)
                .ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            let word = self
                .virtio_blk
                .read(addr & !3)
                .ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if addr < self.mem.ram_base {
            Ok(0)
        } else {
            self.mem.read_byte(addr)
        }
    }

    /// Write a byte. Routes to device MMIO or RAM.
    pub fn write_byte(&mut self, addr: u64, val: u8) -> Result<(), MemoryError> {
        // Debug write watchpoint (byte-level)
        if let Some(watch) = self.write_watch_addr {
            if addr >= watch && addr < watch + 4 && !self.write_watch_hit {
                self.write_watch_hit = true;
                self.write_watch_val = val as u32; // approximate for byte writes
            }
        }
        if Self::in_clint(addr) {
            let word_addr = addr & !3;
            let byte_off = (addr & 3) as usize;
            let mut word = self.clint.read(word_addr).unwrap_or(0);
            word = (word & !(0xFF << (byte_off * 8))) | ((val as u32) << (byte_off * 8));
            if self.clint.write(word_addr, word) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 1 })
            }
        } else if super::uart::Uart::contains(addr) {
            self.uart.write_byte(addr - super::uart::UART_BASE, val);
            Ok(())
        } else if super::plic::Plic::contains(addr) {
            let word_addr = addr & !3;
            let byte_off = (addr & 3) as usize;
            let mut word = self.plic.read(word_addr).unwrap_or(0);
            word = (word & !(0xFF << (byte_off * 8))) | ((val as u32) << (byte_off * 8));
            if self.plic.write(word_addr, word) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 1 })
            }
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            // Virtio doesn't have byte-level writes; ignore
            Ok(())
        } else if addr < self.mem.ram_base {
            Ok(())
        } else {
            self.mem.write_byte(addr, val)
        }
    }

    /// Read a 16-bit half-word. Routes to device MMIO or RAM.
    pub fn read_half(&mut self, addr: u64) -> Result<u16, MemoryError> {
        if Self::in_clint(addr) {
            let word = self
                .clint
                .read(addr & !3)
                .ok_or(MemoryError { addr, size: 2 })?;
            let half_off = ((addr >> 1) & 1) as usize;
            Ok((word >> (half_off * 16)) as u16)
        } else if addr < self.mem.ram_base {
            Ok(0)
        } else {
            self.mem.read_half(addr)
        }
    }

    /// Write a 16-bit half-word. Routes to device MMIO or RAM.
    pub fn write_half(&mut self, addr: u64, val: u16) -> Result<(), MemoryError> {
        if Self::in_clint(addr) {
            let word_addr = addr & !3;
            let half_off = ((addr >> 1) & 1) as usize;
            let mut word = self.clint.read(word_addr).unwrap_or(0);
            word = (word & !(0xFFFF << (half_off * 16))) | ((val as u32) << (half_off * 16));
            if self.clint.write(word_addr, word) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 2 })
            }
        } else if addr < self.mem.ram_base {
            // Silently accept writes to unmapped addresses below RAM
            Ok(())
        } else {
            self.mem.write_half(addr, val)
        }
    }

    /// Advance the CLINT timer by one tick.
    pub fn tick_clint(&mut self) {
        self.clint.tick();
    }

    /// Advance the CLINT timer by N ticks.
    /// Used during Linux boot to simulate realistic CPU/timebase ratio.
    pub fn tick_clint_n(&mut self, n: u64) {
        self.clint.tick_n(n);
    }

    /// Sync CLINT + PLIC hardware state into the MIP register.
    ///
    /// Sets/clears MTIP (bit 7) based on mtime >= mtimecmp.
    /// Sets/clears MSIP (bit 3) based on msip register.
    /// Sets/clears MEIP (bit 11) based on PLIC pending+enabled interrupts.
    /// Other MIP bits (SSIP, STIP, SEIP) are left unchanged (software-writable).
    pub fn sync_mip(&self, mip: &mut u32) {
        // MTIP (bit 7): machine timer interrupt pending
        if self.clint.timer_pending() {
            *mip |= 1 << 7;
        } else {
            *mip &= !(1 << 7);
        }

        // MSIP (bit 3): machine software interrupt pending
        if self.clint.software_pending() {
            *mip |= 1 << 3;
        } else {
            *mip &= !(1 << 3);
        }

        // MEIP (bit 11): machine external interrupt pending from PLIC.
        // Set whenever PLIC has an enabled, pending interrupt above threshold.
        if self.plic.pending_interrupt().is_some() {
            *mip |= 1 << 11;
        } else {
            *mip &= !(1 << 11);
        }
    }

    fn in_clint(addr: u64) -> bool {
        (CLINT_START..CLINT_END).contains(&addr)
    }

    /// Intercept a word write to check if it targets a known page table page.
    ///
    /// When `auto_pte_fixup` is enabled and the write address falls within a
    /// registered page table page (tracked in `known_pt_pages`), this method:
    /// 1. Checks if the written value is a valid PTE with V=1
    /// 2. If the PPN >= PAGE_OFFSET/4096 (0xC0000), subtracts the offset to
    ///    convert from virtual PPN to physical PPN
    /// 3. If the PTE is a non-leaf entry (R=W=X=0), registers the pointed-to
    ///    L2 table page as a known page table page for future interception
    ///
    /// This handles the Linux demand paging case where the page fault handler
    /// creates NEW PTEs with virtual PPNs after the initial SATP fixup.
    fn intercept_pte_write(&mut self, addr: u64, val: u32) -> u32 {
        if !self.auto_pte_fixup || self.known_pt_pages.is_empty() {
            return val;
        }

        // Check if this address is in a known page table page
        let page_base = addr & !0xFFF; // 4KB page alignment
        if !self.known_pt_pages.contains(&page_base) {
            return val;
        }

        // Check if the value looks like a valid PTE (V=1)
        const PTE_V: u32 = 1;
        const PPN_MASK: u32 = 0xFFFF_FC00;
        const LEAF_FLAGS: u32 = 2 | 4 | 8; // R | W | X
        const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12; // 0xC0000

        if (val & PTE_V) == 0 {
            return val;
        }

        let ppn = (val & PPN_MASK) >> 10;

        // Discover new L2 table pages from non-leaf PTEs.
        // A non-leaf PTE (R=W=X=0, V=1) points to a lower-level page table.
        if (val & LEAF_FLAGS) == 0 && ppn < PAGE_OFFSET_PPN && ppn > 0 {
            // Only register if the PPN is in a reasonable physical range.
            // Skip PPN=0 (kernel code at PA 0, see fixup_kernel_page_table).
            let l2_page_addr = (ppn as u64) << 12;
            if (0x1000..0x1000_0000).contains(&l2_page_addr)
                && self.known_pt_pages.insert(l2_page_addr)
            {
                eprintln!(
                        "[pte_intercept] Discovered new L2 table at PA 0x{:08X} (from PTE write at PA 0x{:08X})",
                        l2_page_addr, addr
                    );
            }
        }

        // Fix virtual PPNs: subtract PAGE_OFFSET/4096 if PPN is in kernel VA range
        if ppn >= PAGE_OFFSET_PPN {
            let fixed_ppn = ppn - PAGE_OFFSET_PPN;
            let fixed_val = (val & !PPN_MASK) | (fixed_ppn << 10);
            eprintln!(
                "[pte_intercept] Fixed PTE at PA 0x{:08X}: PPN 0x{:05X} -> 0x{:05X} (val 0x{:08X} -> 0x{:08X})",
                addr, ppn, fixed_ppn, val, fixed_val
            );

            // If this is a non-leaf PTE, also register the (now fixed) L2 page
            if (val & LEAF_FLAGS) == 0 {
                let l2_page_addr = (fixed_ppn as u64) << 12;
                if (0x1000..0x1000_0000).contains(&l2_page_addr)
                    && self.known_pt_pages.insert(l2_page_addr)
                {
                    eprintln!(
                            "[pte_intercept] Discovered new L2 table at PA 0x{:08X} (fixed from virtual PPN)",
                            l2_page_addr
                        );
                }
            }

            return fixed_val;
        }

        val
    }

    /// Fix virtual PPNs in a kernel page table.
    ///
    /// Linux's setup_vm() creates PTEs with virtual PPNs because __pa() is a no-op
    /// without real SBI firmware. This scans all L1 and L2 entries and subtracts
    /// PAGE_OFFSET (0xC0000000 >> 12 = 0xC0000) from any PPN >= 0xC0000.
    ///
    /// Also registers all discovered page table pages in `known_pt_pages` so that
    /// `intercept_pte_write` can fix future PTE writes (demand paging) in real-time.
    ///
    /// Called automatically from write_csr(SATP) when auto_pte_fixup is true.
    pub fn fixup_kernel_page_table(&mut self, pg_dir_phys: u64) {
        const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12; // 0xC0000
        const PPN_MASK: u32 = 0xFFFF_FC00;
        const LEAF_FLAGS: u32 = 2 | 4 | 8; // R | W | X

        let mut l2_tables_to_fix: Vec<u64> = Vec::new();

        // Register the L1 page directory itself as a known page table page.
        // All PTE writes to this page will be intercepted for virtual PPN fixup.
        self.known_pt_pages.insert(pg_dir_phys);

        // Scan all 1024 L1 entries
        for i in 0..1024u32 {
            let l1_addr = pg_dir_phys + (i as u64) * 4;
            let l1_pte = match self.read_word(l1_addr) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if (l1_pte & 1) == 0 {
                continue;
            }

            let l1_ppn = (l1_pte & PPN_MASK) >> 10;

            // Compute the fixed PPN for this L1 entry
            let final_ppn = if l1_ppn >= PAGE_OFFSET_PPN {
                l1_ppn - PAGE_OFFSET_PPN
            } else {
                l1_ppn
            };

            if l1_ppn >= PAGE_OFFSET_PPN {
                let fixed_pte = (l1_pte & !PPN_MASK) | (final_ppn << 10);
                // Use mem.write_word directly to avoid going through intercept_pte_write
                // (we're doing the fixup manually here, no need for double-fixing).
                self.mem.write_word(l1_addr, fixed_pte).ok();
            }

            // If non-leaf L1 entry, queue the L2 table for fixup and register it
            if (l1_pte & LEAF_FLAGS) == 0 {
                let l2_base = (final_ppn as u64) << 12;
                // Skip PA 0 -- the kernel allocates L2 tables there when
                // memblock returns 0 despite DTB reservations. Registering
                // PA 0 as a known PT page causes ALL writes to the first 4KB
                // of RAM (kernel entry point!) to go through intercept_pte_write,
                // corrupting kernel code.
                if l2_base > 0 && l2_base < 0x1000_0000 {
                    // Register this L2 table page for future write interception
                    self.known_pt_pages.insert(l2_base);
                    l2_tables_to_fix.push(l2_base);
                }
            }
        }

        // Fix L2 tables
        for l2_base in &l2_tables_to_fix {
            for j in 0..1024u32 {
                let l2_addr = *l2_base + (j as u64) * 4;
                let l2_pte = match self.read_word(l2_addr) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if (l2_pte & 1) == 0 {
                    continue;
                }

                let l2_ppn = (l2_pte & PPN_MASK) >> 10;

                if l2_ppn >= PAGE_OFFSET_PPN {
                    let fixed_ppn = l2_ppn - PAGE_OFFSET_PPN;
                    let fixed_pte = (l2_pte & !PPN_MASK) | (fixed_ppn << 10);
                    // Use mem.write_word directly to avoid intercept recursion
                    self.mem.write_word(l2_addr, fixed_pte).ok();
                }
            }
        }

        eprintln!(
            "[pte_fixup] Registered {} page table pages (L1 at PA 0x{:08X})",
            self.known_pt_pages.len(),
            pg_dir_phys
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::clint;
    use super::*;

    #[test]
    fn bus_ram_read_write() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(0x8000_0000, 0xDEAD_BEEF)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x8000_0000)
                .expect("operation should succeed"),
            0xDEAD_BEEF
        );
    }

    #[test]
    fn bus_ram_base_zero_read_write() {
        // With ram_base=0, all physical addresses map directly to RAM
        let mut bus = Bus::new(0, 4096);
        bus.write_word(0x0000_0000, 0xDEAD_BEEF)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x0000_0000)
                .expect("operation should succeed"),
            0xDEAD_BEEF
        );
    }

    #[test]
    fn bus_clint_mmio_mtimecmp() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(clint::MTIMECMP_BASE, 0x0000_0100)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(clint::MTIMECMP_BASE)
                .expect("operation should succeed"),
            0x0000_0100
        );
    }

    #[test]
    fn bus_clint_msip() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(clint::MSIP_BASE, 1)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(clint::MSIP_BASE)
                .expect("operation should succeed"),
            1
        );
        assert!(bus.clint.software_pending());
    }

    #[test]
    fn bus_sync_mip_timer() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.clint.mtimecmp = 0; // Timer fires immediately (mtime=0 >= mtimecmp=0)
        let mut mip = 0u32;
        bus.sync_mip(&mut mip);
        assert_eq!(mip & (1 << 7), 1 << 7, "MTIP should be set");
    }

    #[test]
    fn bus_sync_mip_software() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.clint.msip = 1;
        let mut mip = 0u32;
        bus.sync_mip(&mut mip);
        assert_eq!(mip & (1 << 3), 1 << 3, "MSIP should be set");
    }

    #[test]
    fn bus_sync_mip_clears_when_not_pending() {
        let bus = Bus::new(0x8000_0000, 4096);
        let mut mip: u32 = (1 << 7) | (1 << 3);
        bus.sync_mip(&mut mip);
        assert_eq!(mip & (1 << 7), 0, "MTIP should be cleared");
        assert_eq!(mip & (1 << 3), 0, "MSIP should be cleared");
    }

    #[test]
    fn bus_out_of_range_fails() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        // Low addresses return 0 (unmapped below ram_base)
        assert_eq!(
            bus.read_word(0x0000_0000)
                .expect("operation should succeed"),
            0
        );
        assert!(bus.read_word(0x0200_1000).is_err()); // CLINT gap
    }

    #[test]
    fn bus_tick_advances_mtime() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        assert_eq!(bus.clint.mtime, 0);
        bus.tick_clint();
        assert_eq!(bus.clint.mtime, 1);
    }

    #[test]
    fn pte_intercept_fixes_virtual_ppn() {
        // Simulate Linux demand paging: kernel writes PTE with virtual PPN
        // to a known page table page, and the intercept translates it.
        let mut bus = Bus::new(0, 1024 * 1024); // 1MB RAM, base 0
        bus.auto_pte_fixup = true;

        // Register a page table page at PA 0x1000
        let pt_page = 0x1000u64;
        bus.known_pt_pages.insert(pt_page);

        // Write a leaf PTE with virtual PPN (0xC0000 = 0xC0000000 >> 12)
        // PTE format: V=1, R=1, W=1, X=0, A=1, D=1 = 0x07, PPN = 0xC0000
        // PPN bits [31:10], so val = (0xC0000 << 10) | 0x07 = 0x30000007
        let virtual_pte: u32 = (0xC0000 << 10) | 0x07; // 0x30000007
        bus.write_word(pt_page, virtual_pte)
            .expect("operation should succeed");

        // The intercept should have fixed it: PPN 0xC0000 -> 0x00000
        let stored = bus.read_word(pt_page).expect("operation should succeed");
        let expected: u32 = (0x00000 << 10) | 0x07; // 0x00000007
        assert_eq!(
            stored, expected,
            "Virtual PTE 0x{:08X} should be fixed to 0x{:08X}, got 0x{:08X}",
            virtual_pte, expected, stored
        );
    }

    #[test]
    fn pte_intercept_skips_non_pt_page() {
        // Writes to non-page-table pages should pass through unchanged
        let mut bus = Bus::new(0, 1024 * 1024);
        bus.auto_pte_fixup = true;
        bus.known_pt_pages.insert(0x1000);

        // Write to a non-registered page
        bus.write_word(0x2000, 0xDEADBEEF)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x2000).expect("operation should succeed"),
            0xDEADBEEF
        );
    }

    #[test]
    fn pte_intercept_discovers_new_l2_tables() {
        // Writing a non-leaf PTE should register the pointed-to L2 page
        let mut bus = Bus::new(0, 1024 * 1024);
        bus.auto_pte_fixup = true;

        let l1_page = 0x1000u64;
        bus.known_pt_pages.insert(l1_page);

        // Write a non-leaf PTE pointing to L2 at PA 0x2000
        // Non-leaf: V=1, R=0, W=0, X=0 = 0x01, PPN = 0x2 (PA 0x2000)
        let non_leaf_pte: u32 = (2u32 << 10) | 0x01; // 0x00000801
        bus.write_word(l1_page, non_leaf_pte)
            .expect("operation should succeed");

        // The L2 page should now be registered
        assert!(
            bus.known_pt_pages.contains(&0x2000),
            "L2 page at 0x2000 should be auto-discovered"
        );

        // And subsequent writes to the L2 page should be intercepted
        let virtual_l2_pte: u32 = (0xC0001 << 10) | 0x07; // PPN 0xC0001 -> 0x00001
        bus.write_word(0x2000, virtual_l2_pte)
            .expect("operation should succeed");
        let stored = bus.read_word(0x2000).expect("operation should succeed");
        let expected: u32 = (0x00001 << 10) | 0x07;
        assert_eq!(
            stored, expected,
            "Virtual L2 PTE should be fixed to 0x{:08X}, got 0x{:08X}",
            expected, stored
        );
    }

    #[test]
    fn pte_intercept_disabled_when_flag_off() {
        // When auto_pte_fixup is false, no interception should occur
        let mut bus = Bus::new(0, 1024 * 1024);
        bus.auto_pte_fixup = false;
        bus.known_pt_pages.insert(0x1000);

        let virtual_pte: u32 = (0xC0000 << 10) | 0x07;
        bus.write_word(0x1000, virtual_pte)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x1000).expect("operation should succeed"),
            virtual_pte,
            "PTE should NOT be fixed when auto_pte_fixup is false"
        );
    }

    #[test]
    fn pte_intercept_skips_non_valid_pte() {
        // PTEs with V=0 should pass through unchanged
        let mut bus = Bus::new(0, 1024 * 1024);
        bus.auto_pte_fixup = true;
        bus.known_pt_pages.insert(0x1000);

        let invalid_pte: u32 = (0xC0000 << 10) | 0x00; // V=0
        bus.write_word(0x1000, invalid_pte)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x1000).expect("operation should succeed"),
            invalid_pte,
            "Invalid PTE (V=0) should pass through unchanged"
        );
    }

    #[test]
    fn pte_intercept_skips_low_ppn() {
        // PTEs with PPN < PAGE_OFFSET_PPN should pass through unchanged
        let mut bus = Bus::new(0, 1024 * 1024);
        bus.auto_pte_fixup = true;
        bus.known_pt_pages.insert(0x1000);

        let normal_pte: u32 = (0x500 << 10) | 0x07; // PPN 0x500, well below 0xC0000
        bus.write_word(0x1000, normal_pte)
            .expect("operation should succeed");
        assert_eq!(
            bus.read_word(0x1000).expect("operation should succeed"),
            normal_pte,
            "Low PPN PTE should pass through unchanged"
        );
    }
}
