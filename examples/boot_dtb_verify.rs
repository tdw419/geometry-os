//! Verify DTB is accessible by reading through MMU translation.
//! Run: cargo run --example boot_dtb_verify

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi loglevel=7",
    )
    .expect("boot_linux_setup failed");

    vm.bus.auto_pte_fixup = false;
    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = ((dtb_addr.wrapping_add(0xC0000000)) & 0xFFFFFFFF) as u32;
    let dtb_pa = dtb_addr as u32;

    // Run boot loop (simplified M-mode handler)
    let mut count = 0u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    while count < 200_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
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
            } else if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let result = vm.step();
        count += 1;
        if matches!(result, StepResult::Ebreak) {
            break;
        }
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {}
            _ => {}
        }
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[verify] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            for i in 0..64u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (i << 20)).ok();
                }
            }
            for &l1_idx in &[8u32, 48, 64] {
                let addr = pg_dir_phys + (l1_idx as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (l1_idx << 20)).ok();
                }
            }
            // Fix kernel PT
            for l1_scan in 768..780u32 {
                let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                let is_valid = (entry & 1) != 0;
                let is_non_leaf = is_valid && (entry & 0xE) == 0;
                if is_valid && !is_non_leaf {
                    continue;
                } // Good megapage, skip
                let pa_offset = l1_scan - 768;
                let fixup_pte = 0x0000_00CF | (pa_offset << 20);
                vm.bus.write_word(scan_addr, fixup_pte).ok();
            }
            vm.cpu.tlb.flush_all();
            // Restore DTB pointers
            vm.bus.write_word(0x00801008, dtb_va).ok();
            vm.bus.write_word(0x0080100C, dtb_pa).ok();
            last_satp = cur_satp;
        }
    }

    // Now check: what does L1[773] look like in the CURRENT page directory?
    let satp = vm.cpu.csr.satp;
    let pg_dir_ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (pg_dir_ppn as u64) * 4096;
    eprintln!(
        "[verify] Current SATP: 0x{:08X}, pg_dir PA: 0x{:08X}",
        satp, pg_dir_phys
    );

    // Check L1 entries for the DTB VA range
    let dtb_vpn1 = (dtb_va >> 22) & 0x3FF;
    let l1_entry = vm
        .bus
        .read_word(pg_dir_phys + (dtb_vpn1 as u64) * 4)
        .unwrap_or(0);
    eprintln!(
        "[verify] DTB VA 0x{:08X}: VPN1={}, L1[{}] = 0x{:08X}",
        dtb_va, dtb_vpn1, dtb_vpn1, l1_entry
    );
    let is_leaf = (l1_entry & 0xE) != 0;
    let ppn_hi = (l1_entry >> 20) & 0xFFF;
    eprintln!(
        "[verify]   leaf={} ppn_hi={} (PA base=0x{:08X})",
        is_leaf,
        ppn_hi,
        (ppn_hi as u64) << 22
    );

    // Manually compute the expected PA
    let vpn0 = (dtb_va >> 12) & 0x3FF;
    let offset = dtb_va & 0xFFF;
    let expected_pa = ((ppn_hi as u64) << 22) | ((vpn0 as u64) << 12) | (offset as u64);
    eprintln!(
        "[verify]   VPN0={}, offset=0x{:03X}, expected PA=0x{:08X}",
        vpn0, offset, expected_pa
    );
    eprintln!("[verify]   Actual DTB PA = 0x{:08X}", dtb_pa);
    eprintln!("[verify]   Match: {}", expected_pa == dtb_addr);

    // Read DTB directly from PA
    let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!(
        "[verify] DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)",
        dtb_addr, dtb_magic
    );

    // Read what the MMU would return for DTB VA (manually walk page table)
    let mmu_pa = vm.bus.read_word(expected_pa).unwrap_or(0);
    eprintln!(
        "[verify] Value at MMU-translated PA 0x{:08X}: 0x{:08X}",
        expected_pa, mmu_pa
    );

    // Check if kernel's _dtb_early_va is correct
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!(
        "[verify] _dtb_early_va = 0x{:08X} (expect 0x{:08X})",
        dtb_early_va, dtb_va
    );
    eprintln!(
        "[verify] _dtb_early_pa = 0x{:08X} (expect 0x{:08X})",
        dtb_early_pa, dtb_pa
    );

    // Check a few more L1 entries around the kernel range
    for l1_idx in [768, 769, 770, 771, 772, 773, 774, 775] {
        let entry = vm
            .bus
            .read_word(pg_dir_phys + (l1_idx as u64) * 4)
            .unwrap_or(0);
        let ppn_hi = (entry >> 20) & 0xFFF;
        let va_base = (l1_idx as u64) << 22;
        let pa_base = (ppn_hi as u64) << 22;
        let is_leaf = (entry & 0xE) != 0;
        eprintln!(
            "[verify] L1[{}] = 0x{:08X} leaf={} VA 0x{:08X} -> PA 0x{:08X}",
            l1_idx,
            entry,
            is_leaf,
            va_base + 0xC0000000u64,
            pa_base
        );
    }
}
