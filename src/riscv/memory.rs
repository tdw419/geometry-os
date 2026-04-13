// riscv/memory.rs -- Guest RAM (Phase 34)
//
// Byte/half/word access into a flat Vec<u8>.
// All reads and writes return Result; out-of-range access returns Err.
// See docs/RISCV_HYPERVISOR.md §Guest Memory.

use std::fmt;

/// Error returned when a memory access falls outside RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryError {
    pub addr: u64,
    pub size: usize,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "memory access fault: addr=0x{:08X} size={}",
            self.addr, self.size
        )
    }
}

impl std::error::Error for MemoryError {}

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

    /// Convert a physical address to a RAM index, or return an error.
    fn offset(&self, addr: u64, access_size: usize) -> Result<usize, MemoryError> {
        // Addresses below ram_base wrap and will exceed ram.len() -> error.
        let start = addr.wrapping_sub(self.ram_base) as usize;
        if start + access_size <= self.ram.len() {
            Ok(start)
        } else {
            Err(MemoryError {
                addr,
                size: access_size,
            })
        }
    }

    /// Read a byte from a physical address.
    pub fn read_byte(&self, addr: u64) -> Result<u8, MemoryError> {
        let off = self.offset(addr, 1)?;
        Ok(self.ram[off])
    }

    /// Write a byte to a physical address.
    pub fn write_byte(&mut self, addr: u64, val: u8) -> Result<(), MemoryError> {
        let off = self.offset(addr, 1)?;
        self.ram[off] = val;
        Ok(())
    }

    /// Read a 16-bit half-word (little-endian) from a physical address.
    pub fn read_half(&self, addr: u64) -> Result<u16, MemoryError> {
        let off = self.offset(addr, 2)?;
        let b0 = self.ram[off] as u16;
        let b1 = self.ram[off + 1] as u16;
        Ok(b0 | (b1 << 8))
    }

    /// Write a 16-bit half-word (little-endian) to a physical address.
    pub fn write_half(&mut self, addr: u64, val: u16) -> Result<(), MemoryError> {
        let off = self.offset(addr, 2)?;
        self.ram[off] = (val & 0xFF) as u8;
        self.ram[off + 1] = ((val >> 8) & 0xFF) as u8;
        Ok(())
    }

    /// Read a 32-bit word (little-endian) from a physical address.
    pub fn read_word(&self, addr: u64) -> Result<u32, MemoryError> {
        let off = self.offset(addr, 4)?;
        let b0 = self.ram[off] as u32;
        let b1 = self.ram[off + 1] as u32;
        let b2 = self.ram[off + 2] as u32;
        let b3 = self.ram[off + 3] as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    /// Write a 32-bit word (little-endian) to a physical address.
    pub fn write_word(&mut self, addr: u64, val: u32) -> Result<(), MemoryError> {
        let off = self.offset(addr, 4)?;
        self.ram[off] = (val & 0xFF) as u8;
        self.ram[off + 1] = ((val >> 8) & 0xFF) as u8;
        self.ram[off + 2] = ((val >> 16) & 0xFF) as u8;
        self.ram[off + 3] = ((val >> 24) & 0xFF) as u8;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_word_at_base_reads_first_4_bytes_le() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        // Manually set the first 4 bytes of RAM.
        mem.ram[0] = 0x78;
        mem.ram[1] = 0x56;
        mem.ram[2] = 0x34;
        mem.ram[3] = 0x12;
        let val = mem.read_word(0x8000_0000).unwrap();
        assert_eq!(val, 0x1234_5678);
    }

    #[test]
    fn write_word_then_read_word_roundtrips() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        mem.write_word(0x8000_0000, 0xDEAD_BEEF).unwrap();
        let val = mem.read_word(0x8000_0000).unwrap();
        assert_eq!(val, 0xDEAD_BEEF);
    }

    #[test]
    fn out_of_range_read_returns_error() {
        let mem = GuestMemory::new(0x8000_0000, 4096);
        // Address below ram_base.
        assert!(mem.read_byte(0x0000_0000).is_err());
        assert!(mem.read_half(0x0000_0000).is_err());
        assert!(mem.read_word(0x0000_0000).is_err());
        // Address at end of RAM (would overflow).
        assert!(mem.read_byte(0x8000_1000).is_err());
        assert!(mem.read_word(0x8000_0FFD).is_err()); // 3 bytes left, need 4
    }

    #[test]
    fn out_of_range_write_returns_error() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        assert!(mem.write_byte(0x0000_0000, 0xFF).is_err());
        assert!(mem.write_half(0x0000_0000, 0xFFFF).is_err());
        assert!(mem.write_word(0x0000_0000, 0xDEAD_BEEF).is_err());
        assert!(mem.write_word(0x8000_0FFD, 0x1).is_err()); // overflows
    }

    #[test]
    fn read_write_byte_roundtrip() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        mem.write_byte(0x8000_0100, 0xAB).unwrap();
        assert_eq!(mem.read_byte(0x8000_0100).unwrap(), 0xAB);
    }

    #[test]
    fn read_write_half_roundtrip() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        mem.write_half(0x8000_0100, 0xCAFE).unwrap();
        assert_eq!(mem.read_half(0x8000_0100).unwrap(), 0xCAFE);
    }

    #[test]
    fn load_and_read_back() {
        let mut mem = GuestMemory::new(0x8000_0000, 4096);
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04];
        assert!(mem.load(0, data));
        assert_eq!(mem.read_word(0x8000_0000).unwrap(), 0x0403_0201);
    }

    #[test]
    fn load_overflow_returns_false() {
        let mut mem = GuestMemory::new(0x8000_0000, 4);
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05];
        assert!(!mem.load(0, data));
    }

    #[test]
    fn error_contains_address_and_size() {
        let mem = GuestMemory::new(0x8000_0000, 4096);
        let err = mem.read_word(0x0000_0000).unwrap_err();
        assert_eq!(err.addr, 0x0000_0000);
        assert_eq!(err.size, 4);
    }
}
