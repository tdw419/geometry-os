use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Test 1: Disable ALL fixups -- just boot with kernel_map patch only
    eprintln!("=== TEST 1: No fixups, just kernel_map patch ===");
    let (mut vm1, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    // Disable auto_pte_fixup
    vm1.bus.auto_pte_fixup = false;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm1.cpu.csr.satp;
    let mut sbi_count: u64 = 0;

    while count < 2_000_000 {
        if vm1.bus.sbi.shutdown_requested { break; }

        // Detect trap at fw_addr
        if vm1.cpu.pc == fw_addr_u32 && vm1.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm1.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 { // ECALL_S -> SBI call
                sbi_count += 1;
                let result = vm1.bus.sbi.handle_ecall(
                    vm1.cpu.x[17], vm1.cpu.x[16],
                    vm1.cpu.x[10], vm1.cpu.x[11],
                    vm1.cpu.x[12], vm1.cpu.x[13],
                    vm1.cpu.x[14], vm1.cpu.x[15],
                    &mut vm1.bus.uart, &mut vm1.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm1.cpu.x[10] = a0;
                    vm1.cpu.x[11] = a1;
                }
            } else if cause_code != 11 { // Not ECALL_M
                let mpp = (vm1.cpu.csr.mstatus >> 11) & 3;
                if cause_code == 9 {
                    // ECALL_S handled above
                } else if mpp != 3 {
                    // Forward to S-mode
                    let stvec = vm1.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm1.cpu.csr.sepc = vm1.cpu.csr.mepc;
                        vm1.cpu.csr.scause = mcause;
                        vm1.cpu.csr.stval = vm1.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm1.cpu.csr.mstatus = (vm1.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm1.cpu.csr.mstatus >> 1) & 1;
                        vm1.cpu.csr.mstatus = (vm1.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm1.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 { // Timer
                            vm1.bus.clint.mtimecmp = vm1.bus.clint.mtime + 100_000;
                        }
                        vm1.cpu.pc = stvec;
                        vm1.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm1.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm1.cpu.csr.mepc = vm1.cpu.csr.mepc.wrapping_add(4);
        }

        vm1.bus.tick_clint();
        vm1.bus.sync_mip(&mut vm1.cpu.csr.mip);

        let step_result = vm1.step();

        match step_result {
            geometry_os::riscv::cpu::StepResult::Ebreak => break,
            geometry_os::riscv::cpu::StepResult::FetchFault
            | geometry_os::riscv::cpu::StepResult::LoadFault
            | geometry_os::riscv::cpu::StepResult::StoreFault => {
                if vm1.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor && count < 1_000_000 {
                    let ft = match step_result {
                        geometry_os::riscv::cpu::StepResult::FetchFault => "fetch",
                        geometry_os::riscv::cpu::StepResult::LoadFault => "load",
                        _ => "store",
                    };
                    eprintln!("[test1] S-mode {} fault at count={}: PC=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                        ft, count, vm1.cpu.pc, vm1.cpu.csr.scause, vm1.cpu.csr.stval);
                }
            }
            _ => {}
        }

        // Log SATP changes
        let cur_satp = vm1.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[test1] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            // Dump first few L1 entries at the new page directory
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir = (ppn as u64) * 4096;
            for i in 768..780u32 {
                let addr = pg_dir + (i as u64) * 4;
                let entry = vm1.bus.read_word(addr).unwrap_or(0);
                let v = (entry & 1) != 0;
                let r = (entry & 2) != 0;
                let w = (entry & 4) != 0;
                let x = (entry & 8) != 0;
                let ppn_val = (entry >> 10) & 0x3FFFFF;
                eprintln!("[test1]   L1[{}] = 0x{:08X} V={} R={} W={} X={} PPN=0x{:06X}",
                    i, entry, v, r, w, x, ppn_val);
            }
            last_satp = cur_satp;
        }

        count += 1;
    }

    let sbi_str: String = vm1.bus.sbi.console_output.iter().map(|&b| b as char).collect();
    eprintln!("[test1] Done: count={} SBI_calls={} SBI_output={} bytes", count, sbi_count, sbi_str.len());
    if !sbi_str.is_empty() {
        eprintln!("[test1] SBI output:\n{}", sbi_str);
    }
}
