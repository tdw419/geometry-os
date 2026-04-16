/// Run boot for 5M instructions, logging significant events.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 50_000_000u64;
    let mut count: u64 = 0;
    let mut trampoline_patched = false;
    let mut sbi_count: u64 = 0;
    let mut ecall_s_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut smode_faults: u64 = 0;
    let mut last_log_count: u64 = 0;
    let mut last_pc: u32 = 0;
    let mut spin_count: u64 = 0;
    let mut uart_bytes: u64 = 0;
    let mut trap_counts: [u64; 32] = [0; 32]; // cause code histogram

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            // Full identity mapping for all kernel physical memory segments.
            // Kernel has LOAD segments at PA 0x0, 0x00400000, 0x00800000, 0x00C00000,
            // 0x01000000, 0x01400000. Each needs an identity megapage PTE.
            let identity_pte_base: u32 = 0x0000_00EF; // V=1,R=1,W=1,X=1,A=1,D=1
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10]; // kernel physical regions
            for &l1_idx in l1_entries {
                let pte = identity_pte_base | ((l1_idx as u32) << 20);
                let off = (l1_idx * 4) as u64;
                vm.bus.write_word(0x0148_4000u64 + off, pte).ok(); // trampoline_pg_dir
                vm.bus.write_word(0x0080_2000u64 + off, pte).ok(); // early_pg_dir
            }
            trampoline_patched = true;
            eprintln!("[{}] TRAMPOLINE PATCHED (L1[{:?}])", count, l1_entries);
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if (cause_code as usize) < 32 {
                    trap_counts[cause_code as usize] += 1;
                }
                // Log first 10 illegal instruction faulting PCs
                if cause_code == 2
                    && (trap_counts[2] as usize) <= 10
                {
                    eprintln!(
                        "[{}] ILLEGAL INST: mepc=0x{:08X} inst=0x{:08X} priv={:?} mpp={}",
                        count,
                        vm.cpu.csr.mepc,
                        vm.bus
                            .read_word(vm.cpu.csr.mepc as u64)
                            .unwrap_or(0),
                        vm.cpu.privilege,
                        (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK)
                            >> csr::MSTATUS_MPP_LSB
                    );
                }
                if cause_code == csr::CAUSE_ECALL_S {
                    ecall_s_count += 1;
                    let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if mpp != 3 {
                    forward_count += 1;
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
            } else {
                sbi_count += 1;
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            count += 1;
            continue;
        }

        let prev_uart = vm.bus.sbi.console_output.len() as u64;
        let result = vm.step();
        let new_uart = vm.bus.sbi.console_output.len() as u64;
        if new_uart > prev_uart {
            uart_bytes += new_uart - prev_uart;
            if uart_bytes <= 500 || new_uart % 100 < 5 {
                let s = String::from_utf8_lossy(&vm.bus.sbi.console_output[prev_uart as usize..new_uart as usize]);
                eprintln!("[{}] UART: {:?}", count, s);
            }
        }

        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_faults += 1;
                    if smode_faults <= 20 {
                        let ft = match result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => "",
                        };
                        eprintln!("[{}] S-mode {} fault: sepc=0x{:08X} stval=0x{:08X}", count, ft, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                    }
                }
            }
            StepResult::Ebreak => {
                eprintln!("[{}] EBREAK", count);
                break;
            }
            _ => {}
        }

        // Spin detection
        if vm.cpu.pc == last_pc {
            spin_count += 1;
        } else {
            if spin_count > 1000 {
                eprintln!("[{}] Spin ended at PC=0x{:08X} after {} spins", count, last_pc, spin_count);
            }
            spin_count = 0;
            last_pc = vm.cpu.pc;
        }

        // Periodic status
        if count - last_log_count >= 500_000 {
            eprintln!("[{}] PC=0x{:08X} priv={:?} sbi={} ecall_s={} fwd={} faults={} uart={}",
                count, vm.cpu.pc, vm.cpu.privilege, sbi_count, ecall_s_count, forward_count, smode_faults, uart_bytes);
            last_log_count = count;
        }

        count += 1;
    }

    eprintln!("\n=== FINAL ===");
    eprintln!("Count={}", count);
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SP=0x{:08X} RA=0x{:08X} GP=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3]);
    eprintln!("satp=0x{:08X}", vm.cpu.csr.satp);
    eprintln!("SBI_calls={} ECALL_S={} forwards={} smode_faults={}", sbi_count, ecall_s_count, forward_count, smode_faults);
    eprintln!("UART bytes={}", uart_bytes);
    // Print trap cause histogram
    let cause_names = ["misaligned_fetch", "fetch_access", "illegal_inst", "breakpoint",
        "misaligned_load", "load_access", "misaligned_store", "store_access",
        "ecall_U", "ecall_S", "", "ecall_M", "inst_page_fault", "load_page_fault", "store_page_fault"];
    for (i, &c) in trap_counts.iter().enumerate() {
        if c > 0 {
            let name = cause_names.get(i).unwrap_or(&"unknown");
            eprintln!("  trap cause {} ({:?}): {}", i, name, c);
        }
    }
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("UART output:\n{}", s);
    }
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("Direct UART TX:\n{}", s);
    }
}
