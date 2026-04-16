/// Verify fixup_kernel_page_table() produces correct mappings.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, _fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Run until first SATP change to setup_vm() page table
    let mut count = 0u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut satp_changes = 0u64;
    
    while count < 500_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            eprintln!("[{}] SATP changed: 0x{:08X} -> 0x{:08X}", count, last_satp, cur_satp);
            last_satp = cur_satp;
            
            if satp_changes >= 2 {
                // After the second SATP change, check page table state
                break;
            }
        }
        
        // Handle trap handler
        if vm.cpu.pc == _fw_addr as u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause & !(1u32 << 31);
            if mcause != 8 { // not ECALL_S
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }
        
        vm.step();
        count += 1;
    }
    
    eprintln!("\nAfter {} steps, {} SATP changes", count, satp_changes);
    eprintln!("Current SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("Current PC: 0x{:08X}", vm.cpu.pc);
    
    // Check the current page table
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) << 12;
    
    eprintln!("\nPage table at PA 0x{:08X}:", pg_dir_phys);
    
    let page_offset_ppn: u32 = 0xC000_0000 >> 12; // 0xC0000
    
    // Check key L1 entries for the kernel VA range
    for l1_idx in [768, 769, 770, 771, 772, 773, 774, 775, 776, 777, 778, 779, 780] {
        let l1_addr = pg_dir_phys + (l1_idx as u64) * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        if l1_pte == 0 { continue; }
        let ppn = (l1_pte & 0xFFFF_FC00) >> 10;
        let va_start = (l1_idx as u32) << 22;
        let is_leaf = (l1_pte & 0xE) != 0;
        let needs_fixup = ppn >= page_offset_ppn;
        
        if is_leaf {
            let pa_start = (if needs_fixup { ppn - page_offset_ppn } else { ppn }) << 22;
            eprintln!("  L1[{}] VA 0x{:08X}: PTE=0x{:08X} PPN=0x{:05X}{} -> PA 0x{:08X} {}",
                l1_idx, va_start, l1_pte, ppn,
                if needs_fixup { " (FIXED)" } else { "" },
                pa_start,
                if needs_fixup { "WAS_VIRTUAL" } else { "ok" }
            );
        } else {
            let l2_base = (if needs_fixup { ppn - page_offset_ppn } else { ppn }) as u64;
            eprintln!("  L1[{}] VA 0x{:08X}: PTE=0x{:08X} PPN=0x{:05X} -> L2 at PA 0x{:08X} {}",
                l1_idx, va_start, l1_pte, ppn, l2_base << 12,
                if needs_fixup { "WAS_VIRTUAL" } else { "ok" }
            );
            
            // Check a few L2 entries
            for l2_idx in [0, 64, 128, 192, 256] {
                let l2_addr = (l2_base << 12) + (l2_idx as u64) * 4;
                let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                if l2_pte == 0 { continue; }
                let l2_ppn = (l2_pte & 0xFFFF_FC00) >> 10;
                let l2_needs_fixup = l2_ppn >= page_offset_ppn;
                let va = va_start + (l2_idx as u32) * 0x1000;
                let pa = (if l2_needs_fixup { l2_ppn - page_offset_ppn } else { l2_ppn }) << 12;
                eprintln!("    L2[{}] VA 0x{:08X}: PTE=0x{:08X} PPN=0x{:05X} -> PA 0x{:08X} {}",
                    l2_idx, va, l2_pte, l2_ppn, pa,
                    if l2_needs_fixup { "WAS_VIRTUAL" } else { "ok" }
                );
            }
        }
    }
}
