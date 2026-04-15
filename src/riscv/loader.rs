// riscv/loader.rs -- ELF and raw binary image loader (Phase 37)
//
// Parses ELF32 and ELF64 (RV32) and raw flat binary images, loads them into
// guest RAM, and returns the entry point for the CPU to start executing.
//
// ELF format reference: https://refspecs.linuxbase.org/elf/elf.pdf
// RISC-V ELF: EM_RISCV = 243, EF_RISCV_RV32I = 0x0001
//
// The loader supports both ELF32 and ELF64 formats. For ELF64, addresses
// are truncated to 32 bits (appropriate for RV32 VMs). This is needed to
// boot Linux kernels that are built as ELF64 even for RV32 targets.

use super::bus::Bus;

/// ELF magic: 0x7F 'E' 'L' 'F'
const ELF_MAGIC: u32 = 0x464C457F;

/// ELF machine type for RISC-V.
const EM_RISCV: u16 = 243;

/// ELF program header type: PT_LOAD (loadable segment).
const PT_LOAD: u32 = 1;

/// ELF class: 32-bit.
const ELFCLASS32: u8 = 1;

/// ELF class: 64-bit.
const ELFCLASS64: u8 = 2;

/// Error type for loader operations.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Image is too short to contain an ELF header.
    TooShort,
    /// ELF magic mismatch -- not an ELF file.
    NotElf,
    /// Unsupported ELF class (not 32-bit or 64-bit).
    UnsupportedClass,
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
            LoadError::UnsupportedClass => write!(f, "unsupported ELF class (not 32-bit or 64-bit)"),
            LoadError::WrongEndian => write!(f, "expected little-endian ELF"),
            LoadError::WrongMachine => write!(f, "expected RISC-V ELF (EM_RISCV)"),
            LoadError::NoLoadSegments => write!(f, "no PT_LOAD segments in ELF"),
            LoadError::SegmentOverflow => write!(f, "segment doesn't fit in guest RAM"),
            LoadError::BadEntryPoint => write!(f, "entry point outside loaded segments"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Parsed ELF header fields relevant to loading.
pub(crate) struct ElfHeader {
    /// Entry point (truncated to 32 bits for ELF64).
    pub(crate) entry: u32,
    /// Offset to program header table.
    pub(crate) phoff: usize,
    /// Size of each program header entry.
    pub(crate) phentsize: usize,
    /// Number of program header entries.
    pub(crate) phnum: usize,
}

/// Parsed ELF program header fields.
pub(crate) struct ElfPhdr {
    /// Segment type (e.g., PT_LOAD).
    pub(crate) p_type: u32,
    /// Offset in file where segment data starts.
    pub(crate) p_offset: usize,
    /// Virtual address (truncated to 32 bits for ELF64).
    pub(crate) p_vaddr: u32,
    /// Physical address (truncated to 32 bits for ELF64).
    pub(crate) p_paddr: u32,
    /// Size of segment in file.
    pub(crate) p_filesz: usize,
    /// Size of segment in memory (includes BSS).
    pub(crate) p_memsz: usize,
}

/// Validate common ELF header fields (magic, class, endianness, machine).
/// Returns the ELF class (1=32-bit, 2=64-bit).
pub(crate) fn validate_elf_header(image: &[u8]) -> Result<u8, LoadError> {
    // Need at least the ELF identification bytes (first 16) to read class.
    // But for class-specific checks, we need the full header.
    // First check if we can even read the class byte.
    if image.len() < 6 {
        return Err(LoadError::TooShort);
    }

    // Check magic.
    let magic = u32::from_le_bytes([image[0], image[1], image[2], image[3]]);
    if magic != ELF_MAGIC {
        return Err(LoadError::NotElf);
    }

    let class = image[4];
    if class != ELFCLASS32 && class != ELFCLASS64 {
        return Err(LoadError::UnsupportedClass);
    }

    let endian = image[5];
    if endian != 1 {
        return Err(LoadError::WrongEndian);
    }

    // Now check full header size based on class.
    let min_size = match class {
        ELFCLASS32 => 52,
        ELFCLASS64 => 64,
        _ => unreachable!(),
    };
    if image.len() < min_size {
        return Err(LoadError::TooShort);
    }

    let machine = u16::from_le_bytes([image[18], image[19]]);
    if machine != EM_RISCV {
        return Err(LoadError::WrongMachine);
    }

    Ok(class)
}

/// Parse the ELF header fields needed for loading.
pub(crate) fn parse_elf_header(image: &[u8], class: u8) -> ElfHeader {
    match class {
        ELFCLASS32 => {
            let entry = u32::from_le_bytes([image[24], image[25], image[26], image[27]]);
            let phoff = u32::from_le_bytes([image[28], image[29], image[30], image[31]]) as usize;
            let phentsize = u16::from_le_bytes([image[42], image[43]]) as usize;
            let phnum = u16::from_le_bytes([image[44], image[45]]) as usize;
            ElfHeader { entry, phoff, phentsize, phnum }
        }
        ELFCLASS64 => {
            // ELF64 entry is 8 bytes at offset 24; truncate to 32 bits.
            let entry = u64::from_le_bytes([
                image[24], image[25], image[26], image[27],
                image[28], image[29], image[30], image[31],
            ]) as u32;
            // phoff is 8 bytes at offset 32.
            let phoff = u64::from_le_bytes([
                image[32], image[33], image[34], image[35],
                image[36], image[37], image[38], image[39],
            ]) as usize;
            // phentsize is 2 bytes at offset 54.
            let phentsize = u16::from_le_bytes([image[54], image[55]]) as usize;
            // phnum is 2 bytes at offset 56.
            let phnum = u16::from_le_bytes([image[56], image[57]]) as usize;
            ElfHeader { entry, phoff, phentsize, phnum }
        }
        _ => unreachable!("validated in validate_elf_header"),
    }
}

/// Parse a single program header entry.
pub(crate) fn parse_phdr(image: &[u8], offset: usize, class: u8) -> Option<ElfPhdr> {
    if offset + match class {
        ELFCLASS32 => 32,
        ELFCLASS64 => 56,
        _ => return None,
    } > image.len() {
        return None;
    }

    match class {
        ELFCLASS32 => {
            let seg = &image[offset..offset + 32];
            Some(ElfPhdr {
                p_type: u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]),
                p_offset: u32::from_le_bytes([seg[4], seg[5], seg[6], seg[7]]) as usize,
                p_vaddr: u32::from_le_bytes([seg[8], seg[9], seg[10], seg[11]]),
                p_paddr: u32::from_le_bytes([seg[12], seg[13], seg[14], seg[15]]),
                p_filesz: u32::from_le_bytes([seg[16], seg[17], seg[18], seg[19]]) as usize,
                p_memsz: u32::from_le_bytes([seg[20], seg[21], seg[22], seg[23]]) as usize,
            })
        }
        ELFCLASS64 => {
            let seg = &image[offset..offset + 56];
            Some(ElfPhdr {
                p_type: u32::from_le_bytes([seg[0], seg[1], seg[2], seg[3]]),
                // p_offset is 8 bytes at offset 8.
                p_offset: u64::from_le_bytes([
                    seg[8], seg[9], seg[10], seg[11],
                    seg[12], seg[13], seg[14], seg[15],
                ]) as usize,
                // p_vaddr is 8 bytes at offset 16; truncate to 32 bits.
                p_vaddr: u64::from_le_bytes([
                    seg[16], seg[17], seg[18], seg[19],
                    seg[20], seg[21], seg[22], seg[23],
                ]) as u32,
                // p_paddr is 8 bytes at offset 24; truncate to 32 bits.
                p_paddr: u64::from_le_bytes([
                    seg[24], seg[25], seg[26], seg[27],
                    seg[28], seg[29], seg[30], seg[31],
                ]) as u32,
                // p_filesz is 8 bytes at offset 32.
                p_filesz: u64::from_le_bytes([
                    seg[32], seg[33], seg[34], seg[35],
                    seg[36], seg[37], seg[38], seg[39],
                ]) as usize,
                // p_memsz is 8 bytes at offset 40.
                p_memsz: u64::from_le_bytes([
                    seg[40], seg[41], seg[42], seg[43],
                    seg[44], seg[45], seg[46], seg[47],
                ]) as usize,
            })
        }
        _ => None,
    }
}

