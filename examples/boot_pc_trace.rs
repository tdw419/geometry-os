use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .expect("setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 10_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut trampoline_patched = true;
    let mut last_pc: u32 = 0;
    let mut same_pc: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut fault_count: u64 = 0;
    let mut last_sample_pc: u32 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Identity mapping injection on SATP change
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                eprintln!(
                    "[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                    last_satp, cur_satp, count
                );
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                    let already = (l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0;
                    if !already {
                        let identity_pte: u32 = 0x0000_00CF;
                        let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                        for &l1_idx in l1_entries {
                            let pte = identity_pte | (l1_idx << 20);
                            vm.bus
                                .write_word(pg_dir_phys + (l1_idx * 4) as u64, pte)
                                .ok();
                        }
                        vm.cpu.tlb.flush_all();
                        eprintln!("[boot] Injected into pg_dir 0x{:08X}", pg_dir_phys);
                    }
                }
            }
            last_satp = cur_satp;
        }

        // Trap handling
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M = SBI call
                sbi_count += 1;
                if sbi_count <= 5 {
                    eprintln!(
                        "[sbi] SBI call #{sbi_count}: a7=0x{:08X} a6=0x{:08X} a0=0x{:08X}",
                        vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
                    );
                }
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
                fault_count += 1;
                if fault_count <= 10 {
                    eprintln!("[trap] M-mode trap #{fault_count}: cause={cause_code} mepc=0x{:08X} mtval=0x{:08X}",
                        vm.cpu.csr.mepc, vm.cpu.csr.mtval);
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();
        if matches!(
            step_result,
            geometry_os::riscv::cpu::StepResult::FetchFault
                | geometry_os::riscv::cpu::StepResult::LoadFault
                | geometry_os::riscv::cpu::StepResult::StoreFault
        ) {
            fault_count += 1;
        }

        // Sample every 500K
        if count % 500_000 == 0 && count > 0 {
            eprintln!(
                "[sample {}] PC=0x{:08X} prev_pc=0x{:08X} sbi={} faults={}",
                count, vm.cpu.pc, last_sample_pc, sbi_count, fault_count
            );
            last_sample_pc = vm.cpu.pc;
        }

        // Spin detection
        if vm.cpu.pc == last_pc {
            same_pc += 1;
            if same_pc == 1000 {
                eprintln!("[spin] PC=0x{:08X} stuck at count={}", last_pc, count);
            }
        } else {
            last_pc = vm.cpu.pc;
            same_pc = 0;
        }

        count += 1;
    }

    eprintln!(
        "[done] count={} sbi={} faults={} uart={}",
        count,
        sbi_count,
        fault_count,
        vm.bus.uart.tx_buf.len()
    );
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("[UART] {}", &s[..s.len().min(500)]);
    }
    eprintln!(
        "[state] PC=0x{:08X} priv={:?} satp=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );
}
