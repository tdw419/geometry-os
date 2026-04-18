use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let (mut vm, fw_addr, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();
    let fw = fw_addr as u32;

    let max = 200_000u64;
    let mut count = 0u64;

    while count < max {
        if vm.cpu.pc == fw && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let code = mcause & !(1u32 << 31);
            if code == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if code != csr::CAUSE_ECALL_M {
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
            let r = vm.bus.sbi.handle_ecall(
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
            if let Some((a0, a1)) = r {
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

    // Disassemble the hot loop region
    eprintln!("=== Disassembly at hot PCs (PA = VA - 0xC0000000) ===");
    for base_va in [0xC00010A0u32, 0xC0002780u32, 0xC020B0C0u32] {
        let base_pa = base_va.wrapping_sub(0xC0000000);
        eprintln!("\n--- VA 0x{:08X} (PA 0x{:08X}) ---", base_va, base_pa);
        for off in (0..48).step_by(2) {
            let pa = (base_pa + off) as u64;
            let b0 = vm.bus.read_byte(pa).unwrap_or(0xFF);
            let b1 = vm.bus.read_byte(pa + 1).unwrap_or(0xFF);
            let half = (b1 as u16) << 8 | b0 as u16;
            let va = base_va + off;

            // Decode compressed vs regular
            let is_compressed = (half & 0x3) != 0x3;
            if is_compressed {
                eprintln!("  0x{:08X}: 0x{:04X}       (compressed)", va, half);
            } else {
                let b2 = vm.bus.read_byte(pa + 2).unwrap_or(0xFF);
                let b3 = vm.bus.read_byte(pa + 3).unwrap_or(0xFF);
                let word = (b3 as u32) << 24 | (b2 as u32) << 16 | (b1 as u32) << 8 | b0 as u32;
                eprintln!("  0x{:08X}: 0x{:08X} (32-bit)", va, word);
            }
        }
    }

    // Also check: what's the instruction at 0xC00010B2?
    eprintln!("\n=== Registers at count {} ===", count);
    for i in 0..32 {
        if vm.cpu.x[i] != 0 {
            eprintln!("  x{} = 0x{:08X}", i, vm.cpu.x[i]);
        }
    }
    eprintln!("  pc  = 0x{:08X}", vm.cpu.pc);
    eprintln!("  satp= 0x{:08X}", vm.cpu.csr.satp);
}
