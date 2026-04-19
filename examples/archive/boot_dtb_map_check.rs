// Check the DTB that the boot setup generates
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Read DTB header from the VM's memory
    let dtb_bytes: Vec<u8> = (0..64)
        .map(|i| vm.bus.read_byte(dtb_addr + i as u64).unwrap_or(0))
        .collect();

    // FDT header: magic(4) + totalsize(4) + off_dt_struct(4) + off_dt_strings(4) + off_mem_rsvmap(4) + version(4)
    let magic = u32::from_be_bytes([dtb_bytes[0], dtb_bytes[1], dtb_bytes[2], dtb_bytes[3]]);
    let totalsize = u32::from_be_bytes([dtb_bytes[4], dtb_bytes[5], dtb_bytes[6], dtb_bytes[7]]);
    let off_struct = u32::from_be_bytes([dtb_bytes[8], dtb_bytes[9], dtb_bytes[10], dtb_bytes[11]]);
    let off_strings =
        u32::from_be_bytes([dtb_bytes[12], dtb_bytes[13], dtb_bytes[14], dtb_bytes[15]]);
    let off_rsvmap =
        u32::from_be_bytes([dtb_bytes[16], dtb_bytes[17], dtb_bytes[18], dtb_bytes[19]]);
    let version = u32::from_be_bytes([dtb_bytes[20], dtb_bytes[21], dtb_bytes[22], dtb_bytes[23]]);

    eprintln!("DTB at PA 0x{:08X}:", dtb_addr);
    eprintln!("  magic = 0x{:08X} (expected 0xD00DFEED)", magic);
    eprintln!("  totalsize = {} bytes", totalsize);
    eprintln!("  off_struct = 0x{:08X}", off_struct);
    eprintln!("  off_strings = 0x{:08X}", off_strings);
    eprintln!("  off_rsvmap = 0x{:08X}", off_rsvmap);
    eprintln!("  version = {}", version);

    if magic != 0xD00DFEED {
        eprintln!("ERROR: Invalid DTB magic!");
        return;
    }

    // Now check: does the kernel's page table map this DTB address?
    // _dtb_early_va should be dtb_addr + 0xC0000000
    let dtb_va = (dtb_addr as u32).wrapping_add(0xC0000000);
    eprintln!("\n_dtb_early_va = 0x{:08X}", dtb_va);
    eprintln!("_dtb_early_pa = 0x{:08X}", dtb_addr as u32);

    // Check _dtb_early_va protection
    let stored_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let stored_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!(
        "Stored _dtb_early_va = 0x{:08X} (expected 0x{:08X})",
        stored_va, dtb_va
    );
    eprintln!(
        "Stored _dtb_early_pa = 0x{:08X} (expected 0x{:08X})",
        stored_pa, dtb_addr as u32
    );

    // Check what the kernel would see at the DTB VA through the MMU
    // Read the first 4 bytes of DTB via VA (simulate kernel access)
    // The kernel reads via VA 0xC1579000. We need to check if the page table
    // at the active SATP maps this VA correctly.
    let satp = vm.cpu.csr.satp;
    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_phys = pg_dir_ppn * 4096;
    eprintln!(
        "\nActive SATP = 0x{:08X}, pg_dir at PA 0x{:08X}",
        satp, pg_dir_phys
    );

    // Walk page table for DTB VA
    let vpn1 = ((dtb_va >> 22) & 0x3FF) as u64;
    let vpn0 = ((dtb_va >> 12) & 0x3FF) as u64;
    let l1_addr = pg_dir_phys + vpn1 * 4;
    let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
    eprintln!("DTB VA page walk: VPN1={} VPN0={}", vpn1, vpn0);
    eprintln!(
        "  L1[{}] at PA 0x{:08X} = 0x{:08X}",
        vpn1, l1_addr, l1_entry
    );

    let l1_valid = (l1_entry & 1) != 0;
    let l1_leaf = (l1_entry & 0xE) != 0;
    if l1_valid && l1_leaf {
        // Megapage
        let ppn = ((l1_entry >> 10) & 0x3FFFFF) << 2; // 4MB aligned
        let offset = (dtb_va as u64) & 0x3FFFFF;
        let pa = (ppn as u64) << 10 | offset;
        eprintln!(
            "  -> Megapage, PA = 0x{:08X} (expected 0x{:08X})",
            pa, dtb_addr
        );
    } else if l1_valid {
        // L2 page table
        let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn * 4096;
        let l2_addr = l2_base + vpn0 * 4;
        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
        eprintln!("  L2 at PA 0x{:08X} = 0x{:08X}", l2_addr, l2_entry);
        if (l2_entry & 1) != 0 {
            let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
            let offset = (dtb_va as u64) & 0xFFF;
            let pa = (l2_ppn as u64) * 4096 + offset;
            eprintln!("  -> PA = 0x{:08X} (expected 0x{:08X})", pa, dtb_addr);
        } else {
            eprintln!("  -> NOT MAPPED! L2 entry is invalid");
        }
    } else {
        eprintln!("  -> NOT MAPPED! L1 entry is invalid");
    }

    // Also check if the kernel's early page table (SATP=0x80000802) maps the DTB
    let early_satp: u32 = 0x80000802;
    let early_pg_dir_ppn = (early_satp & 0x3FFFFF) as u64;
    let early_pg_dir_phys = early_pg_dir_ppn * 4096;
    let early_l1_addr = early_pg_dir_phys + vpn1 * 4;
    let early_l1_entry = vm.bus.read_word(early_l1_addr).unwrap_or(0);
    eprintln!(
        "\nEarly SATP 0x{:08X} (pg_dir PA 0x{:08X}):",
        early_satp, early_pg_dir_phys
    );
    eprintln!(
        "  L1[{}] at PA 0x{:08X} = 0x{:08X}",
        vpn1, early_l1_addr, early_l1_entry
    );

    let early_l1_valid = (early_l1_entry & 1) != 0;
    let early_l1_leaf = (early_l1_entry & 0xE) != 0;
    if early_l1_valid && early_l1_leaf {
        let ppn = ((early_l1_entry >> 10) & 0x3FFFFF) << 2;
        let offset = (dtb_va as u64) & 0x3FFFFF;
        let pa = (ppn as u64) << 10 | offset;
        eprintln!(
            "  -> Megapage, PA = 0x{:08X} (expected 0x{:08X})",
            pa, dtb_addr
        );
    } else if early_l1_valid {
        let l2_ppn = ((early_l1_entry >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn * 4096;
        let l2_addr = l2_base + vpn0 * 4;
        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
        eprintln!("  L2 at PA 0x{:08X} = 0x{:08X}", l2_addr, l2_entry);
        if (l2_entry & 1) != 0 {
            let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
            let offset = (dtb_va as u64) & 0xFFF;
            let pa = (l2_ppn as u64) * 4096 + offset;
            eprintln!("  -> PA = 0x{:08X} (expected 0x{:08X})", pa, dtb_addr);
        } else {
            eprintln!("  -> NOT MAPPED!");
        }
    } else {
        eprintln!("  -> NOT MAPPED!");
    }

    // Also check the trampoline page table (SATP=0x80001484)
    let tramp_satp: u32 = 0x80001484;
    let tramp_pg_dir_ppn = (tramp_satp & 0x3FFFFF) as u64;
    let tramp_pg_dir_phys = tramp_pg_dir_ppn * 4096;
    let tramp_l1_addr = tramp_pg_dir_phys + vpn1 * 4;
    let tramp_l1_entry = vm.bus.read_word(tramp_l1_addr).unwrap_or(0);
    eprintln!(
        "\nTrampoline SATP 0x{:08X} (pg_dir PA 0x{:08X}):",
        tramp_satp, tramp_pg_dir_phys
    );
    eprintln!(
        "  L1[{}] at PA 0x{:08X} = 0x{:08X}",
        vpn1, tramp_l1_addr, tramp_l1_entry
    );
}
