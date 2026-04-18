//! Diagnostic: Read DTB from RAM after boot_linux_setup and check all header fields.
//! Run: cargo run --example boot_dtb_header

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).unwrap();
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else { None };

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data, initramfs_data.as_deref(), 512, bootargs,
    ).expect("boot_linux_setup failed");

    // Read DTB header from PA
    let pa = dtb_addr as u64;
    let mut header = vec![0u8; 40];
    for (i, b) in header.iter_mut().enumerate() {
        *b = vm.bus.read_byte(pa + i as u64).unwrap_or(0);
    }

    // Parse FDT header (big-endian)
    let magic = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let totalsize = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    let off_dt_struct = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    let off_dt_strings = u32::from_be_bytes([header[12], header[13], header[14], header[15]]);
    let off_mem_rsvmap = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let version = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    let last_comp_version = u32::from_be_bytes([header[24], header[25], header[26], header[27]]);
    let boot_cpuid_phys = u32::from_be_bytes([header[28], header[29], header[30], header[31]]);
    let size_dt_strings = u32::from_be_bytes([header[32], header[33], header[34], header[35]]);
    let size_dt_struct = u32::from_be_bytes([header[36], header[37], header[38], header[39]]);

    eprintln!("DTB at PA 0x{:08X}:", dtb_addr as u32);
    eprintln!("  magic:             0x{:08X} (expect 0xD00DFEED)", magic);
    eprintln!("  totalsize:         {} (0x{:08X})", totalsize, totalsize);
    eprintln!("  off_dt_struct:     0x{:08X}", off_dt_struct);
    eprintln!("  off_dt_strings:    0x{:08X}", off_dt_strings);
    eprintln!("  off_mem_rsvmap:    0x{:08X}", off_mem_rsvmap);
    eprintln!("  version:           {}", version);
    eprintln!("  last_comp_version: {}", last_comp_version);
    eprintln!("  boot_cpuid_phys:   {}", boot_cpuid_phys);
    eprintln!("  size_dt_strings:   {}", size_dt_strings);
    eprintln!("  size_dt_struct:    {}", size_dt_struct);

    // Sanity checks
    if magic != 0xD00DFEED {
        eprintln!("ERROR: bad magic!");
    }
    if totalsize > 0x100000 {
        eprintln!("ERROR: totalsize too large (max 1MB)");
    }
    if version < 17 {
        eprintln!("WARNING: old DTB version, may need 17+");
    }
    if off_mem_rsvmap < 40 {
        eprintln!("ERROR: mem_rsvmap overlaps header");
    }

    // Dump first few bytes of mem_rsvmap
    eprintln!("\nmem_rsvmap entries:");
    let mut addr = pa + off_mem_rsvmap as u64;
    for i in 0..10 {
        let entry_addr = u64::from_be_bytes([
            vm.bus.read_byte(addr).unwrap_or(0),
            vm.bus.read_byte(addr+1).unwrap_or(0),
            vm.bus.read_byte(addr+2).unwrap_or(0),
            vm.bus.read_byte(addr+3).unwrap_or(0),
            vm.bus.read_byte(addr+4).unwrap_or(0),
            vm.bus.read_byte(addr+5).unwrap_or(0),
            vm.bus.read_byte(addr+6).unwrap_or(0),
            vm.bus.read_byte(addr+7).unwrap_or(0),
        ]);
        let entry_size = u64::from_be_bytes([
            vm.bus.read_byte(addr+8).unwrap_or(0),
            vm.bus.read_byte(addr+9).unwrap_or(0),
            vm.bus.read_byte(addr+10).unwrap_or(0),
            vm.bus.read_byte(addr+11).unwrap_or(0),
            vm.bus.read_byte(addr+12).unwrap_or(0),
            vm.bus.read_byte(addr+13).unwrap_or(0),
            vm.bus.read_byte(addr+14).unwrap_or(0),
            vm.bus.read_byte(addr+15).unwrap_or(0),
        ]);
        eprintln!("  [{}] addr=0x{:016X} size=0x{:016X}", i, entry_addr, entry_size);
        if entry_addr == 0 && entry_size == 0 {
            eprintln!("  (terminator)");
            break;
        }
        addr += 16;
    }

    // Dump struct tokens
    eprintln!("\nDTB struct (first 20 tokens):");
    let mut saddr = pa + off_dt_struct as u64;
    for i in 0..20 {
        let token = u32::from_be_bytes([
            vm.bus.read_byte(saddr).unwrap_or(0),
            vm.bus.read_byte(saddr+1).unwrap_or(0),
            vm.bus.read_byte(saddr+2).unwrap_or(0),
            vm.bus.read_byte(saddr+3).unwrap_or(0),
        ]);
        let name: String = match token {
            0x00000001 => "FDT_BEGIN_NODE".into(),
            0x00000002 => "FDT_END_NODE".into(),
            0x00000003 => "FDT_PROP".into(),
            0x00000004 => "FDT_NOP".into(),
            0x00000009 => "FDT_END".into(),
            _ => if token & 0xFF000000 == 0 { format!("string: {:?}", String::from_utf8_lossy(&token.to_be_bytes()).trim_end_matches('\0')) } else { format!("0x{:08X}", token) },
        };
        eprintln!("  [{}] 0x{:08X} = {}", i, token, name);
        saddr += 4;
        if token == 0x00000009 { break; }
    }
}
