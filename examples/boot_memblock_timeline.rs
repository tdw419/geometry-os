/// Diagnostic: read memblock struct BEFORE and AFTER third SATP change.
/// memblock global is at VA 0xC0803448 (PA 0x00803448).
use geometry_os::riscv::RiscvVm;

fn read_memblock(vm: &mut RiscvVm, label: &str) {
    let mb_base: u64 = 0x00803448;
    eprintln!("[DIAG] {} (PA 0x{:08X}):", label, mb_base);
    for i in 0..20 {
        let addr = mb_base + (i as u64) * 4;
        let val = vm.bus.read_word(addr).unwrap_or(0);
        eprintln!("[DIAG]   +{:3}: 0x{:08X}", i * 4, val);
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
    )
    .unwrap();

    let max_instructions = 5_000_000u64;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut count: u64 = 0;
    let mut satp_change_count = 0u32;

    // Read memblock at the very start
    read_memblock(&mut vm, "Before boot");

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                satp_change_count += 1;
                eprintln!(
                    "[DIAG] SATP change #{}: 0x{:08X} -> 0x{:08X} at count={}",
                    satp_change_count, last_satp, cur_satp, count
                );
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;

                    // Read memblock BEFORE fixup
                    read_memblock(
                        &mut vm,
                        format!("Before fixup (SATP change #{})", satp_change_count).as_str(),
                    );

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
                        let ppn = (entry >> 10) & 0x3FFFFF;
                        let needs_fix = !is_valid || (is_non_leaf && ppn == 0);
                        if needs_fix {
                            let pa_offset = l1_scan - 768;
                            let fixup_pte = mega_flags | (pa_offset << 20);
                            vm.bus.write_word(scan_addr, fixup_pte).ok();
                        }
                    }
                    let km_phys: u64 = 0x00C79E90;
                    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
                    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
                    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
                    if km_pa != 0 || km_vapo != 0xC0000000 || km_vkpo != 0x00000000 {
                        eprintln!("[DIAG] Re-patching kernel_map");
                        vm.bus.write_word(km_phys + 12, 0x00000000).ok();
                        vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                        vm.bus.write_word(km_phys + 24, 0x00000000).ok();
                    }
                    vm.cpu.tlb.flush_all();

                    // Read memblock AFTER fixup
                    read_memblock(
                        &mut vm,
                        format!("After fixup (SATP change #{})", satp_change_count).as_str(),
                    );
                }
                last_satp = cur_satp;
            }
        }
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
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
                    if let Some((a0_val, a1_val)) = result {
                        vm.cpu.x[10] = a0_val;
                        vm.cpu.x[11] = a1_val;
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Also read memblock at count=700000 (before third SATP change)
        if count == 700000 {
            read_memblock(&mut vm, "At count=700000 (before 3rd SATP change)");
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    read_memblock(&mut vm, &format!("At end (count={})", count));
}
