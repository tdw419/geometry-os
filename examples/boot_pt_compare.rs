//! Diagnostic: compare early_pg_dir vs swapper_pg_dir L1 entries.
//! Run: cargo run --example boot_pt_compare

use geometry_os::riscv::{cpu::Privilege, cpu::StepResult, RiscvVm};
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel not found");
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi earlyprintk loglevel=7";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut count: u64 = 0;
    let max_instructions: u64 = 800_000;
    let mut smode_fault_count: u64 = 0;
    // Track page directories we've seen
    let mut pg_dirs_seen: Vec<u64> = Vec::new();

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
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

                    pg_dirs_seen.push(pg_dir_phys);
                    eprintln!(
                        "[diag] SATP changed to pg_dir PA 0x{:08X} at count={}",
                        pg_dir_phys, count
                    );

                    // Dump kernel mapping L1 entries (768+) for this page directory
                    dump_kernel_l1(&mut vm, pg_dir_phys);
                }
                last_satp = cur_satp;
            }
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

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
            } else if mpp != 3 {
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
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        match step_result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_fault_count += 1;
                    if smode_fault_count == 1 {
                        eprintln!("[diag] First S-mode fault at count={}", count);
                        // Dump all known page directories' kernel mappings
                        for &pg_dir in &pg_dirs_seen {
                            eprintln!(
                                "\n[diag] === Page directory at PA 0x{:08X} (AFTER boot) ===",
                                pg_dir
                            );
                            dump_kernel_l1(&mut vm, pg_dir);
                        }
                    }
                    if smode_fault_count >= 3 {
                        break;
                    }
                }
            }
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
        }
        count += 1;
    }
    eprintln!("[diag] Total instructions: {}", count);
}

fn dump_kernel_l1(vm: &mut RiscvVm, pg_dir_phys: u64) {
    // Dump L1 entries 768+ (kernel VA 0xC0000000+)
    eprintln!("[pt] Kernel mapping entries (L1[768+] = VA 0xC0000000+):");
    for i in 768..1024 {
        let addr = pg_dir_phys + (i as u64) * 4;
        if let Ok(pte) = vm.bus.read_word(addr) {
            if pte != 0 {
                let ppn = (pte >> 10) & 0x3FFFFF;
                let flags = pte & 0xFF;
                let is_leaf = (pte & 0xE) != 0;
                if is_leaf {
                    eprintln!(
                        "  L1[{:4}] = 0x{:08X} MEGAPAGE PPN=0x{:06X} flags=0x{:02X} PA=0x{:08X}",
                        i,
                        pte,
                        ppn,
                        flags,
                        ppn << 22
                    );
                } else {
                    let l2_pa = (ppn as u64) << 12;
                    eprintln!(
                        "  L1[{:4}] = 0x{:08X} L2@PA=0x{:08X} flags=0x{:02X}",
                        i, pte, l2_pa, flags
                    );
                    // Dump first few non-zero L2 entries
                    let mut l2_count = 0u32;
                    for j in 0..512 {
                        let l2_addr = l2_pa + (j as u64) * 4;
                        if let Ok(l2_pte) = vm.bus.read_word(l2_addr) {
                            if l2_pte != 0 && l2_count < 5 {
                                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                                let l2_flags = l2_pte & 0xFF;
                                let pa = ((l2_ppn as u64) << 12) + (j as u64) * 0x1000;
                                let va = (i as u64) * 0x200000 + (j as u64) * 0x1000;
                                eprintln!(
                        "    L2[{:4}] = 0x{:08X} PPN=0x{:06X} flags=0x{:02X} VA=0x{:08X} -> PA=0x{:08X}",
                                    j, l2_pte, l2_ppn, l2_flags, va, pa
                                );
                                l2_count += 1;
                            }
                        }
                    }
                    if l2_count > 0 {
                        eprintln!(
                            "    ... (showed {} of possibly more non-zero entries)",
                            l2_count
                        );
                    } else {
                        eprintln!("    (all L2 entries are zero)");
                    }
                }
            }
        }
    }
}
