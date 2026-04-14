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

/// CLINT MMIO address range.
const CLINT_START: u64 = 0x0200_0000;
const CLINT_END: u64 = 0x0201_0000;

/// Firmware ROM: 1MB at address 0x00000000-0x000FFFFF.
/// The Linux kernel writes firmware stubs (e.g., mret instructions) at low
/// addresses during early boot, then jumps to them expecting OpenSBI to be
/// there. Without a real firmware, we need to store and return those writes.
const FW_ROM_SIZE: usize = 0x100_000; // 1MB

/// The system bus: owns RAM and devices, routes accesses.
pub struct Bus {
    /// Guest RAM.
    pub mem: GuestMemory,
    /// Firmware ROM at 0x00000000-0x000FFFFF.
    /// Stores firmware stubs that the kernel writes during early boot.
    pub fw_rom: Vec<u8>,
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
}

impl Bus {
    /// Create a new bus with the given RAM base address and size.
    pub fn new(ram_base: u64, ram_size: usize) -> Self {
        Self {
            mem: GuestMemory::new(ram_base, ram_size),
            fw_rom: vec![0u8; FW_ROM_SIZE],
            clint: Clint::new(),
            uart: Uart::new(),
            plic: Plic::new(),
            virtio_blk: VirtioBlk::new(),
            sbi: Sbi::new(),
            syscall_log: Vec::new(),
            mmu_log: Vec::new(),
            sched_log: Vec::new(),
            pending_syscall_idx: None,
        }
    }

    /// Check if address falls in firmware ROM range (0x00000000-0x000FFFFF).
    fn in_fw_rom(addr: u64) -> bool {
        addr < FW_ROM_SIZE as u64
    }

    /// Read a byte from firmware ROM.
    fn fw_rom_read(&self, addr: u64) -> u8 {
        let idx = addr as usize;
        if idx < self.fw_rom.len() {
            self.fw_rom[idx]
        } else {
            0
        }
    }

    /// Write a byte to firmware ROM.
    fn fw_rom_write(&mut self, addr: u64, val: u8) {
        let idx = addr as usize;
        if idx < self.fw_rom.len() {
            self.fw_rom[idx] = val;
        }
    }

    /// Read a 32-bit word. Routes to device MMIO or RAM.
    pub fn read_word(&self, addr: u64) -> Result<u32, MemoryError> {
        if Self::in_clint(addr) {
            self.clint.read(addr).ok_or(MemoryError { addr, size: 4 })
        } else if super::uart::Uart::contains(addr) {
            // UART reads need &mut due to side effects (clearing DR).
            // We clone to work around borrow checker.
            let mut uart = self.uart.clone();
            
            // Note: side effects are lost due to clone. This is acceptable
            // for page table walks which shouldn't touch UART.
            uart.read_word(addr).ok_or(MemoryError { addr, size: 4 })
        } else if super::plic::Plic::contains(addr) {
            self.plic.read(addr).ok_or(MemoryError { addr, size: 4 })
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            self.virtio_blk
                .read(addr)
                .ok_or(MemoryError { addr, size: 4 })
        } else if Self::in_fw_rom(addr) {
            // Read from firmware ROM (stores firmware stubs written by kernel)
            let b0 = self.fw_rom_read(addr) as u32;
            let b1 = self.fw_rom_read(addr + 1) as u32;
            let b2 = self.fw_rom_read(addr + 2) as u32;
            let b3 = self.fw_rom_read(addr + 3) as u32;
            Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
        } else if addr < self.mem.ram_base && !Self::in_clint(addr) {
            // Return 0 for reads from unmapped low addresses
            Ok(0)
        } else {
            self.mem.read_word(addr)
        }
    }

