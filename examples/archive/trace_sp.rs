use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;
use geometry_os::riscv::RiscvVm;

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

    while count < 200_000 {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
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
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
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
                vm.cpu.x[17],
                vm.cpu.x[16],
                vm.cpu.x[10],
                vm.cpu.x[11],
                vm.cpu.x[12],
                vm.cpu.x[13],
                vm.cpu.x[14],
                vm.cpu.x[15],
                &mut vm.bus.uart,
                &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result {
                vm.cpu.x[10] = a0;
                vm.cpu.x[11] = a1;
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }

        // When entering vprintk_store (0xC003B5BC)
        if vm.cpu.pc == 0xC003B5BC {
            eprintln!(
                "[ENTRY] vprintk_store at count={}, sp=0x{:08X}",
                count, vm.cpu.x[2]
            );
        }

        // After c.addi16sp sp,-144 (which is 0xC003B5BC, the first instruction)
        // s0 = sp + 144 after the next instruction at 0xC003B5C2
        if vm.cpu.pc == 0xC003B5C2 {
            eprintln!(
                "[POST-ALLOC] sp=0x{:08X}, s0 should be sp+144=0x{:08X}, actual s0=0x{:08X}",
                vm.cpu.x[2],
                vm.cpu.x[2].wrapping_add(144),
                vm.cpu.x[8]
            );
        }

        // First call to prb_reserve
        if vm.cpu.pc == 0xC003B716 {
            eprintln!(
                "[1ST PRB] count={}, sp=0x{:08X}, s0=0x{:08X}, a0=0x{:08X}",
                count, vm.cpu.x[2], vm.cpu.x[8], vm.cpu.x[10]
            );
        }

        // Second call to prb_reserve
        if vm.cpu.pc == 0xC003B754 {
            eprintln!(
                "[2ND PRB] count={}, sp=0x{:08X}, s0=0x{:08X}, a0=0x{:08X}",
                count, vm.cpu.x[2], vm.cpu.x[8], vm.cpu.x[10]
            );
            eprintln!(
                "  a0 = s0-92 = 0x{:08X}-92 = 0x{:08X}",
                vm.cpu.x[8],
                vm.cpu.x[8].wrapping_sub(92)
            );
        }

        // When vprintk_store calls other functions, check sp
        // vsnprintf call at 0xC003B682
        if vm.cpu.pc == 0xC003B682 {
            eprintln!("[VSNPRINTF] count={}, sp=0x{:08X}", count, vm.cpu.x[2]);
        }

        // prb_reserve_in_last call at 0xC003B6FA
        if vm.cpu.pc == 0xC003B6FA {
            eprintln!("[PRB_IN_LAST] count={}, sp=0x{:08X}", count, vm.cpu.x[2]);
        }

        vm.step();
        count += 1;
    }
}
