// Diagnostic: Check if DTB is readable at VA after MMU enable
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    let dtb_pa = dtb_addr as u32;
    let dtb_va = dtb_pa.wrapping_add(0xC0000000);
    let ibp_pa = 0x00C7A178u64;
    let ibp_va = 0xC0C7A178u32;

    eprintln!("DTB PA: 0x{:08X}, VA: 0x{:08X}", dtb_pa, dtb_va);
    eprintln!(
        "initial_boot_params PA: 0x{:08X}, VA: 0x{:08X}",
        ibp_pa, ibp_va
    );

    // Check DTB at PA (should always work)
    let magic_pa = vm.bus.read_word(dtb_pa as u64).unwrap_or(0);
    eprintln!("DTB magic at PA 0x{:08X}: 0x{:08X}", dtb_pa, magic_pa);

    // Check initial_boot_params at PA
    let ibp_val = vm.bus.read_word(ibp_pa).unwrap_or(0);
    eprintln!(
        "initial_boot_params at PA 0x{:08X}: 0x{:08X} (expect DTB PA 0x{:08X})",
        ibp_pa, ibp_val, dtb_pa
    );

    // Run until after MMU enable (watch for SATP changes)
    let mut last_satp = 0u32;
    let mut satp_changes = 0;
    let max_count = 300_000;

    for count in 0..max_count {
        let result = vm.step();
        let satp = vm.cpu.csr.read(geometry_os::riscv::csr::SATP);

        if satp != last_satp && satp != 0 {
            satp_changes += 1;
            eprintln!(
                "\n[SATP #{}] count={}: 0x{:08X} -> 0x{:08X}",
                satp_changes, count, last_satp, satp
            );
            last_satp = satp;

            // After MMU is enabled (SATP has mode bits), check VA mappings
            if satp & 0x80000000 != 0 {
                let magic_va = vm.bus.read_word(dtb_va as u64).unwrap_or(0);
                eprintln!(
                    "  DTB at VA 0x{:08X}: magic=0x{:08X} (expect 0xD00DFEED)",
                    dtb_va, magic_va
                );

                let ibp_va_val = vm.bus.read_word(ibp_va as u64).unwrap_or(0);
                eprintln!(
                    "  initial_boot_params at VA 0x{:08X}: 0x{:08X} (expect DTB PA 0x{:08X})",
                    ibp_va, ibp_va_val, dtb_pa
                );

                if magic_va != 0xD00DFEED {
                    eprintln!("  *** DTB NOT READABLE AT VA! ***");

                    // Dump the L1 entry that should map this VA
                    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                    let pg_dir_pa = pg_dir_ppn * 4096;
                    let vpn1 = (dtb_va >> 22) as usize;
                    let l1_entry_pa = pg_dir_pa + (vpn1 * 4) as u64;
                    let l1_entry = vm.bus.read_word(l1_entry_pa).unwrap_or(0);
                    eprintln!(
                        "  L1[{}] at PA 0x{:08X}: 0x{:08X}",
                        vpn1, l1_entry_pa, l1_entry
                    );

                    // Check if it's a megapage
                    let is_megapage = (l1_entry & 0xEF) == 0xCF; // V+R+X+G
                    let ppn1 = (l1_entry >> 20) & 0xFFF;
                    let mapped_pa = (ppn1 as u64) << 22;
                    eprintln!(
                        "  PPN1={}, is_megapage={}, mapped PA=0x{:08X}",
                        ppn1, is_megapage, mapped_pa
                    );
                }
            }
        }

        // Check for panic (look for PC in panic region)
        if count > 180000 && count % 1000 == 0 {
            // Read memblock memory.cnt periodically
            // memblock is a kernel internal, hard to find... skip for now
        }

        if satp_changes >= 3 {
            break;
        }
    }

    // Final check after all SATP changes
    eprintln!("\n=== Final state after {} SATP changes ===", satp_changes);
    let magic_va = vm.bus.read_word(dtb_va as u64).unwrap_or(0);
    eprintln!("DTB at VA 0x{:08X}: magic=0x{:08X}", dtb_va, magic_va);

    let ibp_va_val = vm.bus.read_word(ibp_va as u64).unwrap_or(0);
    eprintln!(
        "initial_boot_params at VA 0x{:08X}: 0x{:08X}",
        ibp_va, ibp_va_val
    );
}
