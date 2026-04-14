// riscv/loader.rs -- ELF and raw binary image loader (Phase 37)
//
// Parses ELF32 (RV32) and raw flat binary images, loads them into
// guest RAM, and returns the entry point for the CPU to start executing.
//
// ELF format reference: https://refspecs.linuxbase.org/elf/elf.pdf
// RISC-V ELF: EM_RISCV = 243, EF_RISCV_RV32I = 0x0001
//
// The loader is conservative: it verifies magic, class (32-bit),
// endianness (little), machine type, and only loads PT_LOAD segments.

use super::bus::Bus;

/// ELF magic: 0x7F 'E' 'L' 'F'
const ELF_MAGIC: u32 = 0x464C457F;

/// ELF machine type for RISC-V.
const EM_RISCV: u16 = 243;

/// ELF program header type: PT_LOAD (loadable segment).
const PT_LOAD: u32 = 1;

/// Error type for loader operations.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Image is too short to contain an ELF header.
    TooShort,
    /// ELF magic mismatch -- not an ELF file.
    NotElf,
    /// Wrong ELF class (expected 32-bit, got 64-bit).
    WrongClass,
    /// Wrong endianness (expected little-endian).
    WrongEndian,
    /// Wrong machine type (expected RISC-V).
    WrongMachine,
    /// No loadable segments found.
    NoLoadSegments,
    /// Segment doesn't fit in guest RAM.
    SegmentOverflow,
    /// Entry point outside loaded regions.
    BadEntryPoint,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::TooShort => write!(f, "image too short for ELF header"),
            LoadError::NotElf => write!(f, "not an ELF file (bad magic)"),
            LoadError::WrongClass => write!(f, "expected ELF32, got ELF64"),
            LoadError::WrongEndian => write!(f, "expected little-endian ELF"),
            LoadError::WrongMachine => write!(f, "expected RISC-V ELF (EM_RISCV)"),
            LoadError::NoLoadSegments => write!(f, "no PT_LOAD segments in ELF"),
            LoadError::SegmentOverflow => write!(f, "segment doesn't fit in guest RAM"),
            LoadError::BadEntryPoint => write!(f, "entry point outside loaded segments"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Result of a successful image load.
#[derive(Debug, PartialEq, Eq)]
pub struct LoadInfo {
    /// Entry point address (where PC should start).
    pub entry: u32,
    /// Highest address loaded (end of last segment).
    pub highest_addr: u64,
}

/// Load an ELF32 image into guest RAM via the bus.
///
/// Parses the ELF header and program headers, then copies all PT_LOAD
/// segments into guest memory at their specified physical addresses.
///
/// Returns the entry point address on success.
pub fn load_elf(bus: &mut Bus, image: &[u8]) -> Result<LoadInfo, LoadError> {
    // Need at least 52 bytes for ELF32 header.
    if image.len() < 52 {
        return Err(LoadError::TooShort);
    }

    // Check magic.
    let magic = u32::from_le_bytes([image[0], image[1], image[2], image[3]]);
    if magic != ELF_MAGIC {
        return Err(LoadError::NotElf);
    }

    // Check class: 1 = 32-bit, 2 = 64-bit.
    let class = image[4];
    if class != 1 {
        return Err(LoadError::WrongClass);
    }

    // Check endianness: 1 = little-endian.
    let endian = image[5];
    if endian != 1 {
        return Err(LoadError::WrongEndian);
    }

    // Machine type at offset 18 (2 bytes, little-endian).
    let machine = u16::from_le_bytes([image[18], image[19]]);
    if machine != EM_RISCV {
        return Err(LoadError::WrongMachine);
    }

    // Entry point at offset 24 (4 bytes).
    let entry = u32::from_le_bytes([image[24], image[25], image[26], image[27]]);

    // Program header offset at offset 28 (4 bytes).
    let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;

    // Program header entry size at offset 42 (2 bytes).
    let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;

    // Number of program headers at offset 44 (2 bytes).
    let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;

    let mut highest_addr: u64 = 0;
    let mut loaded_any = false;

    // Parse and load each PT_LOAD segment.
    for i in 0..phnum {
        let off = phoff + i * phentsize;
        if off + phentsize > image.len() {
            break;
        }

        let seg = &image[off..off + phentsize];

        // Program header fields (ELF32):
        //   0: p_type (4 bytes)
        //   4: p_offset (4 bytes) -- offset in file
        //   8: p_vaddr (4 bytes) -- virtual address
        //  12: p_paddr (4 bytes) -- physical address
        //  16: p_filesz (4 bytes) -- size in file
        //  20: p_memsz (4 bytes) -- size in memory
        let p_type = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
        let p_offset = u32::from_le_bytes([seg[4], seg[5], seg[6], seg[7]]) as usize;
        let p_paddr = u32::from_le_bytes([seg[12], seg[13], seg[14], seg[15]]) as u64;
        let p_filesz = u32::from_le_bytes([seg[16], seg[17], seg[18], seg[19]]) as usize;
        let _p_memsz = u32::from_le_bytes([seg[20], seg[21], seg[22], seg[23]]) as usize;

        if p_type != PT_LOAD {
            continue;
        }

        // Clamp file data to actual image size.
        let file_end = p_offset.saturating_add(p_filesz).min(image.len());
        let data = if p_offset < image.len() {
            &image[p_offset..file_end]
        } else {
            &[]
        };

        // Load into guest RAM at physical address.
        // The bus routes by address: p_paddr goes to RAM (offset from ram_base).
        for (j, &byte) in data.iter().enumerate() {
            let addr = p_paddr + j as u64;
            // Best-effort: write bytes. If out of range, stop.
            if bus.write_byte(addr, byte).is_err() {
                return Err(LoadError::SegmentOverflow);
            }
        }

        let seg_end = p_paddr + p_filesz as u64;
        if seg_end > highest_addr {
            highest_addr = seg_end;
        }
        loaded_any = true;
    }

    if !loaded_any {
        return Err(LoadError::NoLoadSegments);
    }

    Ok(LoadInfo {
        entry,
        highest_addr,
    })
}

/// Load a raw flat binary image into guest RAM at the specified base address.
///
/// Used for images that aren't ELF (e.g., OpenSBI firmware payloads,
/// flat binary kernels). Sets PC to `base_addr` after loading.
pub fn load_raw(bus: &mut Bus, image: &[u8], base_addr: u64) -> Result<LoadInfo, LoadError> {
    for (i, &byte) in image.iter().enumerate() {
        let addr = base_addr + i as u64;
        if bus.write_byte(addr, byte).is_err() {
            return Err(LoadError::SegmentOverflow);
        }
    }

    Ok(LoadInfo {
        entry: base_addr as u32,
        highest_addr: base_addr + image.len() as u64,
    })
}

/// Load an ELF32 image into guest RAM at its virtual addresses.
///
/// Like `load_elf`, but uses p_vaddr instead of p_paddr for loading.
/// This is used for Linux kernel boot where the kernel is linked with
/// PAGE_OFFSET (e.g., 0xC0000000) and we place RAM at that base address
/// so that virtual == physical while MMU is off.
///
/// Returns the entry point and highest loaded address.
pub fn load_elf_vaddr(bus: &mut Bus, image: &[u8]) -> Result<LoadInfo, LoadError> {
    if image.len() < 52 {
        return Err(LoadError::TooShort);
    }

    let magic = u32::from_le_bytes([image[0], image[1], image[2], image[3]]);
    if magic != ELF_MAGIC {
        return Err(LoadError::NotElf);
    }

    let class = image[4];
    if class != 1 {
        return Err(LoadError::WrongClass);
    }

    let endian = image[5];
    if endian != 1 {
        return Err(LoadError::WrongEndian);
    }

    let machine = u16::from_le_bytes([image[18], image[19]]);
    if machine != EM_RISCV {
        return Err(LoadError::WrongMachine);
    }

    let entry = u32::from_le_bytes([image[24], image[25], image[26], image[27]]);
    let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;
    let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;
    let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;

    let mut highest_addr: u64 = 0;
    let mut loaded_any = false;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        if off + phentsize > image.len() {
            break;
        }

        let seg = &image[off..off + phentsize];
        let p_type = u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]);
        let p_offset = u32::from_le_bytes([seg[4], seg[5], seg[6], seg[7]]) as usize;
        let p_vaddr = u32::from_le_bytes([seg[8], seg[9], seg[10], seg[11]]) as u64;
        let _p_paddr = u32::from_le_bytes([seg[12], seg[13], seg[14], seg[15]]) as u64;
        let p_filesz = u32::from_le_bytes([seg[16], seg[17], seg[18], seg[19]]) as usize;
        let p_memsz = u32::from_le_bytes([seg[20], seg[21], seg[22], seg[23]]) as usize;

        if p_type != PT_LOAD {
            continue;
        }

        // Load into guest RAM at virtual address.
        let file_end = p_offset.saturating_add(p_filesz).min(image.len());
        let data = if p_offset < image.len() {
            &image[p_offset..file_end]
        } else {
            &[]
        };

        for (j, &byte) in data.iter().enumerate() {
            let addr = p_vaddr + j as u64;
            if bus.write_byte(addr, byte).is_err() {
                return Err(LoadError::SegmentOverflow);
            }
        }

        // Track highest address including BSS (memsz > filesz).
        let seg_end = p_vaddr + p_memsz as u64;
        if seg_end > highest_addr {
            highest_addr = seg_end;
        }
        loaded_any = true;
    }

    if !loaded_any {
        return Err(LoadError::NoLoadSegments);
    }

    Ok(LoadInfo {
        entry,
        highest_addr,
    })
}

