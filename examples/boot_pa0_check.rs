/// Diagnostic: Check what the kernel writes to PA 0 (page table allocation)
/// and verify whether our megapage fixup is correct or harmful.
use geometry_os::riscv::RiscvVm;

fn dump_l2_table(vm: &mut RiscvVm, pa: u64, label: &str) {
    eprintln!("[DIAG] L2 table at PA 0x{:08X} ({}):", pa, label);
    let mut valid_count = 0;
    let mut zero_count = 0;
    let mut other_count = 0;
    for i in 0..64 { // Show first 64 entries
        let addr = pa + (i as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte != 0 {
            let v = (pte >> 0) & 1;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let u = (pte >> 4) & 1;
            let ppn = (pte >> 10) & 0x3FFFFF;
            eprintln!("  [{:3}] = 0x{:08X} V={} R={} W={} X={} U={} PPN=0x{:06X}", i, pte, v, r, w, x, u, ppn);
            valid_count += 1;
        } else {
            zero_count += 1;
        }
    }
    // Check remaining entries
    for i in 64..1024 {
        let addr = pa + (i as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte != 0 { other_count += 1; }
    }
    eprintln!("  Summary: {} non-zero (shown), {} more non-zero (not shown), {} zero", valid_count, other_count, zero_count);
}

fn check_kernel_code_at_pa0(vm: &mut RiscvVm) {
    eprintln!("[DIAG] Checking kernel code at PA 0 (first 16 words):");
    for i in 0..16 {
        let addr = (i as u64) * 4;
        let w = vm.bus.read_word(addr).unwrap_or(0);
        eprintln!("  PA[0x{:03X}] = 0x{:08X}", i*4, w);
    }
}

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
    ).unwrap();

    // Save original kernel code at PA 0
    let mut orig_code: Vec<u32> = Vec::new();
    for i in 0..16 {
        orig_code.push(vm.bus.read_word((i * 4) as u64).unwrap_or(0));
    }
    eprintln!("[DIAG] Original kernel code at PA 0 saved");

    let max_instructions = 800_000u64;
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_change_count = 0u32;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        // SATP change handling
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                satp_change_count += 1;
                eprintln!("\n[DIAG] SATP change #{}: 0x{:08X} -> 0x{:08X} at count={}", satp_change_count, last_satp, cur_satp, count);
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;

                    // Before fixup: dump L1 entries and any L2 tables at PA 0
                    eprintln!("[DIAG] Page table at PA 0x{:08X}, L1 entries [768..779]:", pg_dir_phys);
                    for l1_idx in 768..780u32 {
                        let addr = pg_dir_phys + (l1_idx as u64) * 4;
                        let entry = vm.bus.read_word(addr).unwrap_or(0);
                        let is_valid = (entry & 1) != 0;
                        let is_non_leaf = is_valid && (entry & 0xE) == 0;
                        let ppn_val = (entry >> 10) & 0x3FFFFF;
                        eprintln!("  L1[{}] = 0x{:08X} valid={} non_leaf={} PPN=0x{:06X}",
                            l1_idx, entry, is_valid, is_non_leaf, ppn_val);

                        // If non-leaf entry points to PA 0, dump the L2 table
                        if is_non_leaf && ppn_val == 0 {
                            dump_l2_table(&mut vm, 0, &format!("L1[{}] -> PA 0", l1_idx));
                        }
                    }

                    // Check if PA 0 was overwritten (kernel code vs PTE data)
                    let pa0_word0 = vm.bus.read_word(0).unwrap_or(0);
                    let pa0_word1 = vm.bus.read_word(4).unwrap_or(0);
                    if pa0_word0 != orig_code[0] || pa0_word1 != orig_code[1] {
                        eprintln!("[DIAG] PA 0 WAS OVERWRITTEN!");
                        eprintln!("  Before: 0x{:08X} 0x{:08X}", orig_code[0], orig_code[1]);
                        eprintln!("  After:  0x{:08X} 0x{:08X}", pa0_word0, pa0_word1);
                        check_kernel_code_at_pa0(&mut vm);
                    } else {
                        eprintln!("[DIAG] PA 0 NOT overwritten (still has kernel code)");
                    }

                    // Apply standard fixup (same as boot.rs)
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
                    // kernel_map fixup
                    let km_phys: u64 = 0x00C79E90;
                    vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                    vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                    vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    vm.cpu.tlb.flush_all();
                }
                last_satp = cur_satp;
            }
        }

        // Trap handling (same as boot.rs)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M = SBI call
            } else if cause_code == 9 {
                // ECALL_S = SBI call (delegated)
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
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
                        if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
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

        // Panic detection
        if vm.cpu.pc == 0xC000252E {
            eprintln!("\n[DIAG] *** PANIC at count={} ***", count);
            eprintln!("[DIAG] RA=0x{:08X}", vm.cpu.x[1]);
            // Read the format string
            let fmt_va = vm.cpu.x[10];
            if fmt_va >= 0xC0000000 {
                let fmt_pa = fmt_va - 0xC0000000;
                let mut chars = Vec::new();
                for j in 0..200 {
                    let b = vm.bus.read_byte(fmt_pa as u64 + j as u64).unwrap_or(0);
                    if b == 0 { break; }
                    if b >= 0x20 && b < 0x7f { chars.push(b as char); } else { break; }
                }
                let s: String = chars.iter().collect();
                eprintln!("[DIAG] FMT: \"{}\"", s);
            }
            break;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    eprintln!("\n[DIAG] Final: count={} PC=0x{:08X} SATP=0x{:08X}", count, vm.cpu.pc, vm.cpu.csr.satp);
}
