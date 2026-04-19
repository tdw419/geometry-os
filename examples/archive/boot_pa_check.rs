/// Diagnostic: Check if kernel code at PA 0x003F000 is all zeros.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, _fw, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Check PA 0x003F000 (kernel code via L1[768] megapage)
    println!("=== PA 0x003F000 (kernel .text, should have code) ===");
    let mut zero_count = 0;
    let mut nonzero_count = 0;
    for i in 0..256u64 {
        if let Ok(val) = vm.bus.read_word(0x003F000 + i * 4) {
            if val == 0 {
                zero_count += 1;
            } else {
                nonzero_count += 1;
            }
            if i < 20 {
                println!("  PA 0x{:08X}: 0x{:08X}", 0x003F000 + i * 4, val);
            }
        }
    }
    println!("  zeros={}, nonzero={}", zero_count, nonzero_count);

    // Check PA 0x003F9CC (the specific address from the trace)
    println!("\n=== PA 0x003F9CC (PC from trace) ===");
    for i in 0..8u64 {
        if let Ok(val) = vm.bus.read_word(0x003F9CC + i * 4) {
            println!("  PA 0x{:08X}: 0x{:08X}", 0x003F9CC + i * 4, val);
        }
    }

    // Also check what the ELF says at VA 0xC003F9CC
    println!("\n=== ELF disassembly at VA 0xC003F9CC ===");
    // We can't run objdump here, but we can check the ELF load segments

    // Check PA 0x00000000 (should be kernel _start)
    println!("\n=== PA 0x00000000 (kernel entry) ===");
    for i in 0..8u64 {
        if let Ok(val) = vm.bus.read_word(i * 4) {
            println!("  PA 0x{:08X}: 0x{:08X}", i * 4, val);
        }
    }

    // Check PA 0x020B438 (from trace, in 0xC020B438 range)
    println!("\n=== PA 0x020B438 (from trace) ===");
    for i in 0..8u64 {
        if let Ok(val) = vm.bus.read_word(0x020B438 + i * 4) {
            println!("  PA 0x{:08X}: 0x{:08X}", 0x020B438 + i * 4, val);
        }
    }

    // The L1[768] megapage should map VA 0xC0000000-0xC01FFFFF to PA 0x00000000-0x01FFFFF
    // Check L1[768] in the boot page table
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir = (ppn as u64) * 4096;
    println!(
        "\n=== Boot page table (SATP=0x{:08X}, pg_dir=0x{:08X}) ===",
        satp, pg_dir
    );
    for i in 768..777u32 {
        if let Ok(pte) = vm.bus.read_word(pg_dir + (i * 4) as u64) {
            let full_ppn = (pte >> 10) & 0x3FFFFF;
            let ppn_hi = (full_ppn >> 10) & 0xFFF;
            println!(
                "  L1[{}] = 0x{:08X} PPN=0x{:06X} ppn_hi=0x{:03X} -> PA_base=0x{:08X}",
                i,
                pte,
                full_ppn,
                ppn_hi,
                (ppn_hi as u64) << 22
            );
        }
    }

    // Check if the kernel's first load segment covers PA 0x003F9CC
    println!("\n=== ELF segments ===");
    let mut offset = 0;
    // Simple ELF parsing - look for LOAD segments
    while offset + 32 <= kernel_image.len() as usize {
        // ELF32 header: e_phoff at offset 28 (4 bytes)
        if offset == 0 && kernel_image.len() > 32 {
            let e_phoff = u32::from_le_bytes(kernel_image[28..32].try_into().unwrap());
            let e_phentsize = u16::from_le_bytes(kernel_image[42..44].try_into().unwrap());
            let e_phnum = u16::from_le_bytes(kernel_image[44..46].try_into().unwrap());
            println!(
                "  phoff={} phentsize={} phnum={}",
                e_phoff, e_phentsize, e_phnum
            );

            for p in 0..e_phnum {
                let ph_off = e_phoff as usize + p as usize * e_phentsize as usize;
                if ph_off + 32 <= kernel_image.len() {
                    let p_type =
                        u32::from_le_bytes(kernel_image[ph_off..ph_off + 4].try_into().unwrap());
                    if p_type == 1 {
                        // PT_LOAD
                        let p_offset = u32::from_le_bytes(
                            kernel_image[ph_off + 4..ph_off + 8].try_into().unwrap(),
                        );
                        let p_vaddr = u32::from_le_bytes(
                            kernel_image[ph_off + 8..ph_off + 12].try_into().unwrap(),
                        );
                        let p_paddr = u32::from_le_bytes(
                            kernel_image[ph_off + 12..ph_off + 16].try_into().unwrap(),
                        );
                        let p_filesz = u32::from_le_bytes(
                            kernel_image[ph_off + 16..ph_off + 20].try_into().unwrap(),
                        );
                        let p_memsz = u32::from_le_bytes(
                            kernel_image[ph_off + 20..ph_off + 24].try_into().unwrap(),
                        );
                        println!("  LOAD: offset=0x{:X} vaddr=0x{:08X} paddr=0x{:08X} filesz=0x{:X} memsz=0x{:X}",
                            p_offset, p_vaddr, p_paddr, p_filesz, p_memsz);
                        // Check if 0x003F9CC is in this segment
                        let seg_end = p_paddr + p_memsz;
                        if p_paddr <= 0x003F9CC && 0x003F9CC < seg_end {
                            println!("    ^^^ PA 0x003F9CC is in this segment!");
                        }
                    }
                }
            }
            break;
        }
        offset += 1;
    }
}
