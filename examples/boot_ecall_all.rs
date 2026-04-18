//! Count ALL ECALLs with full details.
//! Run: cargo run --example boot_ecall_all

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
    let max_instr = 50_000_000u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let time_accel = 100u64;
    let mut ecall_count: u64 = 0;
    let mut putchar_count: u64 = 0;

    // Track where ECALL instructions are (ecall opcode = 0x00000073)
    let mut mmode_trap_count: u64 = 0;

    eprintln!("[ecall] Booting...");

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Check if we're at the fw_addr (M-mode trap handler)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            mmode_trap_count += 1;
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;

            if cause_code == 9 {
                // ECALL from S-mode
                ecall_count += 1;
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                let a1 = vm.cpu.x[11];
                let ra = vm.cpu.x[1];

                // Handle the ECALL
                let result = vm.bus.sbi.handle_ecall(
                    a7,
                    a6,
                    a0,
                    a1,
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

                // Check if this was a console putchar
                if a7 == 0x01 || (a7 == 0x02 && a6 == 0) || a7 == 0x4442434E {
                    putchar_count += 1;
                    if putchar_count <= 5 {
                        eprintln!(
                            "[ecall] PUTCHAR #{}: a7=0x{:X} a6=0x{:X} a0=0x{:X} ('{}')",
                            putchar_count, a7, a6, a0, a0 as u8 as char
                        );
                    }
                }

                // Log first 50 ECALLs
                if ecall_count <= 50 {
                    eprintln!("[ecall] #{} at count={}: a7=0x{:08X} a6=0x{:X} a0=0x{:08X} a1=0x{:08X} mepc=0x{:08X} ra=0x{:08X}",
                        ecall_count, count, a7, a6, a0, a1, vm.cpu.csr.mepc, ra);
                }
            } else if cause_code != 11 && mpp != 3 {
                // Other exceptions from S-mode
                if mmode_trap_count <= 50 {
                    eprintln!(
                        "[ecall] M-mode trap #{} at count={}: cause={} mepc=0x{:08X} priv={:?}",
                        mmode_trap_count, count, cause_code, vm.cpu.csr.mepc, vm.cpu.privilege
                    );
                }
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

        for _ in 0..time_accel {
            vm.bus.tick_clint();
        }
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

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

        // SATP change
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[ecall] SATP: 0x{:08X} -> 0x{:08X} at count={}",
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

    let console: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "\n[ecall] Done: {} instr, {} ECALLs, {} putchars, {} M-mode traps, {} console chars",
        count,
        ecall_count,
        putchar_count,
        mmode_trap_count,
        console.len()
    );
    if !console.is_empty() {
        eprintln!(
            "[ecall] Console output:\n{}",
            &console[..console.len().min(3000)]
        );
    }
}
