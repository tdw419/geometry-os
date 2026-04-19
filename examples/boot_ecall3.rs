use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 400_000;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut dtb_early_va_expected: u32 = (dtb_addr.wrapping_add(0xC0000000)) as u32;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C7A0B4u64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_early_va_expected {
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
            }
        }

        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
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
                    let mega_flags: u32 = 0x0000_00CF;
                    for l1_scan in 768..780u32 {
                        let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                        let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn == 0);
                        if !needs_fix {
                            continue;
                        }
                        let pa_offset = l1_scan - 768;
                        let fixup_pte = mega_flags | (pa_offset << 20);
                        vm.bus.write_word(scan_addr, fixup_pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    vm.bus.write_word(0x00C7A098 + 12, 0x00000000).ok();
                    vm.bus.write_word(0x00C7A098 + 20, 0xC0000000).ok();
                    vm.bus.write_word(0x00C7A098 + 24, 0x00000000).ok();
                    vm.bus.write_word(0x00801008, dtb_early_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
                last_satp = cur_satp;
            }
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
            let is_interrupt = (mcause >> 31) & 1 == 1;
            if is_interrupt {
                vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if cause_code == 9 || cause_code == 11 {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                        vm.cpu.csr.mstatus &= !(1u32 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _step_result = vm.step();

        if vm.cpu.pc == 0xC000252E && count > 200_000 {
            break;
        }

        count += 1;
    }

    // Print ECALL log from SBI handler
    println!("\n=== SBI ECALL log ===");
    for (i, (a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().enumerate() {
        let ext_name = match *a7 {
            0x10 => "SBI_EXT_BASE",
            0x01 => "SBI_CONSOLE_PUTCHAR",
            0x00 => "SBI_SET_TIMER",
            x => &format!("0x{:X}", x),
        };
        let func_name = if *a7 == 0x10 {
            match *a6 {
                0 => "GET_SPEC_VERSION",
                1 => "GET_IMPL_ID",
                2 => "GET_IMPL_VERSION",
                3 => "PROBE_EXTENSION",
                x => &format!("func{}", x),
            }
        } else if *a7 == 0x01 {
            if *a6 == 0 {
                "putchar"
            } else {
                "getchar"
            }
        } else {
            ""
        };
        println!(
            "  #{}: ext={}({}) func={}({}) a0=0x{:X}",
            i, ext_name, a7, func_name, a6, a0
        );
    }

    println!(
        "\nTotal: {} instructions, {} SBI ECALLs",
        count,
        vm.bus.sbi.ecall_log.len()
    );
    println!("UART tx: {} bytes", vm.bus.uart.tx_buf.len());
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("{}", &s[..s.len().min(5000)]);
    }
}
