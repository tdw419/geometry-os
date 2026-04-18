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

    // Disable auto_pte_fixup
    vm.bus.auto_pte_fixup = false;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut smode_faults: u64 = 0;

    while count < 5_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                // ECALL_S -> SBI call
                sbi_count += 1;
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
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
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
                        if cause_code == 7 {
                            vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                        }
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

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        match step_result {
            geometry_os::riscv::cpu::StepResult::Ebreak => break,
            geometry_os::riscv::cpu::StepResult::FetchFault
            | geometry_os::riscv::cpu::StepResult::LoadFault
            | geometry_os::riscv::cpu::StepResult::StoreFault => {
                if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
                    smode_faults += 1;
                    if smode_faults <= 5 {
                        let ft = match step_result {
                            geometry_os::riscv::cpu::StepResult::FetchFault => "fetch",
                            geometry_os::riscv::cpu::StepResult::LoadFault => "load",
                            _ => "store",
                        };
                        eprintln!("[boot] S-mode {} fault #{} at count={}: PC=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                            ft, smode_faults, count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                    }
                }
            }
            _ => {}
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );

            // Only inject identity mappings for device regions, NOT kernel megapages
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            let identity_pte: u32 = 0x0000_00CF;

            // Map a large identity range: L1[0..64] covers 0x0-0x0FFFFFFF (256MB)
            for i in 0..64u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    let pte = identity_pte | (i << 20);
                    vm.bus.write_word(addr, pte).ok();
                }
            }
            // CLINT, PLIC, UART
            for &l1_idx in &[8u32, 48, 64] {
                let addr = pg_dir_phys + (l1_idx as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    let pte = identity_pte | (l1_idx << 20);
                    vm.bus.write_word(addr, pte).ok();
                }
            }

            vm.cpu.tlb.flush_all();
            eprintln!("[boot] Injected identity mappings only (no kernel PT fixups)");

            // Verify kernel_map is still correct
            let km_phys: u64 = 0x00C79E90;
            let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
            eprintln!(
                "[boot] kernel_map check: pa=0x{:X} vapo=0x{:X} vkpo=0x{:X}",
                km_pa, km_vapo, km_vkpo
            );
            if km_pa != 0 || km_vapo != 0xC0000000 || km_vkpo != 0 {
                eprintln!("[boot] WARNING: re-patching kernel_map");
                vm.bus.write_word(km_phys + 12, 0).ok();
                vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                vm.bus.write_word(km_phys + 24, 0).ok();
            }

            last_satp = cur_satp;
        }

        count += 1;
    }

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "\n[boot] Done: count={} SBI_calls={} faults={} SBI_output={} bytes",
        count,
        sbi_count,
        smode_faults,
        sbi_str.len()
    );
    if !sbi_str.is_empty() {
        eprintln!("[boot] SBI output (first 2000 chars):");
        let preview: String = sbi_str.chars().take(2000).collect();
        eprintln!("{}", preview);
    }
}
