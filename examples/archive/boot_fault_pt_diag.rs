//! Diagnostic: dump page table state at the fetch fault stall point.
//! Run: cargo run --example boot_fault_pt_diag

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

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle SATP changes like boot_linux
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                eprintln!(
                    "[diag] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                    last_satp, cur_satp, count
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

                    // Re-verify kernel_map
                    let km_phys: u64 = 0x00C79E90;
                    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                    eprintln!(
                        "[diag] kernel_map: pa=0x{:X} vapo=0x{:X} vkpo=0x{:X}",
                        km_pa, km_vapo, km_vkpo
                    );
                }
                last_satp = cur_satp;
            }
        }

        // Handle trap at firmware
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
                    let fault_type = match step_result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        StepResult::StoreFault => "store",
                        _ => unreachable!(),
                    };
                    eprintln!(
                        "[diag] S-mode {} fault #{} at count={}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} satp=0x{:08X}",
                        fault_type, smode_fault_count, count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec, vm.cpu.csr.satp
                    );

                    if smode_fault_count <= 3 {
                        let stval_copy = vm.cpu.csr.stval;
                        let stvec_copy = vm.cpu.csr.stvec;
                        dump_pt_for_va(&mut vm, stval_copy);
                        eprintln!("[diag] ---");
                        dump_pt_for_va(&mut vm, stvec_copy);

                        eprintln!(
                            "[diag] known_pt_pages count: {}",
                            vm.bus.known_pt_pages.len()
                        );
                    }

                    if smode_fault_count >= 5 {
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
    let uart_out = vm.bus.uart.drain_tx();
    eprintln!(
        "[diag] UART output ({} bytes): {:?}",
        uart_out.len(),
        String::from_utf8_lossy(&uart_out)
    );
}

fn dump_pt_for_va(vm: &mut RiscvVm, va: u32) {
    let satp = vm.cpu.csr.satp;
    let vpn1 = (va >> 22) & 0x3FF;
    let vpn0 = (va >> 12) & 0x3FF;
    let offset = va & 0xFFF;

    eprintln!(
        "[pt_dump] VA 0x{:08X}: VPN1={} VPN0={} offset={}",
        va, vpn1, vpn0, offset
    );

    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    let l1_addr = pg_dir_phys + (vpn1 as u64) * 4;

    match vm.bus.read_word(l1_addr) {
        Ok(l1_pte) => {
            let ppn1 = (l1_pte >> 10) & 0x3FFFFF;
            let flags = l1_pte & 0xFF;
            let is_leaf = (l1_pte & 0xE) != 0;
            eprintln!(
                "[pt_dump]   L1[{}] at PA 0x{:08X}: PTE=0x{:08X} PPN=0x{:X} flags=0x{:02X} is_leaf={}",
                vpn1, l1_addr, l1_pte, ppn1, flags, is_leaf
            );

            if !is_leaf && (l1_pte & 1) != 0 {
                let l2_base = (ppn1 as u64) * 4096;
                let l2_addr = l2_base + (vpn0 as u64) * 4;
                match vm.bus.read_word(l2_addr) {
                    Ok(l2_pte) => {
                        let ppn0 = (l2_pte >> 10) & 0x3FFFFF;
                        let l2_flags = l2_pte & 0xFF;
                        let pa = ((ppn0 as u64) << 12) | (offset as u64);
                        eprintln!(
                            "[pt_dump]     L2[{}] at PA 0x{:08X}: PTE=0x{:08X} PPN=0x{:X} flags=0x{:02X} -> PA 0x{:08X}",
                            vpn0, l2_addr, l2_pte, ppn0, l2_flags, pa
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "[pt_dump]     L2[{}] at PA 0x{:08X}: READ ERROR {:?}",
                            vpn0, l2_addr, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            eprintln!(
                "[pt_dump]   L1[{}] at PA 0x{:08X}: READ ERROR {:?}",
                vpn1, l1_addr, e
            );
        }
    }

    // Check if the L2 page is in known_pt_pages
    if let Ok(l1_pte) = vm.bus.read_word(pg_dir_phys + (vpn1 as u64) * 4) {
        if (l1_pte & 1) != 0 && (l1_pte & 0xE) == 0 {
            let l2_base = (((l1_pte >> 10) & 0x3FFFFF) as u64) * 4096;
            let is_known = vm.bus.known_pt_pages.contains(&l2_base);
            eprintln!(
                "[pt_dump]   L2 page at PA 0x{:08X} known_pt_pages={}",
                l2_base, is_known
            );
        }
    }
}
