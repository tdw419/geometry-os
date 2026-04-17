// Diagnostic: trace the instruction that produces the bad jump to 0x804046C8.
// Run: cargo run --example boot_trace_fault

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            bootargs,
        ).expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let max_instr: u64 = 200_000;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut pc_history: Vec<(u64, u32)> = Vec::with_capacity(50);
    let mut found = false;

    use geometry_os::riscv::cpu::StepResult;

    while count < max_instr && !found {
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

        // Trap handling (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

            if cause_code != 11 {
                if mpp != 3 {
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                // ECALL_M -> SBI
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Track PC history
        if count >= 185_000 {
            pc_history.push((count, vm.cpu.pc));
            if pc_history.len() > 50 {
                pc_history.remove(0);
            }
        }

        let step_result = vm.step();
        if let StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault = step_result {
            if count >= 185_000 {
                eprintln!("[diag] FAULT at count={}: {:?}", count, step_result);
                eprintln!("  PC=0x{:08X} sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                    vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                eprintln!("  stvec=0x{:08X} satp=0x{:08X} priv={:?}", vm.cpu.csr.stvec, vm.cpu.csr.satp, vm.cpu.privilege);

                // Dump last 30 PCs
                eprintln!("\n  Last {} PCs before fault:", pc_history.len());
                for (c, pc) in &pc_history {
                    let inst = vm.bus.read_word(*pc as u64).unwrap_or(0);
                    eprintln!("    count={} PC=0x{:08X} inst=0x{:08X}", c, pc, inst);
                }

                // Dump key registers
                eprintln!("\n  Key registers:");
                for i in [5, 6, 7, 10, 11, 12, 13, 14, 15].iter() {
                    eprintln!("    x{} = 0x{:08X}", i, vm.cpu.x[*i]);
                }

                // If stval is in the 0x80xxxxxx range, check where that value came from
                let stval = vm.cpu.csr.stval;
                if stval >= 0x80000000 && stval < 0xC0000000 {
                    eprintln!("\n  stval 0x{:08X} is in the 0x80xxxxxx range (should be 0xC0xxxxxx)", stval);
                    eprintln!("  Difference from expected VA: 0x{:08X}", 0xC0000000u32.wrapping_sub(stval) as i32);
                    // Check if stval = PA + 0x80000000
                    let possible_pa = stval.wrapping_sub(0x80000000);
                    eprintln!("  If PA+0x80000000: possible PA = 0x{:08X}", possible_pa);
                    // Check what's at that PA
                    let pa_val = vm.bus.read_word(possible_pa as u64).unwrap_or(0);
                    eprintln!("  Value at PA 0x{:08X} = 0x{:08X}", possible_pa, pa_val);
                }

                // Check the excp_vect_table entry for the faulting scause
                let scause = vm.cpu.csr.scause & !(1u32 << 31);
                if scause < 32 {
                    let table_va = 0xC0C00AA4 + (scause as u64) * 4;
                    // Read via MMU (use the bus directly at the physical address)
                    let table_pa = 0x00C00AA4 + (scause as u64) * 4;
                    let entry = vm.bus.read_word(table_pa).unwrap_or(0);
                    eprintln!("\n  excp_vect_table[{}] (VA 0x{:08X}, PA 0x{:08X}) = 0x{:08X}",
                        scause, table_va, table_pa, entry);
                }

                found = true;
                break;
            }
        }

        count += 1;
    }

    if !found {
        eprintln!("[diag] No fault found in range. Final PC=0x{:08X}", vm.cpu.pc);
    }

    // Print SBI output
    if !vm.bus.sbi.console_output.is_empty() {
        eprintln!("\n[diag] SBI console output ({} bytes):", vm.bus.sbi.console_output.len());
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("{}", s);
    }
}