/// Detect image format and load accordingly.
///
/// If the image starts with the ELF magic, loads as ELF32.
/// Otherwise, loads as a raw binary at the specified default base.
pub fn load_auto(bus: &mut Bus, image: &[u8], default_base: u64) -> Result<LoadInfo, LoadError> {
    if image.len() >= 4 {
        let magic = u32::from_le_bytes([image[0], image[1], image[2], image[3]]);
        if magic == ELF_MAGIC {
            return load_elf(bus, image);
        }
    }
    load_raw(bus, image, default_base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elf_header(
        entry: u32,
        machine: u16,
        class: u8,
        endian: u8,
    ) -> Vec<u8> {
        let mut hdr = vec![0u8; 52];
        // Magic.
        hdr[0..4].copy_from_slice(&ELF_MAGIC.to_le_bytes());
        // Class.
        hdr[4] = class;
        // Endianness.
        hdr[5] = endian;
        // Version.
        hdr[6] = 1;
        // Machine type.
        hdr[18..20].copy_from_slice(&machine.to_le_bytes());
        // Entry point.
        hdr[24..28].copy_from_slice(&entry.to_le_bytes());
        // phoff = 52 (right after header).
        hdr[28..32].copy_from_slice(&52u32.to_le_bytes());
        // phentsize = 32.
        hdr[42..44].copy_from_slice(&32u16.to_le_bytes());
        // phnum = 0 (no segments).
        hdr[44..46].copy_from_slice(&0u16.to_le_bytes());
        hdr
    }

    fn make_elf_with_segment(
        entry: u32,
        paddr: u32,
        data: &[u8],
    ) -> Vec<u8> {
        let mut img = make_elf_header(entry, EM_RISCV, 1, 1);
        // Update phnum = 1.
        img[44..46].copy_from_slice(&1u16.to_le_bytes());

        // Program header (32 bytes).
        let mut phdr = [0u8; 32];
        // p_type = PT_LOAD.
        phdr[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        // p_offset = 52 + 32 = 84.
        phdr[4..8].copy_from_slice(&84u32.to_le_bytes());
        // p_vaddr = paddr.
        phdr[8..12].copy_from_slice(&paddr.to_le_bytes());
        // p_paddr = paddr.
        phdr[12..16].copy_from_slice(&paddr.to_le_bytes());
        // p_filesz.
        phdr[16..20].copy_from_slice(&(data.len() as u32).to_le_bytes());
        // p_memsz.
        phdr[20..24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        // p_flags = RX.
        phdr[24..28].copy_from_slice(&5u32.to_le_bytes());
        // p_align = 4096.
        phdr[28..32].copy_from_slice(&4096u32.to_le_bytes());

        img.extend_from_slice(&phdr);
        img.extend_from_slice(data);
        img
    }

    #[test]
    fn elf_rejects_too_short() {
        let bus = Bus::new(0x8000_0000, 4096);
        // Need &mut for load_elf, but we need to test error first.
        let mut bus = bus;
        let result = load_elf(&mut bus, &[0x7F, 0x45, 0x4C]);
        assert_eq!(result, Err(LoadError::TooShort));
    }

    #[test]
    fn elf_rejects_bad_magic() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut img = make_elf_header(0x8000_0000, EM_RISCV, 1, 1);
        img[0] = 0x00; // corrupt magic
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::NotElf));
    }

    #[test]
    fn elf_rejects_64bit() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let img = make_elf_header(0x8000_0000, EM_RISCV, 2, 1); // class=2 (64-bit)
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::WrongClass));
    }

    #[test]
    fn elf_rejects_big_endian() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let img = make_elf_header(0x8000_0000, EM_RISCV, 1, 2); // endian=2 (big)
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::WrongEndian));
    }

    #[test]
    fn elf_rejects_wrong_machine() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let img = make_elf_header(0x8000_0000, 0x003E, 1, 1); // EM_X86_64
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::WrongMachine));
    }

    #[test]
    fn elf_rejects_no_load_segments() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let img = make_elf_header(0x8000_0000, EM_RISCV, 1, 1);
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::NoLoadSegments));
    }

    #[test]
    fn elf_loads_segment_into_ram() {
        let mut bus = Bus::new(0x8000_0000, 8192);
        let data: &[u8] = &[0x13, 0x00, 0x00, 0x00]; // NOP instruction
        let img = make_elf_with_segment(0x8000_0000, 0x8000_0000, data);
        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
        // Verify the data was loaded.
        assert_eq!(bus.read_byte(0x8000_0000).unwrap(), 0x13);
        assert_eq!(bus.read_byte(0x8000_0003).unwrap(), 0x00);
    }

    #[test]
    fn elf_loads_multiple_segments() {
        let mut bus = Bus::new(0x8000_0000, 16384);
        let mut img = make_elf_header(0x8000_0000, EM_RISCV, 1, 1);
        // phnum = 2.
        img[44..46].copy_from_slice(&2u16.to_le_bytes());

        // Segment 1: code at 0x8000_0000.
        let code: &[u8] = &[0x13, 0x00, 0x00, 0x00];
        // Segment 2: data at 0x8000_1000.
        let data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];

        // ELF layout: header(52) + phdr1(32) + phdr2(32) = 116 bytes before data.
        // Code data starts at offset 116, data segment at offset 120.
        let code_offset = 52 + 32 + 32; // = 116
        let data_offset = code_offset + code.len(); // = 120

        let mut phdr1 = [0u8; 32];
        phdr1[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        phdr1[4..8].copy_from_slice(&(code_offset as u32).to_le_bytes());
        phdr1[8..12].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        phdr1[12..16].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        phdr1[16..20].copy_from_slice(&4u32.to_le_bytes());
        phdr1[20..24].copy_from_slice(&4u32.to_le_bytes());

        let mut phdr2 = [0u8; 32];
        phdr2[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        phdr2[4..8].copy_from_slice(&(data_offset as u32).to_le_bytes());
        phdr2[8..12].copy_from_slice(&0x8000_1000u32.to_le_bytes());
        phdr2[12..16].copy_from_slice(&0x8000_1000u32.to_le_bytes());
        phdr2[16..20].copy_from_slice(&4u32.to_le_bytes());
        phdr2[20..24].copy_from_slice(&4u32.to_le_bytes());

        img.extend_from_slice(&phdr1);
        img.extend_from_slice(&phdr2);
        img.extend_from_slice(code);
        img.extend_from_slice(data);

        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
        assert_eq!(bus.read_word(0x8000_0000).unwrap(), 0x0000_0013);
        assert_eq!(bus.read_word(0x8000_1000).unwrap(), 0xEFBE_ADDE);
    }

    #[test]
    fn elf_segment_overflow_returns_error() {
        let mut bus = Bus::new(0x8000_0000, 16); // tiny RAM
        let data = vec![0xFFu8; 32]; // won't fit
        let img = make_elf_with_segment(0x8000_0000, 0x8000_0000, &data);
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::SegmentOverflow));
    }

    #[test]
    fn raw_load_at_base() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let data: &[u8] = &[0x13, 0x01, 0xA0, 0x23]; // some instruction
        let info = load_raw(&mut bus, data, 0x8000_0000).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
        assert_eq!(bus.read_byte(0x8000_0000).unwrap(), 0x13);
    }

    #[test]
    fn raw_load_overflow() {
        let mut bus = Bus::new(0x8000_0000, 4);
        let data = vec![0u8; 8];
        let result = load_raw(&mut bus, &data, 0x8000_0000);
        assert_eq!(result, Err(LoadError::SegmentOverflow));
    }

    #[test]
    fn auto_detect_elf() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let data: &[u8] = &[0x13, 0x00, 0x00, 0x00];
        let img = make_elf_with_segment(0x8000_0000, 0x8000_0000, data);
        let info = load_auto(&mut bus, &img, 0x8000_0000).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
    }

    #[test]
    fn auto_detect_raw() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let data: &[u8] = &[0x13, 0x00, 0x00, 0x00];
        let info = load_auto(&mut bus, data, 0x8000_0000).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
        assert_eq!(bus.read_byte(0x8000_0000).unwrap(), 0x13);
    }

    #[test]
    fn elf_skips_non_load_segments() {
        let mut bus = Bus::new(0x8000_0000, 8192);
        let mut img = make_elf_header(0x8000_0000, EM_RISCV, 1, 1);
        // phnum = 2: one PT_NULL, one PT_LOAD.
        img[44..46].copy_from_slice(&2u16.to_le_bytes());

        // Segment 1: PT_NULL (type 0) -- should be skipped.
        let mut phdr_null = [0u8; 32];
        phdr_null[0..4].copy_from_slice(&0u32.to_le_bytes()); // PT_NULL

        // Segment 2: PT_LOAD with actual data.
        // ELF layout: header(52) + phdr_null(32) + phdr_load(32) = 116 bytes.
        let data: &[u8] = &[0xAB, 0xCD, 0xEF, 0x01];
        let data_offset = 52 + 32 + 32; // = 116
        let mut phdr_load = [0u8; 32];
        phdr_load[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        phdr_load[4..8].copy_from_slice(&(data_offset as u32).to_le_bytes());
        phdr_load[8..12].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        phdr_load[12..16].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        phdr_load[16..20].copy_from_slice(&4u32.to_le_bytes());
        phdr_load[20..24].copy_from_slice(&4u32.to_le_bytes());

        img.extend_from_slice(&phdr_null);
        img.extend_from_slice(&phdr_load);
        img.extend_from_slice(data);

        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x8000_0000);
        assert_eq!(bus.read_byte(0x8000_0000).unwrap(), 0xAB);
    }

    #[test]
    fn elf_highest_addr_tracking() {
        let mut bus = Bus::new(0x8000_0000, 8192);
        let data: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let img = make_elf_with_segment(0x8000_0000, 0x8000_0000, data);
        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.highest_addr, 0x8000_0008);
    }
}