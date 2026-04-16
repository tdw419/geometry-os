/// Dump the page table at early_pg_dir (PA 0x00802000) to see what's actually mapped.
/// Also check the boot page table and the kernel's setup_vm output.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    // After setup, dump the boot page table
    let boot_pt_ppn = vm.cpu.csr.satp & 0x3FFFFF;
    let boot_pt_phys = (boot_pt_ppn as u64) << 12;
    eprintln!("=== Boot page table (SATAP=0x{:08X}, PPN=0x{:06X}, PA=0x{:08X}) ===",
        vm.cpu.csr.satp, boot_pt_ppn, boot_pt_phys);

    eprintln!("\nBoot PT L1[0..10] (identity mapping):");
    for i in 0..10 {
        let val = vm.bus.read_word(boot_pt_phys + (i as u64) * 4).unwrap_or(0);
        let v = (val >> 0) & 1;
        let r = (val >> 1) & 1;
        let w = (val >> 2) & 1;
        let x = (val >> 3) & 1;
        let u = (val >> 4) & 1;
        let ppn = (val >> 10) & 0xFFF_FFFF;
        eprintln!("  L1[{}] = 0x{:08X}  V={} R={} W={} X={} U={} PPN=0x{:06X} -> PA 0x{:08X}",
            i, val, v, r, w, x, u, ppn, ppn << 12);
    }

    eprintln!("\nBoot PT L1[768..776] (kernel VA 0xC0000000+):");
    for i in 768..776 {
        let val = vm.bus.read_word(boot_pt_phys + (i as u64) * 4).unwrap_or(0);
        let v = (val >> 0) & 1;
        let r = (val >> 1) & 1;
        let w = (val >> 2) & 1;
        let x = (val >> 3) & 1;
        let u = (val >> 4) & 1;
        let ppn = (val >> 10) & 0xFFF_FFFF;
        eprintln!("  L1[{}] = 0x{:08X}  V={} R={} W={} X={} U={} PPN=0x{:06X} -> PA 0x{:08X}",
            i, val, v, r, w, x, u, ppn, ppn << 12);
    }

    // Now run until the kernel switches to early_pg_dir
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < 200_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("\n[{}] SATP changed: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})",
                count, last_satp, cur_satp, vm.cpu.pc);

            // Dump the new page table root
            let new_ppn = cur_satp & 0x3FFFFF;
            let new_phys = (new_ppn as u64) << 12;
            eprintln!("New page table at PA 0x{:08X}:", new_phys);
            eprintln!("  L1[768] = 0x{:08X}", vm.bus.read_word(new_phys + 768 * 4).unwrap_or(0));
            eprintln!("  L1[769] = 0x{:08X}", vm.bus.read_word(new_phys + 769 * 4).unwrap_or(0));
            eprintln!("  L1[770] = 0x{:08X}", vm.bus.read_word(new_phys + 770 * 4).unwrap_or(0));
            eprintln!("  L1[0]   = 0x{:08X}", vm.bus.read_word(new_phys + 0 * 4).unwrap_or(0));
            eprintln!("  L1[1]   = 0x{:08X}", vm.bus.read_word(new_phys + 1 * 4).unwrap_or(0));
            eprintln!("  L1[2]   = 0x{:08X}", vm.bus.read_word(new_phys + 2 * 4).unwrap_or(0));

            // Check what the identity injection did
            let l1_0_val = vm.bus.read_word(new_phys).unwrap_or(0);
            eprintln!("  L1[0] V={} R={} PPN=0x{:06X}",
                l1_0_val & 1, (l1_0_val >> 1) & 1, (l1_0_val >> 10) & 0xFFF_FFFF);

            last_satp = cur_satp;
        }

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            // Skip trap forwarding for now
        }

        vm.step();
        count += 1;

        // Stop after second SATP change
        if count > 178000 {
            break;
        }
    }

    // Final state: dump early_pg_dir L1[768..772] which maps VA 0xC0000000-0xC07FFFFF
    let final_satp = vm.cpu.csr.satp;
    let final_ppn = final_satp & 0x3FFFFF;
    let final_phys = (final_ppn as u64) << 12;
    eprintln!("\n=== Final state at count={} ===", count);
    eprintln!("SATP=0x{:08X}, PC=0x{:08X}", final_satp, vm.cpu.pc);
    eprintln!("Page table root at PA 0x{:08X}:", final_phys);
    for i in 768..776 {
        let val = vm.bus.read_word(final_phys + (i as u64) * 4).unwrap_or(0);
        let v = (val >> 0) & 1;
        let r = (val >> 1) & 1;
        let ppn = (val >> 10) & 0xFFF_FFFF;
        eprintln!("  L1[{}] = 0x{:08X}  V={} R={} PPN=0x{:06X} -> PA 0x{:08X}",
            i, val, v, r, ppn, ppn << 12);
    }

    // Check what PA 0xC000308Au32 actually maps to through the current SATP
    eprintln!("\n=== Translation check for VA 0xC000308Au32 ===");
    let vpn1 = (0xC000308Au32 >> 22) & 0x3FF;
    let vpn0 = (0xC000308Au32 >> 12) & 0x3FF;
    let offset = 0xC000308Au32 & 0xFFF;
    eprintln!("VPN1={}, VPN0={}, offset={}", vpn1, vpn0, offset);

    let l1_pte = vm.bus.read_word(final_phys + (vpn1 as u64) * 4).unwrap_or(0);
    eprintln!("L1[{}] = 0x{:08X}", vpn1, l1_pte);
    let l1_v = l1_pte & 1;
    let l1_ppn = (l1_pte >> 10) & 0xFFF_FFFF;
    let l1_rwx = (l1_pte >> 1) & 7;
    eprintln!("  V={} R/W/X={} PPN=0x{:06X}", l1_v, l1_rwx, l1_ppn);

    if l1_v != 0 && l1_rwx == 7 {
        // Megapage
        let pa = ((l1_ppn as u64) << 22) | ((vpn0 as u64) << 12) | (offset as u64);
        eprintln!("  -> Megapage PA = 0x{:08X}", pa);
        let actual_word = vm.bus.read_word(pa).unwrap_or(0);
        eprintln!("  -> Read word at PA 0x{:08X}: 0x{:08X}", pa, actual_word);
    } else if l1_v != 0 {
        // L2 pointer
        let l2_base = (l1_ppn as u64) << 12;
        let l2_addr = l2_base + (vpn0 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        eprintln!("  L2 at PA 0x{:08X}: PTE = 0x{:08X}", l2_addr, l2_pte);
        let l2_v = l2_pte & 1;
        let l2_ppn = (l2_pte >> 10) & 0xFFF_FFFF;
        let l2_rwx = (l2_pte >> 1) & 7;
        eprintln!("  L2: V={} R/W/X={} PPN=0x{:06X}", l2_v, l2_rwx, l2_ppn);
        if l2_v != 0 {
            let pa = ((l2_ppn as u64) << 12) | (offset as u64);
            eprintln!("  -> Leaf PA = 0x{:08X}", pa);
            let actual_word = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("  -> Read word at PA 0x{:08X}: 0x{:08X}", pa, actual_word);
        }
    }
}
