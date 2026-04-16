/// Diagnostic: Log the first 50 S-mode faults in detail.
/// Also track when/how many faults happen.

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

    let max = 30_000_000u64;
    let mut count: u64 = 0;
    let mut fault_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown requested at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 11 {
                // ECALL_M -> SBI
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                println!("[diag] SBI ecall_m at count={} mepc=0x{:08X}", count, vm.cpu.csr.mepc);
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

        // SATP change detection + fixup (matching boot_linux)
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            println!("[diag] SATP change: 0x{:08X} -> 0x{:08X} at count={} PC=0x{:08X}",
                last_satp, cur_satp, count, vm.cpu.pc);

            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;

                // Fix virtual PPNs in the kernel's page table
                // (same fixup as boot_linux)
                const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12;
                const PPN_MASK: u32 = 0xFFFF_FC00;
                const PTE_V: u32 = 1;
                const LEAF_FLAGS: u32 = 2 | 4 | 8; // R|W|X

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
                        // Fix L2 tables for non-leaf entries
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
                let l1_entries_to_map: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
                    16, 32, 48, 64, 80, 96, 112, 127];
                let l1_0_val = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                let already_patched = (l1_0_val & 0xCF) == 0xCF && ((l1_0_val >> 20) & 0xFFF) == 0;
                if !already_patched {
                    for &l1_idx in l1_entries_to_map {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(pg_dir_phys + (l1_idx * 4) as u64, pte).ok();
                    }
                    // Kernel VA range: L1[776] -> PA 0x200000
                    let existing = vm.bus.read_word(pg_dir_phys + 776 * 4).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(pg_dir_phys + 776 * 4, identity_pte | (8u32 << 20)).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    println!("[diag] Injected identity mappings at pg_dir PA 0x{:08X}", pg_dir_phys);
                }
            }
            last_satp = cur_satp;
        }

        let step_result = vm.step();
        match step_result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                fault_count += 1;
                if fault_count <= 50 {
                    let fault_type = match step_result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        StepResult::StoreFault => "store",
                        _ => unreachable!(),
                    };
                    let scause = vm.cpu.csr.scause;
                    let stval = vm.cpu.csr.stval;
                    let sepc = vm.cpu.csr.sepc;
                    let pc = vm.cpu.pc;
                    eprintln!("[diag] S-mode {} fault #{} at count={}: PC=0x{:08X} scause=0x{:X} sepc=0x{:08X} stval=0x{:08X}",
                        fault_type, fault_count, count, pc, scause, sepc, stval);
                }
            }
            StepResult::Ebreak => {
                println!("[diag] EBREAK at count={} PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ecall => {}
        }

        count += 1;

        if count % 5_000_000 == 0 {
            println!("[diag] count={} PC=0x{:08X} ecall={} faults={} sbi_console={} uart={}",
                count, vm.cpu.pc, vm.cpu.ecall_count, fault_count,
                vm.bus.sbi.console_output.len(), vm.bus.uart.tx_buf.len());
        }
    }

    println!("\n[diag] Final: count={} PC=0x{:08X} ecall={} faults={}",
        count, vm.cpu.pc, vm.cpu.ecall_count, fault_count);
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("[diag] SBI output:\n{}", s);
    }
}
