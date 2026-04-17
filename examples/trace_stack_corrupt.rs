use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let mut count = 0u64;
    let mut in_second_prb = false;
    let mut first_prb_done = false;
    let mut watch_va: u32 = 0; // Stack VA to watch for writes
    let mut watch_pa: u64 = 0;

    // Run first prb_reserve call normally
    while count < 178_284 {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
            let result = vm.bus.sbi.handle_ecall(
                vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }
        vm.step();
        count += 1;
    }

    // Now we're at count 178284, about to enter second prb_reserve
    // The stack slot for ra will be at sp - 96 + 92 = sp - 4
    // sp = 0xC1401E00, so watch PA = 0xC1401DFC - 0xC0000000 = 0x01401DFC
    watch_va = 0xC1401DFC;
    watch_pa = 0x01401DFC;
    let original_val = vm.bus.read_word(watch_pa).unwrap_or(0xDEAD);
    eprintln!("Watching VA 0x{:08X} (PA 0x{:08X}), current value = 0x{:08X}", watch_va, watch_pa as u32, original_val);

    // Now step one instruction at a time, checking the watched address after each store
    while count < 178_510 {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S || cause_code != csr::CAUSE_ECALL_M {
                if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint,
                    );
                    if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                } else {
                    let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                    if mpp != 3 {
                        let stvec = vm.cpu.csr.stvec & !0x3u32;
                        if stvec != 0 {
                            vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                            vm.cpu.csr.scause = mcause;
                            vm.cpu.csr.stval = vm.cpu.csr.mtval;
                            vm.cpu.pc = stvec;
                            vm.cpu.privilege = Privilege::Supervisor;
                            vm.cpu.tlb.flush_all();
                            count += 1;
                            continue;
                        }
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
        }

        let pc_before = vm.cpu.pc;
        vm.step();

        // Check if the watched address changed
        let current_val = vm.bus.read_word(watch_pa).unwrap_or(0xDEAD);
        if current_val != original_val && current_val == 0x3FFFF000 {
            eprintln!("[{}] WRITE DETECTED at PA 0x{:08X}: now = 0x{:08X}, was 0x{:08X}", 
                count, watch_pa as u32, current_val, original_val);
            eprintln!("  PC was 0x{:08X} before this step, now 0x{:08X}", pc_before, vm.cpu.pc);
            for i in 0..32 {
                if vm.cpu.x[i] != 0 {
                    eprintln!("  x{} = 0x{:08X}", i, vm.cpu.x[i]);
                }
            }
            break;
        }

        count += 1;
    }
    eprintln!("Done at count={}", count);
}
