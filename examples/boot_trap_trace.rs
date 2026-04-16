/// Focus on the ra corruption in print_log_buf_usage_stats.
/// The function saves ra at sp+12, calls _printk, and when _printk returns,
/// lw ra, 12(sp) loads 0 instead of the saved value.
/// Hypothesis: something overwrites the stack between save and restore.
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            512,
            bootargs,
        )
        .expect("setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 500_000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;

    // Watch for the specific _printk call inside print_log_buf_usage_stats
    let mut watching = false;
    let mut watch_sp: u32 = 0;
    let mut watch_ra_addr: u64 = 0; // physical address where ra is saved
    let mut ra_saved_value: u32 = 0;
    let mut _ra_corrupted = false;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Identity injection
        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
        }

        // TLB pre-warming
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            let new_ppn = (cur_satp & 0x3FFFFF) as u64;
            let pt_base = new_ppn << 12;
            let asid = (cur_satp >> 22) & 0x1FF;
            for l1_idx in 0..1024u64 {
                let l1_addr = pt_base + l1_idx * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                if l1_pte == 0 || (l1_pte & 0xE) == 0 {
                    continue;
                }
                let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u32;
                let flags = (l1_pte & 0xFF) as u32;
                for vpn0 in 0..512u32 {
                    let vpn_combined = ((l1_idx as u32) << 10) | vpn0;
                    let eff_ppn = (ppn_hi << 10) | vpn0;
                    vm.cpu.tlb.insert(vpn_combined, asid as u16, eff_ppn, flags);
                }
            }
            last_satp = cur_satp;
        }

        // M-mode trap handling
        if vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let mepc = vm.cpu.csr.mepc;
            let cause_code = (mcause & 0xFF) as u8;

            if cause_code == 9 {
                // ECALL_S - handle as SBI
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
                vm.cpu.csr.mepc = mepc.wrapping_add(4);
                vm.step();
                count += 1;
                continue;
            }

            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
            if cause_code != 8 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }

            vm.cpu.csr.mepc = mepc.wrapping_add(4);
            vm.step();
            count += 1;
            continue;
        }

        // Watch for print_log_buf_usage_stats entry (0xC000305E)
        if vm.cpu.pc == 0xC000305E && !watching && count > 170000 {
            watching = true;
            watch_sp = vm.cpu.x[2]; // sp before addi sp, -16
            watch_ra_addr = (watch_sp - 16 + 12) as u64; // sp-16+12 = sp-4
            eprintln!(
                "[WATCH] print_log_buf_usage_stats entry at count={}, sp=0x{:08X}, ra_save_addr=0x{:08X}",
                count, watch_sp, watch_ra_addr as u32
            );
            eprintln!(
                "[WATCH] ra=0x{:08X} (should be saved to sp+12=0x{:08X})",
                vm.cpu.x[1], (watch_sp - 16 + 12)
            );
        }

        // After sw ra, 12(sp) at 0xC0003062, check what was saved
        if watching && vm.cpu.pc == 0xC0003064 && count > 170000 {
            let current_sp = vm.cpu.x[2];
            let saved_ra = vm.bus.read_word(watch_ra_addr).unwrap_or(0xDEAD);
            ra_saved_value = vm.cpu.x[1]; // ra before the save
            eprintln!(
                "[WATCH] After sw ra,12(sp): sp=0x{:08X} saved_ra_at_0x{:08X}=0x{:08X} (ra was 0x{:08X})",
                current_sp, watch_ra_addr as u32, saved_ra, ra_saved_value
            );
        }

        // Before lw ra, 12(sp) at 0xC0003084, check what's on the stack
        if watching && vm.cpu.pc == 0xC0003084 && count > 170000 {
            let current_sp = vm.cpu.x[2];
            let ra_at_stack = vm.bus.read_word(watch_ra_addr).unwrap_or(0xDEAD);
            eprintln!(
                "[WATCH] Before lw ra,12(sp): sp=0x{:08X} ra_at_stack=0x{:08X} (was saved as 0x{:08X})",
                current_sp, ra_at_stack, ra_saved_value
            );
            if ra_at_stack != ra_saved_value {
                eprintln!("[WATCH] *** RA CORRUPTION DETECTED! ***");
                _ra_corrupted = true;
            }
            watching = false;
            break;
        }

        vm.step();
        count += 1;
    }

    eprintln!("[diag] Total steps: {}", count);
}
