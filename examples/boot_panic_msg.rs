// Capture the panic message from the kernel
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instr: u64 = 20_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut panic_caught = false;

    let panic_addr: u32 = 0xC000252E; // panic()

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
                    // DTB mapping
                    {
                        let dtb_va: u32 = (_dtb_addr.wrapping_add(0xC0000000)) as u32;
                        let dtb_vpn1 = ((dtb_va >> 22) & 0x3FF) as u64;
                        let dtb_vpn0 = ((dtb_va >> 12) & 0x3FF) as u64;
                        let l1_addr = pg_dir_phys + dtb_vpn1 * 4;
                        let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                        let l1_valid = (l1_entry & 1) != 0;
                        let l1_leaf = l1_valid && (l1_entry & 0xE) != 0;
                        if l1_valid && !l1_leaf {
                            let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                            let l2_base = l2_ppn * 4096;
                            let l2_addr = l2_base + dtb_vpn0 * 4;
                            let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                            if (l2_entry & 1) == 0 {
                                let dtb_ppn = (_dtb_addr >> 12) as u32;
                                let dtb_pte: u32 = (dtb_ppn << 10) | 0x0000_00CF;
                                vm.bus.write_word(l2_addr, dtb_pte).ok();
                                eprintln!("[boot] Added DTB L2 mapping");
                            }
                        }
                    }
                    vm.cpu.tlb.flush_all();
                    // kernel_map fixup
                    let km_phys: u64 = 0x00C79E90;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    vm.bus.write_word(0x00801008, (_dtb_addr.wrapping_add(0xC0000000)) as u32).ok();
                    vm.bus.write_word(0x0080100C, _dtb_addr as u32).ok();
                    eprintln!("[boot] SATP changed to 0x{:08X} at count={}", cur_satp, count);
                }
                last_satp = cur_satp;
            }
        }

        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let dtb_early_va_expected = (_dtb_addr.wrapping_add(0xC0000000)) as u32;
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_early_va_expected {
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, _dtb_addr as u32).ok();
                }
            }
        }

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            if cause_code == 9 && mpp != 3 {
                // ECALL_S = SBI call
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

        // Capture panic message
        if vm.cpu.pc == panic_addr && !panic_caught {
            panic_caught = true;
            let a0 = vm.cpu.x[10]; // panic string
            let sp = vm.cpu.x[2];
            let ra = vm.cpu.x[1];
            eprintln!("[PANIC] at count={}, a0=0x{:08X} RA=0x{:08X} SP=0x{:08X}", count, a0, ra, sp);

            // Read the panic string
            let str_pa = if a0 >= 0xC0000000 {
                (a0 - 0xC0000000) as u64
            } else {
                a0 as u64
            };
            let mut msg = Vec::new();
            for i in 0..300 {
                let b = vm.bus.read_byte(str_pa + i).unwrap_or(0);
                if b == 0 {
                    break;
                }
                msg.push(b);
            }
            if let Ok(s) = String::from_utf8(msg.clone()) {
                eprintln!("[PANIC] msg: '{}'", s);
            } else {
                eprintln!("[PANIC] raw: {:?}", msg);
            }

            // Also read the format string (a1 is the fmt string for the second arg)
            // panic(fmt, ...) where fmt is the first arg
            // But first, print the stack to find the caller
            let sp_pa = if sp >= 0xC0000000 {
                (sp - 0xC0000000) as u64
            } else {
                sp as u64
            };
            for (off, name) in [(12, "saved_RA"), (8, "saved_S0"), (4, "saved_S1"), (0, "saved_S2")] {
                let val = vm.bus.read_word(sp_pa + off as u64).unwrap_or(0);
                eprintln!("[PANIC]   SP+{} ({}) = 0x{:08X}", off, name, val);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;

        if count % 5_000_000 == 0 {
            eprintln!("[progress] {}M: PC=0x{:08X}", count / 1_000_000, vm.cpu.pc);
        }
    }

    eprintln!("\nDone: count={}", count);
    eprintln!("UART: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("UART:\n{}", s.chars().take(5000).collect::<String>());
    }
}
