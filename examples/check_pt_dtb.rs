//! Check page table entries for DTB access and initial_boot_params state.
//! cargo run --example check_pt_dtb

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_data,
            initramfs_data.as_deref(),
            512,
            "console=ttyS0 earlycon=sbi",
        ).expect("setup failed");

    eprintln!("Entry: 0x{:08X}, DTB: 0x{:08X}", entry, dtb_addr);

    // Check initial_boot_params before running
    let ibp_before = vm.bus.read_word(0x00C7A178).unwrap_or(0);
    eprintln!("initial_boot_params before boot: 0x{:08X}", ibp_before);

    // Run boot loop with trap handling (similar to boot_linux but with more diagnostics)
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max = 500_000u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut ibp_after_satp: u32 = 0;

    while count < max {
        // Trap handling (simplified from boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code != 11 { // Not ECALL_M
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 { // From S-mode or U-mode
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                // On-demand identity map for low addresses
                let fault_addr = vm.cpu.csr.mtval;
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
                            eprintln!("[diag] On-demand identity map: L1[{}] for fault VA=0x{:08X}", vpn1, fault_addr);
                        }
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();

        // Check SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[diag] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);

            // Check initial_boot_params after SATP change
            let ibp = vm.bus.read_word(0x00C7A178).unwrap_or(0);
            eprintln!("[diag]   initial_boot_params: 0x{:08X}", ibp);
            ibp_after_satp = ibp;

            // Check DTB page table entries
            let ppn = (cur_satp & 0x3FFFFF) as u64;
            let pg_dir_phys = ppn * 4096;
            // DTB VA 0xC1579000 -> L1[773]
            let dtb_l1 = vm.bus.read_word(pg_dir_phys + 773 * 4).unwrap_or(0);
            eprintln!("[diag]   L1[773] (DTB range): 0x{:08X}", dtb_l1);
            let is_leaf = (dtb_l1 & 0xE) != 0;
            let ppn_val = (dtb_l1 >> 10) & 0x3FFFFF;
            if is_leaf {
                eprintln!("[diag]     -> megapage PA=0x{:08X}", ppn_val << 22);
            } else if (dtb_l1 & 1) != 0 {
                eprintln!("[diag]     -> non-leaf L2 at PA=0x{:08X}", ppn_val << 12);
                // Check L2 entry for DTB VA
                let dtb_vpn0 = ((0xC1579_000u32 >> 12) & 0x3FF) as u64;
                let l2_addr = ((ppn_val as u64) << 12) | (dtb_vpn0 * 4);
                let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                eprintln!("[diag]     L2[{}] = 0x{:08X}", dtb_vpn0, l2_entry);
                let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
                eprintln!("[diag]     -> PA=0x{:08X}", l2_ppn << 12);
            } else {
                eprintln!("[diag]     -> UNMAPPED!");
            }

            // Check _dtb_early_va
            let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
            let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            eprintln!("[diag]   _dtb_early_va: 0x{:08X} (expect 0x{:08X})", deva, dtb_addr.wrapping_add(0xC0000000));
            eprintln!("[diag]   _dtb_early_pa: 0x{:08X} (expect 0x{:08X})", depa, dtb_addr);

            // Check phys_ram_base
            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            eprintln!("[diag]   phys_ram_base: 0x{:08X}", prb);

            // Check memblock
            let mb = 0x00803448u64;
            let mem_cnt = vm.bus.read_word(mb + 8).unwrap_or(0);
            eprintln!("[diag]   memblock memory.cnt: {}", mem_cnt);

            last_satp = cur_satp;
        }

        count += 1;
    }

    eprintln!("\nAfter 500K instructions:");
    eprintln!("  PC: 0x{:08X}", vm.cpu.pc);
    let ibp = vm.bus.read_word(0x00C7A178).unwrap_or(0);
    eprintln!("  initial_boot_params: 0x{:08X}", ibp);
    let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!("  phys_ram_base: 0x{:08X}", prb);

    // Now run 2M more with DTB watchdog
    eprintln!("\n--- Running 2M more with DTB watchdog ---");
    let dtb_va_expected: u32 = dtb_addr.wrapping_add(0xC0000000) as u32;
    while count < 2_500_000 {
        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_va_expected {
                    vm.bus.write_word(0x00801008, dtb_va_expected).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                    eprintln!("[watchdog] Restored _dtb_early_va at count={}", count);
                }
            }
        }

        // Same trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                let fault_addr = vm.cpu.csr.mtval;
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
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    eprintln!("\nAfter 2.5M total:");
    eprintln!("  PC: 0x{:08X}", vm.cpu.pc);
    let prb2 = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!("  phys_ram_base: 0x{:08X}", prb2);
    let mb = 0x00803448u64;
    let mem_cnt = vm.bus.read_word(mb + 8).unwrap_or(0);
    eprintln!("  memblock memory.cnt: {}", mem_cnt);
    let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
    eprintln!("  _dtb_early_va: 0x{:08X}", deva);
}
