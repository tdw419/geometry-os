/// Trace the instructions leading up to the first illegal instruction at mepc=0x00000004.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178000; // Stop just after first illegal
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;

    // Ring buffer for PC history
    let mut pc_history: Vec<(u64, u32)> = Vec::new();
    let history_size = 50;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested { break; }

        // Trampoline patching
        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
            eprintln!("[trace] Trampoline patched at count={}", count);
        }

        // Track SATP
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            eprintln!("[trace] SATP changed to 0x{:08X} at count={}", cur_satp, count);
        }
        last_satp = cur_satp;

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;

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
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();

                        if cause_code == 2 && count >= 177000 {
                            eprintln!("[trace] FORWARD trap at count={}: mepc=0x{:08X} cause=2 mpp={} -> stvec=0x{:08X}",
                                count, vm.cpu.csr.sepc, mpp, stvec);
                            eprintln!("[trace]   x[1](ra)=0x{:08X} x[5](t0)=0x{:08X}", vm.cpu.x[1], vm.cpu.x[5]);
                        }
                        count += 1;
                        continue;
                    }
                }

                if cause_code == 2 {
                    eprintln!("[trace] M-mode illegal at count={}: mepc=0x{:08X} mpp={} (skipping)",
                        count, vm.cpu.csr.mepc, mpp);
                }

                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
        }

        // Track PC history (only after trampoline patch)
        if trampoline_patched && count >= 177000 {
            pc_history.push((count, vm.cpu.pc));
            if pc_history.len() > history_size {
                pc_history.remove(0);
            }
        }

        let _ = vm.step();
        count += 1;
    }

    eprintln!("\n=== Last {} PC transitions before halt ===", pc_history.len());
    for (c, pc) in &pc_history {
        let insn = vm.bus.read_word(*pc as u64).unwrap_or(0);
        eprintln!("  count={} PC=0x{:08X} insn=0x{:08X}", c, pc, insn);
    }

    eprintln!("\nFinal: PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("x[0]=0x{:08X} x[1]=0x{:08X} x[2]=0x{:08X} x[5]=0x{:08X}",
        vm.cpu.x[0], vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[5]);
    eprintln!("SATP=0x{:08X} STVEC=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
}
