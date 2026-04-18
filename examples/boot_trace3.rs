use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::collections::HashSet;

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
    let max_instr = 5_000_000u64;
    let time_accel = 100u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_ecall = 0u64;

    // Track unique function entries (jumps to new high addresses after ret)
    let mut seen_functions: HashSet<u32> = HashSet::new();
    let mut last_was_ret = false;
    let mut in_udelay = false;
    let mut udelay_entries: u64 = 0;
    let mut non_udelay_instrs: u64 = 0;
    let mut last_100_pcs: Vec<u32> = Vec::new();
    let mut sample_idx: u64 = 0;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        let cur_ecall = vm.cpu.ecall_count;
        if cur_ecall != last_ecall {
            eprintln!(
                "[boot] ECALL #{} at count={}: a7=0x{:X} a6=0x{:X} a0=0x{:X}",
                cur_ecall, count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
            );
            last_ecall = cur_ecall;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
            if cause_code == 9 {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
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

        // Track if we're in udelay
        let pc = vm.cpu.pc;
        in_udelay = pc >= 0xC020B0D2 && pc <= 0xC020B136;
        if in_udelay && pc == 0xC020B0D2 {
            udelay_entries += 1;
        }

        // Track non-udelay instructions
        if !in_udelay {
            non_udelay_instrs += 1;
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

        // Sample PCs every 100K instructions after ECALL
        if count > 182000 && count % 100_000 == 0 && sample_idx < 50 {
            last_100_pcs.push(vm.cpu.pc);
            sample_idx += 1;
        }
    }

    eprintln!(
        "[diag] udelay_entries={} non_udelay_instrs={}",
        udelay_entries, non_udelay_instrs
    );
    eprintln!("[diag] Sampled PCs (every 100K after ECALL):");
    for (i, pc) in last_100_pcs.iter().enumerate() {
        eprintln!(
            "  [{}] 0x{:08X}{}",
            i,
            pc,
            if *pc >= 0xC020B0D2 && *pc <= 0xC020B136 {
                " (udelay)"
            } else {
                ""
            }
        );
    }
    eprintln!("[diag] console={}", vm.bus.sbi.console_output.len());
    eprintln!("[diag] ECALLs={}", vm.cpu.ecall_count);
}
