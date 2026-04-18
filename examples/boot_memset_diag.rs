use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count: u64 = 300_000;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;

    // Watch for the first time we hit the memset loop
    let mut hit_memset = false;
    let mut memset_start_count = 0u64;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            if cause_code == 9 {
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
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 1)) | 0;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Inject identity mappings on SATP change
        if vm.cpu.csr.satp != last_satp {
            let cur_satp = vm.cpu.csr.satp;
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
            }
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Detect first entry to memset loop at 0x10B2
        if !hit_memset && vm.cpu.pc == 0x000010B2 {
            hit_memset = true;
            memset_start_count = count;
            eprintln!(
                "[MEMSET] First entry at count={}, PC=0x{:08X}",
                count, vm.cpu.pc
            );
            eprintln!("  x13 (start) = 0x{:08X} ({})", vm.cpu.x[13], vm.cpu.x[13]);
            eprintln!("  x14 (end)   = 0x{:08X} ({})", vm.cpu.x[14], vm.cpu.x[14]);
            eprintln!(
                "  Iterations  = {}",
                (vm.cpu.x[14].wrapping_sub(vm.cpu.x[13])) / 4
            );
            eprintln!("  RA  = 0x{:08X}", vm.cpu.x[1]);
            eprintln!("  SP  = 0x{:08X}", vm.cpu.x[2]);
            eprintln!("  SATP= 0x{:08X}", vm.cpu.csr.satp);
            eprintln!("  Priv= {:?}", vm.cpu.privilege);

            // Also check what's around x13/x14 - what memory range is being cleared?
            let start = vm.cpu.x[13];
            let end = vm.cpu.x[14];
            eprintln!(
                "  Clearing range: 0x{:08X} - 0x{:08X} ({} bytes)",
                start,
                end,
                end.wrapping_sub(start)
            );
        }

        let _ = vm.step();
        count += 1;

        if count % 100_000 == 0 {
            eprintln!("Progress: count={} PC=0x{:08X}", count, vm.cpu.pc);
        }
    }

    if hit_memset {
        let memset_iters = count - memset_start_count;
        eprintln!(
            "[MEMSET] Ran for {} iterations ({} bytes cleared)",
            memset_iters,
            memset_iters * 4
        );
    }
    println!(
        "Done: count={} PC=0x{:08X} UART={}",
        count,
        vm.cpu.pc,
        vm.bus.uart.tx_buf.len()
    );
}
