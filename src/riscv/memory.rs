// riscv/memory.rs -- Guest RAM (Phase 34 stub)
//
// Byte/half/word access into a flat Vec<u8>.
// See docs/RISCV_HYPERVISOR.md §Guest Memory.

/// Guest physical memory.
pub struct GuestMemory {
    /// Raw byte storage.
    ram: Vec<u8>,
    /// Physical address where RAM starts (typically 0x8000_0000).
    pub ram_base: u64,
}

impl GuestMemory {
    /// Create guest memory with the given base address and size in bytes.
    pub fn new(ram_base: u64, size: usize) -> Self {
        Self {
            ram: vec![0u8; size],
            ram_base,
        }
    }

    /// Read a byte from a physical address.
    /// Returns 0 if the address is outside RAM.
    pub fn read_byte(&self, addr: u64) -> u8 {
        let offset = addr.wrapping_sub(self.ram_base) as usize;
        if offset < self.ram.len() {
            self.ram[offset]
        } else {
            0
        }
    }

    /// Write a byte to a physical address.
    /// Silently ignores writes outside RAM.
    pub fn write_byte(&mut self, addr: u64, val: u8) {
        let offset = addr.wrapping_sub(self.ram_base) as usize;
        if offset < self.ram.len() {
            self.ram[offset] = val;
        }
    }

    /// Read a 32-bit word (little-endian) from a physical address.
    pub fn read_word(&self, addr: u64) -> u32 {
        let b0 = self.read_byte(addr) as u32;
        let b1 = self.read_byte(addr + 1) as u32;
        let b2 = self.read_byte(addr + 2) as u32;
        let b3 = self.read_byte(addr + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Write a 32-bit word (little-endian) to a physical address.
    pub fn write_word(&mut self, addr: u64, val: u32) {
        self.write_byte(addr, (val & 0xFF) as u8);
        self.write_byte(addr + 1, ((val >> 8) & 0xFF) as u8);
        self.write_byte(addr + 2, ((val >> 16) & 0xFF) as u8);
        self.write_byte(addr + 3, ((val >> 24) & 0xFF) as u8);
    }

    /// Load a binary blob into RAM at the given offset from ram_base.
    /// Returns false if the blob doesn't fit.
    pub fn load(&mut self, offset: usize, data: &[u8]) -> bool {
        if offset + data.len() > self.ram.len() {
            return false;
        }
        self.ram[offset..offset + data.len()].copy_from_slice(data);
        true
    }

    /// RAM size in bytes.
    pub fn size(&self) -> usize {
        self.ram.len()
    }
}
