// Trace what happens between SATP changes and init_unavailable_range.
// Focus on: DTB parsing, memblock_add, max_mapnr setup.
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instr: u64 = 800_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    // Key addresses to watch
    let setup_arch_entry: u32 = 0xC0403C40; // setup_arch entry
    let early_init_dt_scan: u32 = 0xC041BB7A; // early_init_dt_scan
    let early_init_dt_scan_mem: u32 = 0xC041C0D0; // early_init_dt_scan_memory (approx)
    let memblock_add: u32 = 0xC04032D0; // memblock_add (approx)
    let init_unavailable: u32 = 0xC04096B2; // init_unavailable_range entry

    // Watch for first call to early_init_dt_scan
    let mut dtb_scan_called = false;
    let mut dtb_scan_count = 0u32;
    let mut dtb_scan_arg = 0u32;

    // Watch for init_unavailable_range
    let mut init_unavail_count = 0u32;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // SATP change handling
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                    let identity_pte: u32 = 0x0000_00CF;
                    for &l1_idx in device_l1 {
                        let addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let existing = vm.bus.read_word(addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            let pte = identity_pte | (l1_idx << 20);
                            vm.bus.write_word(addr, pte).ok();
                        }
                    }
                    vm.cpu.tlb.flush_all();
                    let mega_flags: u32 = 0x0000_00CF;
                    for l1_scan in 768..780u32 {
                        let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                        let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn_val = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn_val == 0);
                        if needs_fix {
                            let pa_offset = l1_scan - 768;
                            let fixup_pte = mega_flags | (pa_offset << 20);
                            vm.bus.write_word(scan_addr, fixup_pte).ok();
                        }
                    }
                    vm.cpu.tlb.flush_all();
                    let km_phys: u64 = 0x00C79E90;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    eprintln!("[trace] SATP changed to 0x{:08X} at count={}", cur_satp, count);
                }
                last_satp = cur_satp;
            }
        }

        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let dtb_early_va_expected = (dtb_addr.wrapping_add(0xC0000000)) as u32;
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_early_va_expected {
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
            }
        }

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            if cause_code == 9 && mpp != 3 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Watch for entry to early_init_dt_scan
        if vm.cpu.pc == early_init_dt_scan && !dtb_scan_called {
            dtb_scan_called = true;
            dtb_scan_count += 1;
            dtb_scan_arg = vm.cpu.x[10];
            eprintln!("[trace] early_init_dt_scan called at count={}, a0(DTB)=0x{:08X}", count, dtb_scan_arg);
        }

        // Watch for entry to init_unavailable_range
        if vm.cpu.pc == init_unavailable {
            init_unavail_count += 1;
            if init_unavail_count <= 3 {
                eprintln!("[trace] init_unavailable_range called at count={}, a0=0x{:08X} a1=0x{:08X}",
                    count, vm.cpu.x[10], vm.cpu.x[11]);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;
    }

    // Final state
    eprintln!("\n=== Final state at count={} ===", count);
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("dtb_scan_called={} dtb_scan_count={} dtb_scan_arg=0x{:08X}",
        dtb_scan_called, dtb_scan_count, dtb_scan_arg);
    eprintln!("init_unavail_count={}", init_unavail_count);

    // Key memory
    let phys_ram_base = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    let max_mapnr = vm.bus.read_word(0x0148247C).unwrap_or(0);
    let mem_map = vm.bus.read_word(0x01482478).unwrap_or(0);
    eprintln!("phys_ram_base=0x{:08X}", phys_ram_base);
    eprintln!("max_mapnr=0x{:08X} ({})", max_mapnr, max_mapnr);
    eprintln!("mem_map=0x{:08X}", mem_map);

    // memblock memory count (memblock.memory.cnt at 0xC148212C)
    let memblock_mem_cnt = vm.bus.read_word(0x0148212C).unwrap_or(0);
    eprintln!("memblock.memory.cnt=0x{:08X}", memblock_mem_cnt);

    // UART
    eprintln!("UART: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("UART:\n{}", s.chars().take(3000).collect::<String>());
    }
}