    /// Write a 32-bit word. Routes to device MMIO or RAM.
    pub fn write_word(&mut self, addr: u64, val: u32) -> Result<(), MemoryError> {
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
        } else if Self::in_fw_rom(addr) {
            // Store in firmware ROM (kernel writes firmware stubs here)
            self.fw_rom_write(addr, (val & 0xFF) as u8);
            self.fw_rom_write(addr + 1, ((val >> 8) & 0xFF) as u8);
            self.fw_rom_write(addr + 2, ((val >> 16) & 0xFF) as u8);
            self.fw_rom_write(addr + 3, ((val >> 24) & 0xFF) as u8);
            Ok(())
        } else {
            self.mem.write_word(addr, val)
        }
    }

    /// Read a byte. Routes to device MMIO or RAM.
    pub fn read_byte(&self, addr: u64) -> Result<u8, MemoryError> {
        if Self::in_clint(addr) {
            let word = self.clint.read(addr & !3).ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if super::uart::Uart::contains(addr) {
            let mut uart = self.uart.clone();
            Ok(uart.read_byte(addr - super::uart::UART_BASE))
        } else if super::plic::Plic::contains(addr) {
            let word = self.plic.read(addr & !3).ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if super::virtio_blk::VirtioBlk::contains(addr) {
            let word = self.virtio_blk.read(addr & !3).ok_or(MemoryError { addr, size: 1 })?;
            let byte_off = (addr & 3) as usize;
            Ok((word >> (byte_off * 8)) as u8)
        } else if Self::in_fw_rom(addr) {
            Ok(self.fw_rom_read(addr))
        } else if addr < self.mem.ram_base && !Self::in_clint(addr) {
            Ok(0)
        } else {
            self.mem.read_byte(addr)
        }
    }

    /// Write a byte. Routes to device MMIO or RAM.
    pub fn write_byte(&mut self, addr: u64, val: u8) -> Result<(), MemoryError> {
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
        } else if Self::in_fw_rom(addr) {
            self.fw_rom_write(addr, val);
            Ok(())
        } else {
            self.mem.write_byte(addr, val)
        }
    }

    /// Read a 16-bit half-word. Routes to device MMIO or RAM.
    pub fn read_half(&self, addr: u64) -> Result<u16, MemoryError> {
        if Self::in_clint(addr) {
            let word = self.clint.read(addr & !3).ok_or(MemoryError { addr, size: 2 })?;
            let half_off = ((addr >> 1) & 1) as usize;
            Ok((word >> (half_off * 16)) as u16)
        } else if Self::in_fw_rom(addr) {
            let lo = self.fw_rom_read(addr) as u16;
            let hi = self.fw_rom_read(addr + 1) as u16;
            Ok(lo | (hi << 8))
        } else if addr < self.mem.ram_base && !Self::in_clint(addr) {
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
            word =
                (word & !(0xFFFF << (half_off * 16))) | ((val as u32) << (half_off * 16));
            if self.clint.write(word_addr, word) {
                Ok(())
            } else {
                Err(MemoryError { addr, size: 2 })
            }
        } else if Self::in_fw_rom(addr) {
            self.fw_rom_write(addr, (val & 0xFF) as u8);
            self.fw_rom_write(addr + 1, ((val >> 8) & 0xFF) as u8);
            Ok(())
        } else if addr < self.mem.ram_base && !Self::in_clint(addr) {
            // Silently accept writes to unmapped low addresses
            Ok(())
        } else {
            self.mem.write_half(addr, val)
        }
    }

    /// Advance the CLINT timer by one tick.
    pub fn tick_clint(&mut self) {
        self.clint.tick();
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
}

#[cfg(test)]
mod tests {
    use super::super::clint;
    use super::*;

    #[test]
    fn bus_ram_read_write() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(0x8000_0000, 0xDEAD_BEEF).unwrap();
        assert_eq!(bus.read_word(0x8000_0000).unwrap(), 0xDEAD_BEEF);
    }

    #[test]
    fn bus_clint_mmio_mtimecmp() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(clint::MTIMECMP_BASE, 0x0000_0100).unwrap();
        assert_eq!(bus.read_word(clint::MTIMECMP_BASE).unwrap(), 0x0000_0100);
    }

    #[test]
    fn bus_clint_msip() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        bus.write_word(clint::MSIP_BASE, 1).unwrap();
        assert_eq!(bus.read_word(clint::MSIP_BASE).unwrap(), 1);
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
        let bus = Bus::new(0x8000_0000, 4096);
        // Low addresses now return 0 (boot ROM area) instead of error
        assert_eq!(bus.read_word(0x0000_0000).unwrap(), 0);
        assert!(bus.read_word(0x0200_1000).is_err()); // CLINT gap
    }

    #[test]
    fn bus_tick_advances_mtime() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        assert_eq!(bus.clint.mtime, 0);
        bus.tick_clint();
        assert_eq!(bus.clint.mtime, 1);
    }
}