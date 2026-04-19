use geometry_os::riscv::cpu::Privilege;
/// Diagnostic: Add detailed logging to fixup to see what's happening to L1[771].
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 500_000u64;
    let mut count: u64 = 0;
    let mut last_satap: u32 = vm.cpu.csr.satp;
    let fw_addr_u32 = fw_addr as u32;
    let mut fixup_count: u64 = 0;

    // Check L1[771] before boot
    let init_satp = vm.cpu.csr.satp;
    let init_ppn = init_satp & 0x3FFFFF;
    let init_pg_dir = (init_ppn as u64) * 4096;
    if let Ok(pte) = vm.bus.read_word(init_pg_dir + 771 * 4) {
        println!("[diag] BEFORE BOOT: L1[771] in boot_pg_dir = 0x{:08X}", pte);
    }

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
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

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satap {
            let pg_dir_phys = ((cur_satp & 0x3FFFFF) as u64) * 4096;
            println!(
                "\n[diag] SATP change #{}: 0x{:08X} -> 0x{:08X} at count={}",
                fixup_count + 1,
                last_satap,
                cur_satp,
                count
            );
            println!("[diag] pg_dir at PA 0x{:08X}", pg_dir_phys);

            // Check L1[771] BEFORE fixup
            if let Ok(pte) = vm.bus.read_word(pg_dir_phys + 771 * 4) {
                let ppn = (pte >> 10) & 0x3FFFFF;
                println!(
                    "[diag] L1[771] BEFORE fixup: PTE=0x{:08X} PPN=0x{:06X} ({})",
                    pte,
                    ppn,
                    if ppn >= 0xC0000 { "NEEDS FIXUP" } else { "OK" }
                );
            }

            // Run fixup (matching boot_linux)
            const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12; // 0xC0000
            const PPN_MASK: u32 = 0xFFFF_FC00;
            const PTE_V: u32 = 1;
            const LEAF_FLAGS: u32 = 2 | 4 | 8;
            let mut fixed_count: u32 = 0;

            for i in 0..1024u32 {
                let l1_addr = pg_dir_phys + (i as u64) * 4;
                if let Ok(l1_pte) = vm.bus.read_word(l1_addr) {
                    if (l1_pte & PTE_V) == 0 {
                        continue;
                    }
                    let l1_ppn = (l1_pte & PPN_MASK) >> 10;
                    if l1_ppn >= PAGE_OFFSET_PPN {
                        let fixed_ppn = l1_ppn - PAGE_OFFSET_PPN;
                        let fixed_pte = (l1_pte & !PPN_MASK) | (fixed_ppn << 10);
                        if i == 771 {
                            println!("[diag]   Fixing L1[771]: 0x{:08X} -> 0x{:08X} (PPN 0x{:06X} -> 0x{:06X})",
                                l1_pte, fixed_pte, l1_ppn, fixed_ppn);
                        }
                        vm.bus.write_word(l1_addr, fixed_pte).ok();
                        fixed_count += 1;
                    }
                    if (l1_pte & LEAF_FLAGS) == 0 {
                        let final_ppn = if l1_ppn >= PAGE_OFFSET_PPN {
                            l1_ppn - PAGE_OFFSET_PPN
                        } else {
                            l1_ppn
                        };
                        let l2_base = (final_ppn as u64) << 12;
                        if l2_base < 0x1000_0000 {
                            for j in 0..1024u32 {
                                let l2_addr = l2_base + (j as u64) * 4;
                                if let Ok(l2_pte) = vm.bus.read_word(l2_addr) {
                                    if (l2_pte & PTE_V) == 0 {
                                        continue;
                                    }
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
            println!("[diag] Fixup: fixed {} L1 entries", fixed_count);

            // Check L1[771] AFTER fixup
            if let Ok(pte) = vm.bus.read_word(pg_dir_phys + 771 * 4) {
                let ppn = (pte >> 10) & 0x3FFFFF;
                println!(
                    "[diag] L1[771] AFTER fixup: PTE=0x{:08X} PPN=0x{:06X}",
                    pte, ppn
                );
            }

            // Inject identity mappings
            let identity_pte: u32 = 0x0000_00CF;
            let l1_entries: &[u32] = &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127,
            ];
            let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
            if !((l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0) {
                for &idx in l1_entries {
                    vm.bus
                        .write_word(pg_dir_phys + (idx * 4) as u64, identity_pte | (idx << 20))
                        .ok();
                }
                let existing = vm.bus.read_word(pg_dir_phys + 776 * 4).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus
                        .write_word(pg_dir_phys + 776 * 4, identity_pte | (8u32 << 20))
                        .ok();
                }
                vm.cpu.tlb.flush_all();
                println!("[diag] Injected identity mappings");
            }

            // Check L1[771] AFTER identity injection
            if let Ok(pte) = vm.bus.read_word(pg_dir_phys + 771 * 4) {
                let ppn = (pte >> 10) & 0x3FFFFF;
                println!(
                    "[diag] L1[771] AFTER identity inject: PTE=0x{:08X} PPN=0x{:06X}",
                    pte, ppn
                );
            }

            fixup_count += 1;
            last_satap = cur_satp;
        }

        let step_result = vm.step();
        if matches!(
            step_result,
            geometry_os::riscv::cpu::StepResult::FetchFault
                | geometry_os::riscv::cpu::StepResult::LoadFault
                | geometry_os::riscv::cpu::StepResult::StoreFault
        ) {
            if count < 200000 {
                eprintln!("[diag] Fault at count={}: PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                // Check L1[771] at time of fault
                let satp = vm.cpu.csr.satp;
                let ppn = satp & 0x3FFFFF;
                let pg_dir = (ppn as u64) * 4096;
                if let Ok(pte) = vm.bus.read_word(pg_dir + 771 * 4) {
                    eprintln!("[diag]   L1[771] at fault time: PTE=0x{:08X}", pte);
                }
            }
        }

        count += 1;
    }

    println!("\n[diag] Final: count={} PC=0x{:08X}", count, vm.cpu.pc);
}
