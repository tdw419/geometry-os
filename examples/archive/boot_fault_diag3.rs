// Diagnostic: trace exactly what happens around the first fetch fault.
// The kernel faults trying to call alloc_pte_fixmap at 0x804046C8 (should be 0xC04046C8).
// We trace the last N PCs before the fault to find the jalr that loads the corrupted address.

use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let max_instr: u64 = 200_000;
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut pc_history: Vec<(u64, u32)> = Vec::with_capacity(200);
    let mut smode_fault_count: u64 = 0;

    while count < max_instr {
        // SATP change handling (same as boot_linux)
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[diag] SATP changed at count={}: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                let identity_pte: u32 = 0x0000_00CF;
                for &l1_idx in device_l1_entries {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                vm.cpu.tlb.flush_all();

                // After second SATP change (early_pg_dir), check L1[769] mapping
                if count > 170_000 {
                    let l1_769_addr = pg_dir_phys + 769 * 4;
                    let l1_769 = vm.bus.read_word(l1_769_addr).unwrap_or(0);
                    let ppn = l1_769 >> 10;
                    eprintln!(
                        "[diag] L1[769] at PA 0x{:08X}: pte=0x{:08X} ppn=0x{:X}",
                        l1_769_addr, l1_769, ppn
                    );

                    // Check kernel_map
                    let km_phys: u64 = 0x00C79E90;
                    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                    eprintln!(
                        "[diag] kernel_map: pa=0x{:X} vapo=0x{:X} vkpo=0x{:X}",
                        km_pa, km_vapo, km_vkpo
                    );

                    // Try to read the pt_ops pointer.
                    // In the kernel, pt_ops is a static variable. It's typically in .data or .bss.
                    // Let's check if the early_pg_dir_pte_ops struct at 0xC0404xxx has correct values.
                    // The struct is at some VA. Let's scan for the pattern 0xC04046C8 (alloc_pte_fixmap VA).
                    // Actually, let's check the second LOAD segment (.init.data) for pt_ops.
                    // early_pg_dir_pte_ops is likely near alloc_pte_fixmap in .init.text.
                    // Let's read a range of memory around where pt_ops might be.
                    // The pt_ops pointer itself is in .data/.bss. Let's check 0xC0C00000+ area.
                    // Actually, let's look at paging_init code to find where pt_ops is stored.

                    // For now, let's check the L1[768] entry too (0xC0000000 mapping)
                    let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
                    eprintln!(
                        "[diag] L1[768]: pte=0x{:08X} ppn=0x{:X}",
                        l1_768,
                        l1_768 >> 10
                    );
                }
            }
            last_satp = cur_satp;
        }

        // M-mode trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M - SBI call
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
                let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;
                if mpp != 3 && (vm.cpu.csr.stvec & !0x3) != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = vm.cpu.csr.stvec & !0x3;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let pc_before = vm.cpu.pc;
        let step_result = vm.step();

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                smode_fault_count += 1;
                if smode_fault_count <= 5 {
                    eprintln!("[diag] Fault #{} at count={}: PC=0x{:08X} prev_pc=0x{:08X} scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} priv={:?}",
                        smode_fault_count, count, vm.cpu.pc, pc_before,
                        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec,
                        vm.cpu.privilege);

                    // Dump the last 10 PCs from history
                    let start = pc_history.len().saturating_sub(10);
                    eprintln!("[diag] Last 10 PCs before fault:");
                    for i in start..pc_history.len() {
                        let (c, p) = pc_history[i];
                        eprintln!("  count={} PC=0x{:08X}", c, p);
                    }

                    // For fetch faults, check what the sepc instruction should be
                    if matches!(step_result, StepResult::FetchFault) {
                        // The sepc is the address the kernel tried to jump to
                        let sepc = vm.cpu.csr.sepc;
                        eprintln!("[diag] Fault target: sepc=0x{:08X} (expected 0xC04046C8 = alloc_pte_fixmap)", sepc);

                        // Check what's at the correct VA (if mapped)
                        let correct_va = 0xC04046C8u64;
                        let word = vm.bus.read_word(correct_va);
                        eprintln!("[diag] Word at VA 0xC04046C8 (PA 0x004046C8): {:?}", word);

                        // Check the L1 PTE that maps VA 0xC0400000 region
                        // This is after setup_vm with the final page table
                        let satp = vm.cpu.csr.satp;
                        let mode = (satp >> 31) & 1;
                        if mode == 1 {
                            let ppn = (satp & 0x3FFFFF) as u64;
                            let pg_dir_phys = ppn * 4096;
                            // VA 0xC04046C8: VPN1 = (0xC04046C8 >> 22) = 769
                            let vpn1 = (sepc >> 22) & 0x3FF;
                            let l1_addr = pg_dir_phys + (vpn1 as u64) * 4;
                            let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                            eprintln!("[diag] For sepc=0x{:08X}: VPN1={} L1[{}] at PA 0x{:08X}: pte=0x{:08X} ppn=0x{:X}",
                                sepc, vpn1, vpn1, l1_addr, l1_pte, l1_pte >> 10);

                            // Also check what the CORRECT L1 entry should be for the actual target
                            let correct_vpn1 = (0xC04046C8u32 >> 22) & 0x3FF;
                            let correct_l1_addr = pg_dir_phys + (correct_vpn1 as u64) * 4;
                            let correct_l1_pte = vm.bus.read_word(correct_l1_addr).unwrap_or(0);
                            eprintln!("[diag] For correct VA 0xC04046C8: VPN1={} L1[{}] at PA 0x{:08X}: pte=0x{:08X} ppn=0x{:X}",
                                correct_vpn1, correct_vpn1, correct_l1_addr, correct_l1_pte, correct_l1_pte >> 10);
                        }
                    }
                }
            }
            StepResult::Ebreak => {
                eprintln!("[diag] EBREAK at count={}", count);
                break;
            }
            _ => {}
        }

        // Record PC history (keep last 200)
        if pc_history.len() >= 200 {
            pc_history.remove(0);
        }
        pc_history.push((count, pc_before));
        count += 1;
    }

    eprintln!(
        "[diag] Done at count={}, smode_faults={}",
        count, smode_fault_count
    );
}
