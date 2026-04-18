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

    while count < 178_505 {
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
        vm.step();
        count += 1;
    }

    // PA = VA - 0xC0000000
    eprintln!("=== Raw bytes at PA 0x3F9D0 (VA 0xC003F9D0) ===");
    for off in (0..64).step_by(2) {
        let pa = 0x3F9D0u64 + off as u64;
        // Read byte-by-byte to avoid endianness issues
        let b0 = vm.bus.read_byte(pa).unwrap_or(0xFF);
        let b1 = vm.bus.read_byte(pa + 1).unwrap_or(0xFF);
        let half = (b1 as u16) << 8 | b0 as u16;
        let va = 0xC003F9D0u32 + off;
        eprintln!(
            "  VA 0x{:08X} PA 0x{:07X}: 0x{:04X} [b0=0x{:02X} b1=0x{:02X}]",
            va, pa, half, b0, b1
        );
    }

    eprintln!("\n=== Registers ===");
    for i in 0..32 {
        if vm.cpu.x[i] != 0 {
            eprintln!("  x{} = 0x{:08X}", i, vm.cpu.x[i]);
        }
    }
}