/// Result of a successful image load.
#[derive(Debug, PartialEq, Eq)]
pub struct LoadInfo {
    /// Entry point address (where PC should start).
    pub entry: u32,
    /// Highest address loaded (end of last segment).
    pub highest_addr: u64,
}

/// Load an ELF image (32-bit or 64-bit) into guest RAM via the bus.
///
/// Parses the ELF header and program headers, then copies all PT_LOAD
/// segments into guest memory at their specified physical addresses.
/// For ELF64 images, addresses are truncated to 32 bits (appropriate for
/// RV32 VMs, e.g., booting a 64-bit ELF Linux kernel on a 32-bit CPU).
///
/// Returns the entry point address on success.
pub fn load_elf(bus: &mut Bus, image: &[u8]) -> Result<LoadInfo, LoadError> {
    let class = validate_elf_header(image)?;
    let hdr = parse_elf_header(image, class);

    let mut highest_addr: u64 = 0;
    let mut loaded_any = false;

    // Parse and load each PT_LOAD segment.
    for i in 0..hdr.phnum {
        let off = hdr.phoff + i * hdr.phentsize;
        let phdr = match parse_phdr(image, off, class) {
            Some(p) => p,
            None => break,
        };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        // Clamp file data to actual image size.
        let file_end = phdr.p_offset.saturating_add(phdr.p_filesz).min(image.len());
        let data = if phdr.p_offset < image.len() {
            &image[phdr.p_offset..file_end]
        } else {
            &[]
        };

        // Load file data into guest RAM at physical address.
        for (j, &byte) in data.iter().enumerate() {
            let addr = phdr.p_paddr as u64 + j as u64;
            if bus.write_byte(addr, byte).is_err() {
                return Err(LoadError::SegmentOverflow);
            }
        }

        // Zero BSS: p_memsz > p_filesz means uninitialized data (BSS).
        // The gap between file data and memory size must be zeroed.
        if phdr.p_memsz > phdr.p_filesz {
            for j in phdr.p_filesz..phdr.p_memsz {
                let addr = phdr.p_paddr as u64 + j as u64;
                if bus.write_byte(addr, 0).is_err() {
                    break; // best-effort: stop at RAM boundary
                }
            }
        }

        let seg_end = phdr.p_paddr as u64 + phdr.p_memsz.max(phdr.p_filesz) as u64;
        if seg_end > highest_addr {
            highest_addr = seg_end;
        }
        loaded_any = true;
    }

    if !loaded_any {
        return Err(LoadError::NoLoadSegments);
    }

    Ok(LoadInfo {
        entry: hdr.entry,
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

/// Load an ELF image (32-bit or 64-bit) into guest RAM at its virtual addresses.
///
/// Like `load_elf`, but uses p_vaddr instead of p_paddr for loading.
/// This is used for Linux kernel boot where the kernel is linked with
/// PAGE_OFFSET (e.g., 0xC0000000) and we place RAM at that base address
/// so that virtual == physical while MMU is off.
///
/// Returns the entry point and highest loaded address.
pub fn load_elf_vaddr(bus: &mut Bus, image: &[u8]) -> Result<LoadInfo, LoadError> {
    let class = validate_elf_header(image)?;
    let hdr = parse_elf_header(image, class);

    let mut highest_addr: u64 = 0;
    let mut loaded_any = false;

    for i in 0..hdr.phnum {
        let off = hdr.phoff + i * hdr.phentsize;
        let phdr = match parse_phdr(image, off, class) {
            Some(p) => p,
            None => break,
        };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        // Load into guest RAM at virtual address.
        let file_end = phdr.p_offset.saturating_add(phdr.p_filesz).min(image.len());
        let data = if phdr.p_offset < image.len() {
            &image[phdr.p_offset..file_end]
        } else {
            &[]
        };

        for (j, &byte) in data.iter().enumerate() {
            let addr = phdr.p_vaddr as u64 + j as u64;
            if bus.write_byte(addr, byte).is_err() {
                return Err(LoadError::SegmentOverflow);
            }
        }

        // Track highest address including BSS (memsz > filesz).
        let seg_end = phdr.p_vaddr as u64 + phdr.p_memsz as u64;
        if seg_end > highest_addr {
            highest_addr = seg_end;
        }
        loaded_any = true;
    }

    if !loaded_any {
        return Err(LoadError::NoLoadSegments);
    }

    Ok(LoadInfo {
        entry: hdr.entry,
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
    fn elf_rejects_unsupported_class() {
        let mut bus = Bus::new(0x8000_0000, 4096);
        let mut img = vec![0u8; 52];
        img[0..4].copy_from_slice(&ELF_MAGIC.to_le_bytes());
        img[4] = 3; // class=3 (invalid, not 32 or 64)
        img[5] = 1;
        img[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        let result = load_elf(&mut bus, &img);
        assert_eq!(result, Err(LoadError::UnsupportedClass));
    }

    #[test]
    fn elf_accepts_64bit() {
        // ELF64 header with a PT_LOAD segment should load successfully.
        let mut bus = Bus::new(0, 8192);
        let img = make_elf64_with_segment(0x1000, 0x1000, &[0x13, 0x00, 0x00, 0x00]);
        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x1000);
        assert_eq!(bus.read_byte(0x1000).unwrap(), 0x13);
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

    /// Build a minimal ELF64 image with one PT_LOAD segment.
    fn make_elf64_with_segment(entry: u32, paddr: u32, data: &[u8]) -> Vec<u8> {
        // ELF64 header is 64 bytes.
        let mut hdr = vec![0u8; 64];
        // Magic.
        hdr[0..4].copy_from_slice(&ELF_MAGIC.to_le_bytes());
        // Class = 64-bit.
        hdr[4] = ELFCLASS64;
        // Endianness = little.
        hdr[5] = 1;
        // Version = 1.
        hdr[6] = 1;
        // Machine = RISC-V.
        hdr[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        // Entry point (8 bytes, low 32 bits matter).
        hdr[24..32].copy_from_slice(&(entry as u64).to_le_bytes());
        // phoff = 64 (right after 64-byte header).
        hdr[32..40].copy_from_slice(&64u64.to_le_bytes());
        // phentsize = 56 (ELF64 program header size).
        hdr[54..56].copy_from_slice(&56u16.to_le_bytes());
        // phnum = 1.
        hdr[56..58].copy_from_slice(&1u16.to_le_bytes());

        // ELF64 program header (56 bytes).
        let mut phdr = [0u8; 56];
        // p_type = PT_LOAD.
        phdr[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        // p_flags = RX.
        phdr[4..8].copy_from_slice(&5u32.to_le_bytes());
        // p_offset = 64 + 56 = 120 (8 bytes).
        phdr[8..16].copy_from_slice(&120u64.to_le_bytes());
        // p_vaddr (8 bytes).
        phdr[16..24].copy_from_slice(&(paddr as u64).to_le_bytes());
        // p_paddr (8 bytes).
        phdr[24..32].copy_from_slice(&(paddr as u64).to_le_bytes());
        // p_filesz (8 bytes).
        phdr[32..40].copy_from_slice(&(data.len() as u64).to_le_bytes());
        // p_memsz (8 bytes).
        phdr[40..48].copy_from_slice(&(data.len() as u64).to_le_bytes());
        // p_align = 4096.
        phdr[48..56].copy_from_slice(&4096u64.to_le_bytes());

        let mut img = hdr;
        img.extend_from_slice(&phdr);
        img.extend_from_slice(data);
        img
    }

    #[test]
    fn elf64_loads_segment_into_ram() {
        let mut bus = Bus::new(0, 8192);
        let data: &[u8] = &[0x13, 0x00, 0x00, 0x00]; // NOP
        let img = make_elf64_with_segment(0x1000, 0x1000, data);
        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x1000);
        assert_eq!(bus.read_byte(0x1000).unwrap(), 0x13);
        assert_eq!(bus.read_byte(0x1003).unwrap(), 0x00);
        assert_eq!(info.highest_addr, 0x1004);
    }

    #[test]
    fn elf64_entry_truncated_to_32_bits() {
        let mut bus = Bus::new(0, 8192);
        let data: &[u8] = &[0xFF];
        // Entry = 0x1_8000_0000 (above 32-bit range), should truncate to 0x80000000.
        let mut img = make_elf64_with_segment(0x8000_0000, 0x1000, data);
        // Overwrite entry with 64-bit value.
        img[24..32].copy_from_slice(&0x1_8000_0000u64.to_le_bytes());
        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x8000_0000); // truncated
    }

    #[test]
    fn elf64_multiple_segments() {
        let mut bus = Bus::new(0, 16384);
        // Manually build ELF64 with 2 segments.
        let mut hdr = vec![0u8; 64];
        hdr[0..4].copy_from_slice(&ELF_MAGIC.to_le_bytes());
        hdr[4] = ELFCLASS64;
        hdr[5] = 1;
        hdr[6] = 1;
        hdr[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        hdr[24..32].copy_from_slice(&0x1000u64.to_le_bytes());
        hdr[32..40].copy_from_slice(&64u64.to_le_bytes());
        hdr[54..56].copy_from_slice(&56u16.to_le_bytes());
        hdr[56..58].copy_from_slice(&2u16.to_le_bytes()); // phnum = 2

        let code: &[u8] = &[0x13, 0x00, 0x00, 0x00];
        let data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];

        let seg1_offset = 64 + 56 + 56; // = 176
        let seg2_offset = seg1_offset + code.len(); // = 180

        // Segment 1: code at 0x1000.
        let mut phdr1 = [0u8; 56];
        phdr1[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        phdr1[8..16].copy_from_slice(&(seg1_offset as u64).to_le_bytes());
        phdr1[16..24].copy_from_slice(&0x1000u64.to_le_bytes());
        phdr1[24..32].copy_from_slice(&0x1000u64.to_le_bytes());
        phdr1[32..40].copy_from_slice(&(code.len() as u64).to_le_bytes());
        phdr1[40..48].copy_from_slice(&(code.len() as u64).to_le_bytes());

        // Segment 2: data at 0x2000.
        let mut phdr2 = [0u8; 56];
        phdr2[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        phdr2[8..16].copy_from_slice(&(seg2_offset as u64).to_le_bytes());
        phdr2[16..24].copy_from_slice(&0x2000u64.to_le_bytes());
        phdr2[24..32].copy_from_slice(&0x2000u64.to_le_bytes());
        phdr2[32..40].copy_from_slice(&(data.len() as u64).to_le_bytes());
        phdr2[40..48].copy_from_slice(&(data.len() as u64).to_le_bytes());

        let mut img = hdr;
        img.extend_from_slice(&phdr1);
        img.extend_from_slice(&phdr2);
        img.extend_from_slice(code);
        img.extend_from_slice(data);

        let info = load_elf(&mut bus, &img).unwrap();
        assert_eq!(info.entry, 0x1000);
        assert_eq!(bus.read_byte(0x1000).unwrap(), 0x13);
        assert_eq!(bus.read_byte(0x2000).unwrap(), 0xDE);
    }
}