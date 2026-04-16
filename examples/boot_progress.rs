/// Progressive boot diagnostic: trace key transitions during Linux boot.
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, entry_phys, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    eprintln!("[setup] entry_phys=0x{:08X} dtb_addr=0x{:08X} fw_addr=0x{:08X}", entry_phys, dtb_addr, fw_addr);
    eprintln!("[setup] PC=0x{:08X} priv={:?} satp=0x{:08X}", vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);

    let max_count = 20_000_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut smode_faults: u64 = 0;
    let mut uart_bytes: u64 = 0;
    let mut last_log: u64 = 0;
    let mut pc_transitions: Vec<(u64, u32, u32)> = Vec::new(); // (count, old_pc, new_pc)
    let mut last_pc: u32 = vm.cpu.pc;
    let mut phase_log: Vec<String> = Vec::new();

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})", count, last_satp, cur_satp, vm.cpu.pc);
            last_satp = cur_satp;

            // Inject identity mappings on SATP change
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127];
                let l1_0_val = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                let already_patched = (l1_0_val & 0xCF) == 0xCF && ((l1_0_val >> 20) & 0xFFF) == 0;
                if !already_patched {
                    for &l1_idx in l1_entries {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(pg_dir_phys + (l1_idx * 4) as u64, pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    eprintln!("[{}] Injected identity mappings at pg_dir PA 0x{:08X}", count, pg_dir_phys);
                }
            }
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == csr::CAUSE_ECALL_M || cause_code == csr::CAUSE_ECALL_S {
                // SBI call
                sbi_count += 1;
                let r = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                if sbi_count <= 20 {
                    let ext = vm.cpu.x[17];
                    let fn_id = vm.cpu.x[16];
                    eprintln!("[{}] SBI call #{}: ext=0x{:02X} fn={} a0=0x{:08X}", count, sbi_count, ext, fn_id, vm.cpu.x[10]);
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 && cause_code != 0 {
                    // Forward to S-mode
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        if forward_count <= 20 {
                            eprintln!("[{}] Forward to S-mode: cause={} mepc=0x{:08X} stvec=0x{:08X}",
                                count, cause_code, vm.cpu.csr.sepc, stvec);
                        }
                        count += 1;
                        continue;
                    }
                }
                // Skip faulting instruction
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
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output[prev_uart as usize..new_uart as usize]);
            eprintln!("[{}] UART: {:?}", count, s);
        }

        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_faults += 1;
                    if smode_faults <= 30 {
                        let ft = match result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => "",
                        };
                        eprintln!("[{}] S-mode {} fault: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                            count, ft, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                    }
                }
            }
            StepResult::Ebreak => {
                eprintln!("[{}] EBREAK", count);
                break;
            }
            _ => {}
        }

        // Track major PC transitions
        if vm.cpu.pc != last_pc {
            // Only log when transitioning to/from different address ranges
            let old_range = last_pc >> 24;
            let new_range = vm.cpu.pc >> 24;
            if old_range != new_range && pc_transitions.len() < 50 {
                pc_transitions.push((count, last_pc, vm.cpu.pc));
            }
            last_pc = vm.cpu.pc;
        }

        // Periodic status
        if count - last_log >= 1_000_000 {
            eprintln!("[{}] PC=0x{:08X} priv={:?} satp=0x{:08X} sbi={} fwd={} faults={} uart={}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp,
                sbi_count, forward_count, smode_faults, uart_bytes);
            last_log = count;
        }

        count += 1;
    }

    eprintln!("\n=== SUMMARY ===");
    eprintln!("Instructions: {}", count);
    eprintln!("Final PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("satp=0x{:08X} stvec=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
    eprintln!("SP=0x{:08X} RA=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1]);
    eprintln!("SBI calls={} forwards={} smode_faults={}", sbi_count, forward_count, smode_faults);
    eprintln!("UART bytes={}", uart_bytes);

    if !pc_transitions.is_empty() {
        eprintln!("\n=== PC RANGE TRANSITIONS ===");
        for (c, old, new) in &pc_transitions {
            eprintln!("[{}] 0x{:08X} -> 0x{:08X}", c, old, new);
        }
    }

    // UART output
    let sbi_out = &vm.bus.sbi.console_output;
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        eprintln!("\n=== SBI CONSOLE ===\n{}", s);
    }
}
