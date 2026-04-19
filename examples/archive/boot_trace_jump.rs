// Diagnostic: trace the instruction that produces the jump to 0x804046C8.
// Run: cargo run --example boot_trace_jump

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        bootargs,
    )
    .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    // Run until 186555 (the known fault count) minus some margin
    let max_instr: u64 = 186_560;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_pc: u32 = 0;

    // Track the last 5 PC transitions before the fault
    let mut pc_history: Vec<(u64, u32, u32)> = Vec::with_capacity(10);

    use geometry_os::riscv::cpu::StepResult;

    while count < max_instr {
        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(addr, 0xCF | (l1_idx << 20)).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        // Handle trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 4;

            if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1u32 << 1);
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
            // Skip faulting instruction
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        match step_result {
            StepResult::FetchFault => {
                let pc = vm.cpu.pc;
                let sepc = vm.cpu.csr.sepc;
                let stval = vm.cpu.csr.stval;
                eprintln!(
                    "[fault] count={} FETCH_FAULT: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                    count, pc, sepc, stval
                );
                // Dump the last few PC transitions
                eprintln!("[fault] Last {} PC transitions:", pc_history.len().min(5));
                for (c, old_pc, new_pc) in pc_history.iter().rev().take(5) {
                    // Read instruction at old_pc
                    let instr = vm.bus.read_word(*old_pc as u64).unwrap_or(0);
                    eprintln!(
                        "  count={} PC: 0x{:08X} -> 0x{:08X} (instr=0x{:08X})",
                        c, old_pc, new_pc, instr
                    );
                }
                // Read instruction at the address BEFORE sepc (the branch that caused the jump)
                let prev_instr_addr = sepc.wrapping_sub(4);
                let prev_instr = vm.bus.read_word(prev_instr_addr as u64).unwrap_or(0);
                eprintln!(
                    "[fault] Instruction before sepc: addr=0x{:08X} instr=0x{:08X}",
                    prev_instr_addr, prev_instr
                );
                // Also check 2 bytes before (compressed instruction)
                let prev_half = vm.bus.read_half(prev_instr_addr as u64).unwrap_or(0);
                eprintln!(
                    "[fault] Halfword before sepc: addr=0x{:08X} half=0x{:04X}",
                    prev_instr_addr, prev_half
                );
                // Check what's in registers
                eprintln!("[fault] Registers: a0=0x{:08X} a1=0x{:08X} ra=0x{:08X} sp=0x{:08X} gp=0x{:08X} tp=0x{:08X} t0=0x{:08X}",
                    vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4], vm.cpu.x[5]);
                break;
            }
            StepResult::LoadFault | StepResult::StoreFault => {
                // S-mode page faults during boot are expected, continue
                if count > 186_500 {
                    eprintln!(
                        "[fault] count={} {:?}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                        count, step_result, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval
                    );
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }

        // Track PC transitions
        let cur_pc = vm.cpu.pc;
        if cur_pc != last_pc {
            pc_history.push((count, last_pc, cur_pc));
            if pc_history.len() > 10 {
                pc_history.remove(0);
            }
            last_pc = cur_pc;
        }

        count += 1;
    }

    // Final state
    eprintln!("\n=== Final State ===");
    eprintln!(
        "count={} PC=0x{:08X} priv={:?}",
        count, vm.cpu.pc, vm.cpu.privilege
    );
    eprintln!("SATP=0x{:08X} SP=0x{:08X}", vm.cpu.csr.satp, vm.cpu.x[2]);
}
