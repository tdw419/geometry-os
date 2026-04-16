use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb_addr) = 
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instr: u64 = 500_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut post_final_satp = false;
    let mut prev_ra: u32 = 0; // Track RA across iterations

    // Circular buffer: track last 30 RA changes with full context
    let mut ra_log: Vec<(u64, u32, u32, u32, u32)> = Vec::new(); // (count, old_ra, new_ra, pc, sp)
    let max_ra_log: usize = 30;

    // Also track stores to addresses near the stack region that write 0x3FFFF000
    // We'll do this by watching the stack pointer area
    
    while count < max_instr {
        if vm.bus.sbi.shutdown_requested { break; }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            last_satp = cur_satp;
            if cur_satp == 0x80000802 {
                post_final_satp = true;
                eprintln!("[diag] Final SATP set, watching RA changes");
            }
        }

        // Track RA changes (circular buffer)
        let cur_ra = vm.cpu.x[1];
        if post_final_satp && cur_ra != prev_ra {
            ra_log.push((count, prev_ra, cur_ra, vm.cpu.pc, vm.cpu.x[2]));
            if ra_log.len() > max_ra_log {
                ra_log.remove(0);
            }
        }
        prev_ra = cur_ra;

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;

            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if cause_code == 9 && mpp != 3 {
                // ECALL_S = SBI from S-mode
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
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
                    if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        // Stop at first S-mode fault
        if matches!(step_result, StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault)
            && vm.cpu.privilege == Privilege::Supervisor 
        {
            eprintln!("[diag] First fault at count={}: PC=0x{:08X} sepc=0x{:08X}", 
                count, vm.cpu.pc, vm.cpu.csr.sepc);
            break;
        }

        count += 1;
    }

    // Print the last N RA changes leading to the fault
    eprintln!("\n[diag] === Last {} RA changes before fault ===", ra_log.len());
    for (cnt, old_ra, new_ra, pc, sp) in &ra_log {
        let marker = if *new_ra == 0x3FFFF000 { " *** BAD ***" } else { "" };
        // Translate PC to PA for disassembly reference
        let pa = if *pc >= 0xC0000000 { pc - 0xC0000000 } else { *pc };
        let inst = vm.bus.read_word(pa as u64).unwrap_or(0);
        // For compressed instructions, also read the half-word
        let inst16 = vm.bus.read_half(pa as u64).unwrap_or(0);
        eprintln!("[diag]   count={}: RA 0x{:08X} -> 0x{:08X} PC=0x{:08X} SP=0x{:08X} inst16=0x{:04X} inst32=0x{:08X}{}",
            cnt, old_ra, new_ra, pc, sp, inst16, inst, marker);
    }

    // If we found the bad RA, check what's at SP+92 at that point
    if let Some((_, _, _, pc_bad, sp_bad)) = ra_log.iter().find(|(_, _, new_ra, _, _)| *new_ra == 0x3FFFF000) {
        eprintln!("\n[diag] === Stack at corruption (SP=0x{:08X}) ===", sp_bad);
        // SP+92 is where RA was loaded from
        // Read the stack contents at PA (SP - 0xC0000000)
        let sp_pa = (*sp_bad as u64).saturating_sub(0xC0000000);
        for offset in [60i64, 64, 68, 72, 76, 80, 84, 88, 92, 96, 100, 104] {
            let addr = sp_pa + offset as u64;
            let val = vm.bus.read_word(addr).unwrap_or(0);
            eprintln!("[diag]   SP+{} (PA 0x{:08X}) = 0x{:08X}{}", 
                offset, addr, val, if val == 0x3FFFF000 { " <-- 0x3FFFF000!" } else { "" });
        }
        
        // The previous RA change should tell us who saved to the stack
        if let Some(idx) = ra_log.iter().position(|(_, _, new_ra, _, _)| *new_ra == 0x3FFFF000) {
            if idx > 0 {
                let (cnt, old_ra, new_ra, pc, sp) = &ra_log[idx - 1];
                eprintln!("\n[diag] === RA change before corruption ===");
                eprintln!("[diag]   count={}: RA 0x{:08X} -> 0x{:08X} at PC=0x{:08X} SP=0x{:08X}",
                    cnt, old_ra, new_ra, pc, sp);
            }
        }
    }
}
