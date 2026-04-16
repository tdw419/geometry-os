/// Diagnostic: Check the exception handler table at excp_vect_table (0xC0C00AA4)
/// and understand why it contains 0x3FFFF000 instead of a valid handler address.

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 180_000u64; // Just past the first SATP change (at 177K)
    let mut count: u64 = 0;
    let mut last_satap: u32 = vm.cpu.csr.satp;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
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
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // SATP change detection + fixup
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satap {
            println!("[diag] SATP change: 0x{:08X} -> 0x{:08X} at count={}", last_satap, cur_satp, count);
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;

                // Run fixup (same as boot_linux)
                const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12;
                const PPN_MASK: u32 = 0xFFFF_FC00;
                const PTE_V: u32 = 1;
                const LEAF_FLAGS: u32 = 2 | 4 | 8;

                for i in 0..1024u32 {
                    let l1_addr = pg_dir_phys + (i as u64) * 4;
                    if let Ok(l1_pte) = vm.bus.read_word(l1_addr) {
                        if (l1_pte & PTE_V) == 0 { continue; }
                        let l1_ppn = (l1_pte & PPN_MASK) >> 10;
                        if l1_ppn >= PAGE_OFFSET_PPN {
                            let fixed_ppn = l1_ppn - PAGE_OFFSET_PPN;
                            let fixed_pte = (l1_pte & !PPN_MASK) | (fixed_ppn << 10);
                            vm.bus.write_word(l1_addr, fixed_pte).ok();
                        }
                        if (l1_pte & LEAF_FLAGS) == 0 {
                            let final_ppn = if l1_ppn >= PAGE_OFFSET_PPN { l1_ppn - PAGE_OFFSET_PPN } else { l1_ppn };
                            let l2_base = (final_ppn as u64) << 12;
                            if l2_base < 0x1000_0000 {
                                for j in 0..1024u32 {
                                    let l2_addr = l2_base + (j as u64) * 4;
                                    if let Ok(l2_pte) = vm.bus.read_word(l2_addr) {
                                        if (l2_pte & PTE_V) == 0 { continue; }
                                        let l2_ppn = (l2_pte & PPN_MASK) >> 10;
                                        if l2_ppn >= PAGE_OFFSET_PPN {
                                            let fixed = l2_ppn - PAGE_OFFSET_PPN;
                                            let fixed_pte = (l2_pte & !PPN_MASK) | (fixed << 10);
                                            vm.bus.write_word(l2_addr, fixed_pte).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Inject identity mappings
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127];
                let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                if !((l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0) {
                    for &idx in l1_entries {
                        vm.bus.write_word(pg_dir_phys + (idx * 4) as u64, identity_pte | (idx << 20)).ok();
                    }
                    let existing = vm.bus.read_word(pg_dir_phys + 776 * 4).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(pg_dir_phys + 776 * 4, identity_pte | (8u32 << 20)).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    println!("[diag] Injected identity mappings at pg_dir PA 0x{:08X}", pg_dir_phys);
                }

                // Dump the page table entries for the excp_vect_table region
                // VA 0xC0C00AA4 -> VPN1 = 0xC0C00AA4 >> 22 = 772, VPN0 = (0xC0C00AA4 >> 12) & 0x3FF = 0x0AA = 170
                let vpn1 = 0xC0C0_0AA4u32 >> 22;
                let vpn0 = (0xC0C0_0AA4u32 >> 12) & 0x3FF;
                println!("[diag] excp_vect_table VA=0xC0C00AA4: VPN1={} VPN0={}", vpn1, vpn0);
                if let Ok(l1_pte) = vm.bus.read_word(pg_dir_phys + (vpn1 * 4) as u64) {
                    let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
                    let is_leaf = (l1_pte & 0xE) != 0;
                    println!("[diag]   L1[{}]=0x{:08X} PPN=0x{:X} leaf={}", vpn1, l1_pte, l1_ppn, is_leaf);
                    if !is_leaf {
                        let l2_addr = (l1_ppn as u64) << 12;
                        if let Ok(l2_pte) = vm.bus.read_word(l2_addr + (vpn0 * 4) as u64) {
                            let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                            println!("[diag]     L2[{}]=0x{:08X} PPN=0x{:X}", vpn0, l2_pte, l2_ppn);
                            // The physical address of excp_vect_table
                            let pa = (l2_ppn as u64) << 12 | (0xAA4 & 0xFFF);
                            println!("[diag]     excp_vect_table PA = 0x{:08X}", pa);

                            // Read the first 16 entries of excp_vect_table
                            println!("[diag]     excp_vect_table entries:");
                            for i in 0..16u64 {
                                if let Ok(val) = vm.bus.read_word(pa + i * 4) {
                                    println!("       [{}]: 0x{:08X}", i, val);
                                }
                            }
                        }
                    } else {
                        // Megapage: PA = l1_ppn * 2MB + offset
                        let pa = (l1_ppn as u64) << 22 | (0xAA4 & 0x3FFFFF);
                        println!("[diag]     Megapage, excp_vect_table PA = 0x{:08X}", pa);
                        println!("[diag]     excp_vect_table entries:");
                        for i in 0..16u64 {
                            if let Ok(val) = vm.bus.read_word(pa + i * 4) {
                                println!("       [{}]: 0x{:08X}", i, val);
                            }
                        }
                    }
                }
            }
            last_satap = cur_satp;
        }

        vm.step();
        count += 1;
    }

    // After boot setup, check the excp_vect_table using MMU translation
    println!("\n[diag] Post-boot: checking excp_vect_table via MMU");
    println!("[diag] SATP=0x{:08X}", vm.cpu.csr.satp);

    // The table is at VA 0xC0C00AA4. Read 16 entries.
    // We can't use the MMU from outside, so read via the physical address we found.
    // Let's just check what the kernel sees at that address by looking at the raw ELF.
    // But more importantly, let's check what our PTE fixup left.
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) * 4096;
    println!("[diag] Current page directory at PA 0x{:08X}", pg_dir_phys);

    // Check L1[772] (for VA 0xC0C00000+)
    let vpn1 = 772;
    if let Ok(l1_pte) = vm.bus.read_word(pg_dir_phys + (vpn1 * 4) as u64) {
        println!("[diag] L1[772] = 0x{:08X}", l1_pte);
    }

    // Also check what the kernel's setup_vm() created for L1[768..776]
    for i in 768..777u32 {
        if let Ok(l1_pte) = vm.bus.read_word(pg_dir_phys + (i * 4) as u64) {
            let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
            let flags = l1_pte & 0x3FF;
            let is_leaf = (l1_pte & 0xE) != 0;
            println!("[diag] L1[{}] = 0x{:08X} PPN=0x{:06X} flags=0x{:03X} leaf={}",
                i, l1_pte, l1_ppn, flags, is_leaf);
        }
    }
}
