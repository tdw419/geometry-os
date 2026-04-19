//! Boot with accelerated CLINT time (1000x) to make udelay complete faster.
//! Run: cargo run --example boot_fast_time

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
    let time_accel = 1000u64; // Tick CLINT 1000x per instruction
    let mut sbi_count: u64 = 0;
    let mut last_ecall_log: String = String::new();
    let mut console_len_at_last_log: usize = 0;
    let mut stall_count: u64 = 0;
    let mut last_pc: u32 = 0;

    eprintln!(
        "[fast] Booting with {}x time acceleration, {} instr limit...",
        time_accel, max_instr
    );

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // M-mode trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;

            if cause_code == 9 {
                // ECALL_S = SBI call
                sbi_count += 1;
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
                    if sbi_count <= 20 {
                        last_ecall_log = format!(
                            "SBI #{}: a7=0x{:02X} a6={} a0=0x{:08X} -> ({}, {})",
                            sbi_count, a7, a6, a0, ret_a0, ret_a1
                        );
                        eprintln!("[fast] {}", last_ecall_log);
                    }
                }
            } else if cause_code != 11 && mpp != 3 {
                // Forward to S-mode
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

        // Advance CLINT time by time_accel per instruction
        for _ in 0..time_accel {
            vm.bus.tick_clint();
        }
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        count += 1;

        match step_result {
            StepResult::Ebreak => {
                eprintln!("[fast] EBREAK at count={}", count);
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                // On-demand identity mapping for low addresses
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

        // Periodic status logging
        if count % 5_000_000 == 0 {
            let cur_console = vm.bus.sbi.console_output.len();
            let new_chars = cur_console - console_len_at_last_log;
            eprintln!(
                "[fast] {}M: PC=0x{:08X} SBI={} console={} (+{}) ecall={}",
                count / 1_000_000,
                vm.cpu.pc,
                sbi_count,
                cur_console,
                new_chars,
                vm.cpu.ecall_count
            );
            console_len_at_last_log = cur_console;
        }

        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[fast] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
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

        // Detect stall
        if vm.cpu.pc == last_pc {
            stall_count += 1;
            if stall_count > 100_000 {
                eprintln!(
                    "[fast] STALL DETECTED at PC=0x{:08X} after {} iterations",
                    vm.cpu.pc, stall_count
                );
                break;
            }
        } else {
            stall_count = 0;
            last_pc = vm.cpu.pc;
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
        "\n[fast] Done: {} instructions, {} SBI calls",
        count, sbi_count
    );
    eprintln!("[fast] PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("[fast] Console: {} chars", sbi_str.len());
    eprintln!(
        "[fast] mtime={}, mtimecmp={}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp
    );

    if !sbi_str.is_empty() {
        eprintln!("\n[fast] === Console output ===");
        eprintln!("{}", &sbi_str[..sbi_str.len().min(5000)]);
    } else {
        eprintln!("[fast] No console output");
    }

    // Check phys_ram_base
    let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
    eprintln!("[fast] phys_ram_base = 0x{:08X}", prb);
}
