/// Diagnostic: Check page table state after SATP changes during boot.
/// Manually reads PTEs to verify if fixup is working.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

/// Manual fixup: subtract PAGE_OFFSET from PPNs >= 0xC0000
fn manual_fixup(vm: &mut RiscvVm, pg_dir_phys: u64) {
    const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12;
    const PPN_MASK: u32 = 0xFFFF_FC00;
    const LEAF_FLAGS: u32 = 2 | 4 | 8;

    let mut l2_tables: Vec<u64> = Vec::new();

    for i in 0..1024u32 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let pte = match vm.bus.read_word(addr) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if (pte & 1) == 0 { continue; }
        let ppn = (pte & PPN_MASK) >> 10;
        if ppn >= PAGE_OFFSET_PPN {
            let fixed = (pte & !PPN_MASK) | ((ppn - PAGE_OFFSET_PPN) << 10);
            vm.bus.write_word(addr, fixed).ok();
        }
        if (pte & LEAF_FLAGS) == 0 {
            let final_ppn = if ppn >= PAGE_OFFSET_PPN { ppn - PAGE_OFFSET_PPN } else { ppn };
            let l2_base = (final_ppn as u64) << 12;
            if l2_base < 0x1000_0000 {
                l2_tables.push(l2_base);
            }
        }
    }

    for l2_base in &l2_tables {
        for j in 0..1024u32 {
            let addr = *l2_base + (j as u64) * 4;
            let pte = match vm.bus.read_word(addr) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if (pte & 1) == 0 { continue; }
            let ppn = (pte & PPN_MASK) >> 10;
            if ppn >= PAGE_OFFSET_PPN {
                let fixed = (pte & !PPN_MASK) | ((ppn - PAGE_OFFSET_PPN) << 10);
                vm.bus.write_word(addr, fixed).ok();
            }
        }
    }
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max = 800_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut first_fault: bool = true;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        // M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
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
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 1)) | (sie << 1);
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

        // SATP change + fixup
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X}", count, last_satp, cur_satp);
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;

                // Read PTEs BEFORE fixup
                let l1_768_before = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
                let l1_768_ppn_before = (l1_768_before >> 10) & 0xFFF_FFFF;

                manual_fixup(&mut vm, pg_dir_phys);

                // Read PTEs AFTER fixup
                let l1_768_after = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
                let l1_768_ppn_after = (l1_768_after >> 10) & 0xFFF_FFFF;

                eprintln!("  pg_dir at PA 0x{:08X}", pg_dir_phys);
                eprintln!("  L1[768] before: 0x{:08X} PPN=0x{:05X}", l1_768_before, l1_768_ppn_before);
                eprintln!("  L1[768] after:  0x{:08X} PPN=0x{:05X}", l1_768_after, l1_768_ppn_after);

                // Show all non-zero kernel VA entries (768-1023)
                let mut fixed_count = 0u32;
                for idx in 768..1024 {
                    let pte = vm.bus.read_word(pg_dir_phys + (idx * 4) as u64).unwrap_or(0);
                    if pte != 0 {
                        let ppn = (pte >> 10) & 0xFFF_FFFF;
                        let is_leaf = (pte & 0xE) != 0;
                        let is_fixed = ppn < 0xC0000 && (idx as u32) >= 768;
                        if is_leaf && ppn < 0xC0000 {
                            eprintln!("  L1[{}] = 0x{:08X} -> PA 0x{:05X}000 {}",
                                idx, pte, ppn, if is_fixed { "FIXED" } else { "ok" });
                        } else if !is_leaf {
                            // Non-leaf: L2 page table
                            eprintln!("  L1[{}] = 0x{:08X} -> L2 at PA 0x{:05X}000 (non-leaf)",
                                idx, pte, ppn);
                        }
                        fixed_count += 1;
                    }
                }
                eprintln!("  Total non-zero kernel entries: {}", fixed_count);

                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        let step_result = vm.step();
        if let StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault = step_result {
            if vm.cpu.privilege == Privilege::Supervisor && first_fault {
                first_fault = false;
                let fault_type = match step_result {
                    StepResult::FetchFault => "fetch",
                    StepResult::LoadFault => "load",
                    StepResult::StoreFault => "store",
                    _ => unreachable!(),
                };
                eprintln!("\n[{}] FIRST S-mode {} fault: PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                    count, fault_type, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                eprintln!("  SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} satp=0x{:08X}",
                    vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.csr.satp);

                // Check the page table walk for the faulting VA
                let fault_va = vm.cpu.csr.stval;
                let vpn1 = (fault_va >> 22) & 0x3FF;
                let vpn0 = (fault_va >> 12) & 0x3FF;
                let satp_val = vm.cpu.csr.satp;
                let pgd_pa = ((satp_val & 0x3FFFFF) as u64) * 4096;
                let l1_pte = vm.bus.read_word(pgd_pa + (vpn1 as u64) * 4).unwrap_or(0);
                eprintln!("  Fault VA 0x{:08X} VPN1={} VPN0={}", fault_va, vpn1, vpn0);
                eprintln!("  Page dir at PA 0x{:08X}", pgd_pa);
                eprintln!("  L1[{}] = 0x{:08X} (V={} leaf={})", vpn1, l1_pte,
                    (l1_pte & 1) != 0, (l1_pte & 0xE) != 0);

                if (l1_pte & 1) != 0 {
                    let l1_ppn = ((l1_pte >> 10) & 0xFFF_FFFF) as u64;
                    if (l1_pte & 0xE) == 0 {
                        // Non-leaf: walk L2
                        let l2_base = l1_ppn << 12;
                        let l2_addr = l2_base + (vpn0 as u64) * 4;
                        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                        let l2_ppn = ((l2_pte >> 10) & 0xFFF_FFFF) as u64;
                        eprintln!("  L2 table at PA 0x{:08X}", l2_base);
                        eprintln!("  L2[{}] at PA 0x{:08X} = 0x{:08X} (V={} leaf={})",
                            vpn0, l2_addr, l2_pte, (l2_pte & 1) != 0, (l2_pte & 0xE) != 0);
                        if (l2_pte & 1) != 0 {
                            eprintln!("  -> Maps to PA 0x{:08X} (PPN=0x{:05X})",
                                (l2_ppn << 12) as u32, l2_ppn as u32);
                            // Check what's at that PA
                            let target_pa = l2_ppn << 12;
                            let word = vm.bus.read_word(target_pa).unwrap_or(0);
                            eprintln!("  -> Word at PA 0x{:08X}: 0x{:08X}", target_pa as u32, word);
                        }
                    } else {
                        // Leaf (megapage)
                        eprintln!("  -> Megapage maps to PA 0x{:08X}", (l1_ppn << 12) as u32);
                    }
                }
            }
        }

        count += 1;
    }

    eprintln!("\n[diag] Final: count={} PC=0x{:08X} priv={:?} satp=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
}
