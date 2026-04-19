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

    while count < 178_510 {
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

        // When the second call to prb_reserve happens
        // 0xC003B754: jal ra,c003f8aa <prb_reserve>
        if vm.cpu.pc == 0xC003B754 {
            eprintln!("[SECOND CALL] About to call prb_reserve");
            eprintln!("  a0=0x{:08X} (output struct ptr)", vm.cpu.x[10]);
            eprintln!("  a1=0x{:08X}", vm.cpu.x[11]);
            eprintln!("  a2=0x{:08X}", vm.cpu.x[12]);
            eprintln!("  sp=0x{:08X}", vm.cpu.x[2]);
            eprintln!("  s0=0x{:08X} (vprintk_store fp)", vm.cpu.x[8]);
            eprintln!("  s9=0x{:08X}", vm.cpu.x[25]);
            // Check: a0 + 8 should NOT overlap with prb_reserve's saved ra
            // prb_reserve will adjust sp by -96, saved ra at sp_new + 92
            let future_sp_new = vm.cpu.x[2] - 96;
            let saved_ra_addr = future_sp_new + 92;
            eprintln!(
                "  After prb_reserve entry: sp_new=0x{:08X}, saved_ra at 0x{:08X}",
                future_sp_new, saved_ra_addr
            );
            eprintln!("  a0+0 = 0x{:08X}", vm.cpu.x[10]);
            eprintln!("  a0+8 = 0x{:08X}", vm.cpu.x[10] + 8);
            eprintln!(
                "  OVERLAP: a0+8 == saved_ra? {}",
                vm.cpu.x[10] + 8 == saved_ra_addr
            );
        }

        vm.step();
        count += 1;
    }
}
