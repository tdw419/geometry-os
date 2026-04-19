//! Detailed ECALL tracing during boot.
//! Run: cargo run --example boot_ecall_trace

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
    let max_instr = 10_000_000u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let time_accel = 1000u64;
    let mut last_ecall_count: u64 = 0;
    let mut udelay_enter_count: u64 = 0;
    let mut panic_enter_count: u64 = 0;
    let mut last_pc: u32 = 0;
    let mut reported_udelay = false;
    let mut reported_panic = false;

    eprintln!("[trace] Booting with ECALL tracing...");

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Track ecall_count changes
        let cur_ecall = vm.cpu.ecall_count;
        if cur_ecall != last_ecall_count {
            eprintln!(
                "[trace] ECALL #{} at count={}: PC_before=0x{:08X} priv={:?}",
                cur_ecall, count, last_pc, vm.cpu.privilege
            );
            eprintln!(
                "[trace]   a0(x10)=0x{:08X} a1(x11)=0x{:08X} a7(x17)=0x{:08X} a6(x16)=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[17], vm.cpu.x[16]
            );
            eprintln!(
                "[trace]   RA(x1)=0x{:08X} SP(x2)=0x{:08X} GP(x3)=0x{:08X} TP(x4)=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
            );
            eprintln!(
                "[trace]   After: a0(x10)=0x{:08X} a1(x11)=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11]
            );
            last_ecall_count = cur_ecall;
        }

        // Track udelay entry
        let pc = vm.cpu.pc;
        if pc == 0xC020B0D2 && !reported_udelay {
            eprintln!(
                "[trace] udelay entered at count={}: a0(microseconds)=0x{:08X} ({})",
                count, vm.cpu.x[10], vm.cpu.x[10]
            );
            eprintln!("[trace]   Called from RA=0x{:08X}", vm.cpu.x[1]);
            reported_udelay = true;
        }
        // Count udelay entries
        if pc == 0xC020B0D2 {
            udelay_enter_count += 1;
        }

        // Track panic entry
        if (pc >= 0xC0002500 && pc <= 0xC0002800) && !reported_panic {
            eprintln!("[trace] PANIC entered at count={}: PC=0x{:08X}", count, pc);
            eprintln!(
                "[trace]   a0(fmt_str)=0x{:08X} a1=0x{:08X} a2=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]
            );
            eprintln!(
                "[trace]   RA=0x{:08X} SP=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2]
            );
            // Try to read the format string
            let fmt_va = vm.cpu.x[10];
            if fmt_va > 0xC0000000 && fmt_va < 0xC2000000 {
                let mut fmt = String::new();
                for i in 0..200 {
                    let b = vm.bus.read_byte(fmt_va as u64 + i as u64).unwrap_or(0);
                    if b == 0 {
                        break;
                    }
                    fmt.push(b as char);
                }
                eprintln!("[trace]   fmt string: \"{}\"", fmt);
            }
            reported_panic = true;
        }
        if pc >= 0xC0002500 && pc <= 0xC0002800 {
            panic_enter_count += 1;
        }

        // M-mode trap handling
        if pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
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

        for _ in 0..time_accel {
            vm.bus.tick_clint();
        }
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        count += 1;
        last_pc = pc;

        match step_result {
            StepResult::Ebreak => {
                eprintln!("[trace] EBREAK at count={}", count);
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
                "[trace] SATP: 0x{:08X} -> 0x{:08X} at count={}",
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

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "\n[trace] Done: {} instr, {} ECALLs, {} udelay entries, {} panic entries",
        count, vm.cpu.ecall_count, udelay_enter_count, panic_enter_count
    );
    eprintln!("[trace] Console: {} chars", sbi_str.len());
    if !sbi_str.is_empty() {
        eprintln!("[trace] {}", &sbi_str[..sbi_str.len().min(3000)]);
    }
}
