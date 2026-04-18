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
    let max_instr: u64 = 2_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut panic_count: u64 = 0;

    // panic() is at 0xC000252E. Watch for calls to it.
    let panic_addr: u32 = 0xC000252E;

    // Also track SBI calls
    let mut sbi_calls: u64 = 0;
    let mut sbi_last_ext: u32 = 0;
    let mut sbi_last_fn: u32 = 0;

    // Track first few ECALLs to see what the kernel tries
    let mut ecall_log: Vec<(u64, u32, u32, u32)> = Vec::new(); // (count, a7, a6, a0)

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // SATP change handling (device identity mappings)
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    // Device regions
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
                    // Fix kernel PT entries
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
                    // Fix kernel_map
                    let km_phys: u64 = 0x00C79E90;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    eprintln!("[diag] SATP changed to 0x{:08X} at count={}", cur_satp, count);
                }
                last_satp = cur_satp;
            }
        }

        // DTB pointer watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let dtb_addr = _dtb_addr;
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
                // ECALL_S = SBI from S-mode
                sbi_calls += 1;
                let ext = vm.cpu.x[17];
                let fn_id = vm.cpu.x[16];
                let arg0 = vm.cpu.x[10];
                sbi_last_ext = ext;
                sbi_last_fn = fn_id;
                if ecall_log.len() < 20 {
                    ecall_log.push((count, ext, fn_id, arg0));
                }
                let result = vm.bus.sbi.handle_ecall(
                    ext, fn_id, arg0, vm.cpu.x[11], vm.cpu.x[12],
                    vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
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

        // Check if we're about to enter panic()
        if vm.cpu.pc == panic_addr {
            panic_count += 1;
            if panic_count <= 3 {
                eprintln!("[diag] PANIC #{} at count={}: PC=0x{:08X}", panic_count, count, vm.cpu.pc);
                // a0 = panic string pointer
                let panic_str_ptr = vm.cpu.x[10];
                eprintln!("[diag]   a0 (panic msg) = 0x{:08X}", panic_str_ptr);
                // Try to read the panic string
                let str_pa = if panic_str_ptr >= 0xC0000000 {
                    (panic_str_ptr - 0xC0000000) as u64
                } else {
                    panic_str_ptr as u64
                };
                let mut msg = Vec::new();
                for i in 0..200 {
                    let b = vm.bus.read_byte(str_pa + i).unwrap_or(0);
                    if b == 0 { break; }
                    msg.push(b);
                }
                if let Ok(s) = String::from_utf8(msg.clone()) {
                    eprintln!("[diag]   panic msg: '{}'", s);
                } else {
                    eprintln!("[diag]   panic msg (raw): {:?}", msg);
                }
                // Print backtrace
                eprintln!("[diag]   RA=0x{:08X} SP=0x{:08X}", vm.cpu.x[1], vm.cpu.x[2]);
                let sp = vm.cpu.x[2];
                let sp_pa = if sp >= 0xC0000000 { (sp - 0xC0000000) as u64 } else { sp as u64 };
                // panic saves RA at SP+60
                for (off, name) in [(60, "saved_RA"), (56, "saved_S0"), (52, "saved_S1")] {
                    let val = vm.bus.read_word(sp_pa + off as u64).unwrap_or(0);
                    eprintln!("[diag]   SP+{} ({}) = 0x{:08X}", off, name, val);
                }
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;

        // Progress every 1M
        if count % 1_000_000 == 0 {
            eprintln!("[progress] {}M: PC=0x{:08X} SBI={} panic={}",
                count / 1_000_000, vm.cpu.pc, sbi_calls, panic_count);
        }
    }

    eprintln!("[diag] Done: count={} SBI_calls={} panic_count={}",
        count, sbi_calls, panic_count);
    eprintln!("[diag] UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(3000).collect();
        eprintln!("[diag] UART:\n{}", preview);
    }

    eprintln!("\n[diag] === ECALL log (first 20) ===");
    for (cnt, ext, fn_id, arg0) in &ecall_log {
        eprintln!("[diag]   count={}: a7(ext)={} a6(fn)={} a0={}", cnt, ext, fn_id, arg0);
    }
}
