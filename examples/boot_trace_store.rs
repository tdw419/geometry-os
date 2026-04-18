use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut fault_log_count: u64 = 0;
    let max_faults: u64 = 50;

    while count < 500_000 {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[boot] SBI shutdown requested at count={}", count);
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                sbi_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Check for SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        match step_result {
            StepResult::Ebreak => {
                eprintln!("[boot] EBREAK at count={}", count);
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && fault_log_count < max_faults {
                    fault_log_count += 1;
                    let ft = match step_result {
                        StepResult::FetchFault => "FETCH",
                        StepResult::LoadFault => "LOAD",
                        _ => "STORE",
                    };
                    eprintln!("[fault] #{} at count={}: {} PC=0x{:08X} scause=0x{:02X} stval=0x{:08X} sepc=0x{:08X}",
                        fault_log_count, count, ft, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.sepc);
                    
                    // Check page table for the faulting VA
                    let fault_va = vm.cpu.csr.stval;
                    let l1_idx = (fault_va >> 22) & 0x3FF;
                    let satp = vm.cpu.csr.satp;
                    let pg_dir_ppn = satp & 0x3FFFFF;
                    let pg_dir_phys = (pg_dir_ppn as u64) * 4096;
                    let l1_entry = vm.bus.read_word(pg_dir_phys + (l1_idx as u64) * 4).unwrap_or(0);
                    eprintln!("  stval L1[{}] = 0x{:08X}", l1_idx, l1_entry);
                    
                    // Also check stvec mapping  
                    let stvec = vm.cpu.csr.stvec;
                    let stvec_l1 = (stvec >> 22) & 0x3FF;
                    let stvec_l1_entry = vm.bus.read_word(pg_dir_phys + (stvec_l1 as u64) * 4).unwrap_or(0);
                    let is_leaf = (stvec_l1_entry & 0xE) != 0;
                    eprintln!("  stvec=0x{:08X} L1[{}] = 0x{:08X} leaf={}", stvec, stvec_l1, stvec_l1_entry, is_leaf);
                    if !is_leaf && (stvec_l1_entry & 1) != 0 {
                        // Non-leaf, check L2
                        let l2_ppn = (stvec_l1_entry >> 10) & 0x3FFFFF;
                        let l2_idx = (stvec >> 12) & 0x3FF;
                        let l2_entry = vm.bus.read_word((l2_ppn as u64) * 4096 + (l2_idx as u64) * 4).unwrap_or(0);
                        eprintln!("  stvec L2[{}] at PA 0x{:08X} = 0x{:08X}", l2_idx, l2_ppn * 4096, l2_entry);
                    }
                }
            }
            _ => {}
        }
        count += 1;
    }
    eprintln!("[boot] Done: count={} SBI_calls={}", count, sbi_count);
    
    // Check UART output
    let uart_buf = &vm.bus.uart.tx_buf;
    if !uart_buf.is_empty() {
        let text: String = uart_buf.iter().filter(|&&c| c >= 0x20 && c < 0x7F).map(|&c| c as char).collect();
        eprintln!("[boot] UART output ({} bytes): {}", uart_buf.len(), &text[..text.len().min(500)]);
    } else {
        eprintln!("[boot] No UART output");
    }
}
