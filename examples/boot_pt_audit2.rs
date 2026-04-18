//! Check kernel's page tables after setup_vm for DTB mapping.
//! Run: cargo run --example boot_pt_audit2

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
        128,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .expect("boot setup failed");

    vm.bus.auto_pte_fixup = false;
    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = ((dtb_addr.wrapping_add(0xC0000000)) & 0xFFFFFFFF) as u32;
    let dtb_pa = dtb_addr as u32;
    let mut count: u64 = 0;
    let max_instr = 200_000u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: Vec<(u64, u32, u32)> = Vec::new();

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
            if cause_code == 9 {
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                let result = vm.bus.sbi.handle_ecall(
                    a7,
                    a6,
                    a0,
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((ret_a0, ret_a1)) = result {
                    vm.cpu.x[10] = ret_a0;
                    vm.cpu.x[11] = ret_a1;
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

        let step_result = vm.step();
        count += 1;

        match step_result {
            StepResult::Ebreak => {
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    let fault_addr = vm.cpu.csr.stval;
                    let cause_code = vm.cpu.csr.scause & !(1u32 << 31);
                    let is_pf = cause_code == 12 || cause_code == 13 || cause_code == 15;
                    if is_pf && fault_addr < 0x0200_0000 {
                        let satp = vm.cpu.csr.satp;
                        let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                        if pg_dir_ppn > 0 {
                            let pg_dir_phys = pg_dir_ppn * 4096;
                            let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                            let l1_addr = pg_dir_phys + vpn1 * 4;
                            let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                            if (existing & 1) == 0 {
                                let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                                vm.bus.write_word(l1_addr, pte).ok();
                                vm.cpu.tlb.flush_all();
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes.push((count, last_satp, cur_satp));
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
            for l1_scan in 768..780u32 {
                let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                let is_valid = (entry & 1) != 0;
                let is_non_leaf = is_valid && (entry & 0xE) == 0;
                if is_valid && !is_non_leaf {
                    continue;
                }
                let pa_offset = l1_scan - 768;
                vm.bus
                    .write_word(scan_addr, 0x0000_00CF | (pa_offset << 20))
                    .ok();
            }
            vm.cpu.tlb.flush_all();
            vm.bus.write_word(0x00801008, dtb_va).ok();
            vm.bus.write_word(0x0080100C, dtb_pa).ok();
            last_satp = cur_satp;
        }
    }

    // Audit the CURRENT page table (early_pg_dir at SATP PPN)
    let satp = vm.cpu.csr.satp;
    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_phys = pg_dir_ppn * 4096;

    eprintln!(
        "[audit] Current SATP = 0x{:08X}, pg_dir at PA 0x{:08X}",
        satp, pg_dir_phys
    );
    eprintln!("[audit] SATP changes: {:?}", satp_changes);

    // Check L1[773] (DTB VA range: 0xC1400000-0xC17FFFFF)
    let dtb_l1_idx = 773u64;
    let l1_entry = vm.bus.read_word(pg_dir_phys + dtb_l1_idx * 4).unwrap_or(0);
    let is_valid = (l1_entry & 1) != 0;
    let is_non_leaf = is_valid && (l1_entry & 0xE) == 0;
    eprintln!(
        "[audit] L1[773] = 0x{:08X} V={} non_leaf={}",
        l1_entry, is_valid, is_non_leaf
    );

    if is_non_leaf {
        // L2 table exists - check if DTB page is mapped
        let l2_ppn = ((l1_entry >> 10) as u64) * 4096;
        eprintln!("[audit] L2 table at PA 0x{:08X}", l2_ppn);

        // DTB VA 0xC1579000: VPN0 = (0xC1579000 >> 12) & 0x3FF = 0x579
        let vpn0 = ((0xC1579000u64 >> 12) & 0x3FF) as u64;
        let l2_entry = vm.bus.read_word(l2_ppn + vpn0 * 4).unwrap_or(0);
        eprintln!(
            "[audit] L2[0x{:03X}] (VA 0xC1579000) = 0x{:08X} V={}",
            vpn0,
            l2_entry,
            l2_entry & 1
        );

        // Check a few more L2 entries around the DTB
        for offset in -2..=2i32 {
            let idx = (vpn0 as i64 + offset as i64) as u64;
            let entry = vm.bus.read_word(l2_ppn + idx * 4).unwrap_or(0);
            eprintln!(
                "[audit] L2[0x{:03X}] = 0x{:08X} V={}",
                idx,
                entry,
                entry & 1
            );
        }
    } else if is_valid {
        // Megapage - should cover entire 4MB range
        let pa_base = ((l1_entry >> 10) as u64) << 12;
        eprintln!("[audit] Megapage: PA base = 0x{:08X}", pa_base);
    } else {
        eprintln!("[audit] NOT MAPPED! DTB at VA 0xC1579000 is inaccessible!");
    }

    // Also check the boot page table (SATP from first entry)
    if !satp_changes.is_empty() {
        let boot_satp = satp_changes[0].1;
        let boot_ppn = (boot_satp & 0x3FFFFF) as u64;
        let boot_phys = boot_ppn * 4096;
        let boot_l1 = vm.bus.read_word(boot_phys + dtb_l1_idx * 4).unwrap_or(0);
        eprintln!(
            "\n[audit] Boot page table L1[773] = 0x{:08X} (SATP was 0x{:08X})",
            boot_l1, boot_satp
        );
    }

    // Check the kernel's trampoline_pg_dir L1[773]
    // trampoline_pg_dir is at PA 0x01484000
    let tramp_l1 = vm.bus.read_word(0x01484000 + dtb_l1_idx * 4).unwrap_or(0);
    eprintln!("[audit] trampoline_pg_dir L1[773] = 0x{:08X}", tramp_l1);

    // Check early_pg_dir L1[773]
    let early_l1 = vm.bus.read_word(0x00802000 + dtb_l1_idx * 4).unwrap_or(0);
    eprintln!("[audit] early_pg_dir L1[773] = 0x{:08X}", early_l1);
}
