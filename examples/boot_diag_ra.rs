use geometry_os::riscv::cpu::StepResult;
/// Diagnostic: trace exactly when RA changes to 0x3FFFF000
/// and what instruction causes it.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio,0x10000000 panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_ra: u32 = vm.cpu.x[1];
    let mut last_sp: u32 = vm.cpu.x[2];
    let mut ra_traced = false;
    let mut smode_fault_count: u32 = 0;

    let max_instructions: u64 = 200_000;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // SATP change handling (same as boot_linux)
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_0_val = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                    let already_patched =
                        (l1_0_val & 0xCF) == 0xCF && ((l1_0_val >> 20) & 0xFFF) == 0;
                    if !already_patched {
                        let l1_entries: &[u32] = &[
                            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127,
                        ];
                        for &l1_idx in l1_entries {
                            let pte = 0xCF | (l1_idx << 20);
                            vm.bus
                                .write_word(pg_dir_phys + (l1_idx * 4) as u64, pte)
                                .ok();
                        }
                        vm.cpu.tlb.flush_all();
                        eprintln!("[diag] SATP changed to 0x{:08X}, injected identity mappings at PA 0x{:08X}", cur_satp, pg_dir_phys);
                    }
                }
                last_satp = cur_satp;
            }
        }

        // M-mode trap handler (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                // ECALL_S -> SBI call
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if cause_code == 8 && mpp != 3 {
                    // ECALL_S forwarded
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();
        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
                    smode_fault_count += 1;
                    if smode_fault_count <= 3 {
                        let fault_type = match step_result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => "",
                        };
                        eprintln!("[diag] S-mode {} fault #{} at count={}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} RA=0x{:08X} SP=0x{:08X}",
                            fault_type, smode_fault_count, count, vm.cpu.pc,
                            vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec,
                            vm.cpu.x[1], vm.cpu.x[2]);
                        // Dump the page table for the faulting VA
                        let fault_va = vm.cpu.csr.stval;
                        let satp = vm.cpu.csr.satp;
                        let ppn = satp & 0x3FFFFF;
                        let pg_dir_phys = (ppn as u64) * 4096;
                        let vpn1 = ((fault_va >> 22) & 0x3FF) as u64;
                        let vpn0 = ((fault_va >> 12) & 0x3FF) as u64;
                        let l1_pte = vm.bus.read_word(pg_dir_phys + vpn1 * 4).unwrap_or(0);
                        eprintln!(
                            "[diag]   fault VA=0x{:08X} VPN1={} VPN0={} SATP=0x{:08X}",
                            fault_va, vpn1, vpn0, satp
                        );
                        eprintln!(
                            "[diag]   pg_dir_phys=0x{:08X} L1[{}]=0x{:08X}",
                            pg_dir_phys, vpn1, l1_pte
                        );
                        if (l1_pte & (1 << 0)) != 0 && (l1_pte & 0xE) != 0 {
                            // megapage
                            let l1_ppn = (l1_pte & 0xFFFFFC00) >> 10;
                            eprintln!(
                                "[diag]   megapage PPN=0x{:08X} (phys would be 0x{:08X})",
                                l1_ppn,
                                l1_ppn << 12
                            );
                        } else if (l1_pte & (1 << 0)) != 0 {
                            // L2 pointer
                            let l1_ppn = (l1_pte & 0xFFFFFC00) >> 10;
                            let l2_addr = (l1_ppn as u64) << 12;
                            let l2_pte = vm.bus.read_word(l2_addr + vpn0 * 4).unwrap_or(0);
                            eprintln!(
                                "[diag]   L2 base PA=0x{:08X} L2[{}]=0x{:08X}",
                                l2_addr, vpn0, l2_pte
                            );
                        }
                    }
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }

        // Check if RA changed
        let cur_ra = vm.cpu.x[1];
        if cur_ra != last_ra {
            if !ra_traced && cur_ra == 0x3FFFF000 {
                ra_traced = true;
                eprintln!(
                    "[diag] *** RA changed to 0x{:08X} at count={} ***",
                    cur_ra, count
                );
                eprintln!("[diag]   previous RA was 0x{:08X}", last_ra);
                eprintln!("[diag]   PC=0x{:08X} SP=0x{:08X}", vm.cpu.pc, vm.cpu.x[2]);
                // Disassemble the instruction at PC
                let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
                eprintln!("[diag]   instruction at PC: 0x{:08X}", inst);
            }
            last_ra = cur_ra;
        }

        // Check if SP changed drastically
        let cur_sp = vm.cpu.x[2];
        if (cur_sp as i32 - last_sp as i32).abs() > 0x10000 {
            eprintln!(
                "[diag] SP jumped: 0x{:08X} -> 0x{:08X} at count={}, PC=0x{:08X}",
                last_sp, cur_sp, count, vm.cpu.pc
            );
            last_sp = cur_sp;
        } else {
            last_sp = cur_sp;
        }

        count += 1;
    }

    if !ra_traced {
        eprintln!(
            "[diag] RA never became 0x3FFFF000 within {} instructions",
            max_instructions
        );
        eprintln!(
            "[diag] Final: PC=0x{:08X} RA=0x{:08X} SP=0x{:08X} SATP=0x{:08X} smode_faults={}",
            vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2], vm.cpu.csr.satp, smode_fault_count
        );
    }
}
