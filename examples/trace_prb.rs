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
    let mut in_prb_reserve = false;
    let mut prb_entry_count = 0u64;
    let mut prb_entry_ra: u32 = 0;
    let mut prb_entry_sp: u32 = 0;

    while count < 180_000 {
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

        // Detect entry into prb_reserve
        if vm.cpu.pc == 0xC003F8AA && !in_prb_reserve {
            in_prb_reserve = true;
            prb_entry_count = count;
            prb_entry_ra = vm.cpu.x[1];
            prb_entry_sp = vm.cpu.x[2];
            eprintln!(
                "[CALL {}] prb_reserve at count={}, ra=0x{:08X}, sp=0x{:08X}",
                prb_entry_count, count, prb_entry_ra, prb_entry_sp
            );
            // Also show the stack content at sp+92 (where ra will be saved)
            let saved_ra_addr = vm.cpu.x[2].wrapping_add(92);
            eprintln!("  Stack[sp+92] before save = PA check needed");
        }

        // Detect C.JR ra at 0xC003F9E6 (return from prb_reserve)
        if vm.cpu.pc == 0xC003F9E6 && in_prb_reserve {
            eprintln!(
                "[RET {}] prb_reserve returning at count={}, ra=0x{:08X}",
                count, count, vm.cpu.x[1]
            );
            if vm.cpu.x[1] != prb_entry_ra {
                eprintln!(
                    "  *** RA CHANGED! Entry ra=0x{:08X}, now ra=0x{:08X} ***",
                    prb_entry_ra, vm.cpu.x[1]
                );
                // Check what's on the stack
                let sp = vm.cpu.x[2];
                // Read from physical memory (PA = VA - 0xC0000000 for kernel addresses)
                let pa_base = sp.wrapping_sub(4); // After addi16sp sp,96, sp should be original
                eprintln!("  sp=0x{:08X}, checking saved ra on stack:", sp);
                // Actually sp was already adjusted by addi16sp, so sp+92 was where ra was
                // But after addi16sp sp,96, sp is back to original value
                // The save was at original_sp - 96 + 92 = original_sp - 4
                // After restore, sp = original_sp, so saved_ra was at sp - 4
                let saved_ra_pa = (sp as u64).wrapping_sub(4);
                // Check if this is a kernel VA
                if saved_ra_pa >= 0xC0000000 {
                    let phys = saved_ra_pa - 0xC0000000;
                    let val = vm.bus.read_word(phys).unwrap_or(0xDEAD);
                    eprintln!(
                        "  Stack[sp-4] = VA 0x{:08X} PA 0x{:08X} = 0x{:08X}",
                        saved_ra_pa as u32, phys as u32, val
                    );
                }
            }
            in_prb_reserve = false;
        }

        vm.step();
        count += 1;
    }
    eprintln!("Done at count={}", count);
}
