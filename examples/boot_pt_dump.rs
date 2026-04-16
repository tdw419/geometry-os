/// Check early_pg_dir contents after boot reaches 250K instructions.

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

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 250_000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            let new_ppn = (cur_satp & 0x3FFFFF) as u64;
            let pt_base = new_ppn << 12;
            let asid = (cur_satp >> 22) & 0x1FF;
            for l1_idx in 0..1024u64 {
                let l1_addr = pt_base + l1_idx * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                if l1_pte == 0 { continue; }
                let is_leaf = (l1_pte & 0xE) != 0;
                if !is_leaf { continue; }
                let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u32;
                let flags = (l1_pte & 0xFF) as u32;
                for vpn0 in 0..512u32 {
                    let vpn_combined = ((l1_idx as u32) << 10) | vpn0;
                    let eff_ppn = (ppn_hi << 10) | vpn0;
                    vm.cpu.tlb.insert(vpn_combined, asid as u16, eff_ppn, flags);
                }
            }
        }
        last_satp = cur_satp;

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
        }

        let _ = vm.step();
        count += 1;
    }

    // Dump the active page table (early_pg_dir at 0x00802000)
    eprintln!("=== Active page table at PA 0x00802000 (early_pg_dir) ===");
    let mut non_zero_count = 0;
    for i in 0..1024u64 {
        let pte = vm.bus.read_word(0x00802000 + i * 4).unwrap_or(0);
        if pte != 0 {
            let is_leaf = (pte & 0xE) != 0;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let va_start = i << 22;
            eprintln!("  L1[{}] = 0x{:08X} (V={} leaf={} PPN=0x{:06X}) -> VA 0x{:08X}-0x{:08X}",
                i, pte, pte & 1, is_leaf, ppn, va_start, va_start + 0x3FFFFF);
            non_zero_count += 1;

            // If non-leaf, dump first few L2 entries
            if !is_leaf && ppn != 0 {
                let l2_base = (ppn as u64) << 12;
                for j in 0..16u64 {
                    let l2_pte = vm.bus.read_word(l2_base + j * 4).unwrap_or(0);
                    if l2_pte != 0 {
                        let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                        let l2_leaf = (l2_pte & 0xE) != 0;
                        eprintln!("    L2[{}] = 0x{:08X} (leaf={} PPN=0x{:06X})", j, l2_pte, l2_leaf, l2_ppn);
                    }
                }
            }
        }
    }
    eprintln!("Non-zero L1 entries: {}", non_zero_count);

    // Also dump trampoline_pg_dir
    eprintln!("\n=== trampoline_pg_dir at PA 0x01484000 ===");
    let mut non_zero_count2 = 0;
    for i in 0..1024u64 {
        let pte = vm.bus.read_word(0x01484000 + i * 4).unwrap_or(0);
        if pte != 0 {
            let is_leaf = (pte & 0xE) != 0;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let va_start = i << 22;
            eprintln!("  L1[{}] = 0x{:08X} (V={} leaf={} PPN=0x{:06X}) -> VA 0x{:08X}-0x{:08X}",
                i, pte, pte & 1, is_leaf, ppn, va_start, va_start + 0x3FFFFF);
            non_zero_count2 += 1;
        }
    }
    eprintln!("Non-zero L1 entries: {}", non_zero_count2);
}
