// Capture the FIRST page fault that leads to "kill idle task" panic.
// We run boot_linux_setup manually so we can intercept at the M-mode level.
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
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 20_000_000;
    let mut first_fault_logged = false;
    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;
    let mut last_satp = vm.cpu.csr.satp;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        let _ = vm.step();
        count += 1;

        // Track SATP changes (to see when linear mapping is set up)
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_pa = (ppn as u64) * 4096;
            eprintln!(
                "[SATP] 0x{:08X} (pg_dir PA 0x{:08X}) at count={}",
                cur_satp, pg_dir_pa, count
            );
            last_satp = cur_satp;
        }

        // Catch the first trap at the firmware handler that comes from S-mode
        if !first_fault_logged && vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
            let mtval = vm.cpu.csr.mtval;

            // Only care about page faults (12, 13, 15) and access faults (5, 7) from S-mode
            if mpp == 1
                && (cause_code == 5
                    || cause_code == 7
                    || cause_code == 12
                    || cause_code == 13
                    || cause_code == 15)
            {
                first_fault_logged = true;
                let cause_name = match cause_code {
                    5 => "LOAD_ACCESS",
                    7 => "STORE_ACCESS",
                    12 => "FETCH_PAGE_FAULT",
                    13 => "LOAD_PAGE_FAULT",
                    15 => "STORE_PAGE_FAULT",
                    _ => "???",
                };
                eprintln!("\n=== FIRST S-MODE FAULT at count={} ===", count);
                eprintln!(
                    "  cause={} ({}), mtval=0x{:08X}, mepc=0x{:08X}",
                    cause_code, cause_name, mtval, vm.cpu.csr.mepc
                );
                eprintln!("  satp=0x{:08X}", cur_satp);

                // Page table walk for mtval
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_pa = (ppn as u64) * 4096;
                let va = mtval as u32;
                let vpn1 = ((va >> 22) & 0x3FF) as u64;
                let vpn0 = ((va >> 12) & 0x3FF) as u64;
                let l1_addr = pg_dir_pa + vpn1 * 4;
                let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                eprintln!(
                    "  PT walk: L1[{}] at PA 0x{:08X} = 0x{:08X} (valid={} leaf={} ppn=0x{:06X})",
                    vpn1,
                    l1_addr,
                    l1_entry,
                    (l1_entry & 1) != 0,
                    (l1_entry & 0xE) != 0,
                    (l1_entry >> 10) & 0x3FFFFF
                );

                if (l1_entry & 1) != 0 && (l1_entry & 0xE) == 0 {
                    let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                    let l2_base = l2_ppn * 4096;
                    let l2_addr = l2_base + vpn0 * 4;
                    let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                    eprintln!(
                        "           L2[{}] at PA 0x{:08X} = 0x{:08X} (valid={} ppn=0x{:06X})",
                        vpn0,
                        l2_addr,
                        l2_entry,
                        (l2_entry & 1) != 0,
                        (l2_entry >> 10) & 0x3FFFFF
                    );
                }

                // Dump memblock state
                let memblock_pa: u64 = 0x00803448;
                let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
                let res_cnt = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
                eprintln!(
                    "  memblock: memory.cnt={}, reserved.cnt={}",
                    mem_cnt, res_cnt
                );

                // Dump memory regions (12-byte stride)
                let mem_regions_ptr = vm.bus.read_word(memblock_pa + 20).unwrap_or(0);
                if mem_regions_ptr >= 0xC0000000 {
                    let base_pa = (mem_regions_ptr - 0xC0000000) as u64;
                    for ri in 0..mem_cnt.min(8) {
                        let b = vm.bus.read_word(base_pa + (ri as u64) * 12).unwrap_or(0);
                        let s = vm
                            .bus
                            .read_word(base_pa + (ri as u64) * 12 + 4)
                            .unwrap_or(0);
                        eprintln!(
                            "    memory[{}]: base=0x{:08X} size=0x{:08X} (end=0x{:08X})",
                            ri,
                            b,
                            s,
                            b + s
                        );
                    }
                }

                // Dump registers at fault point
                eprintln!("  Registers:");
                for i in [10, 11, 12, 13, 14, 15, 5, 6, 7, 8, 9].iter() {
                    eprintln!("    x[{}] = 0x{:08X}", i, vm.cpu.x[*i]);
                }

                // Decode what the faulting instruction is
                let mepc = vm.cpu.csr.mepc;
                let mepc_pa = if mepc >= 0xC0000000 {
                    mepc - 0xC0000000
                } else {
                    mepc
                };
                let inst = vm.bus.read_word(mepc_pa as u64).unwrap_or(0);
                eprintln!("  Faulting inst at PA 0x{:08X}: 0x{:08X}", mepc_pa, inst);
            }
        }

        // Check for panic
        if vm.cpu.pc >= panic_start && vm.cpu.pc < panic_end {
            let a0 = vm.cpu.x[10];
            if a0 >= 0xC0000000 && a0 < 0xE0000000 {
                let pa = (a0 - 0xC0000000) as u64;
                let mut s = String::new();
                for j in 0..300 {
                    if let Ok(byte) = vm.bus.read_byte(pa + j) {
                        if byte == 0 {
                            break;
                        }
                        if byte >= 0x20 && byte < 0x7F {
                            s.push(byte as char);
                        } else {
                            s.push('.');
                        }
                    }
                }
                eprintln!("[PANIC] at count={}: \"{}\"", count, s);
            }
            break;
        }

        if count % 5_000_000 == 0 {
            eprintln!(
                "[progress] {}M, PC=0x{:08X}, ECALLs={}",
                count / 1_000_000,
                vm.cpu.pc,
                vm.cpu.ecall_count
            );
        }
    }

    eprintln!(
        "[done] {} instructions, first_fault_logged={}",
        count, first_fault_logged
    );
}
