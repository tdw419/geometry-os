use geometry_os::riscv::cpu::{Privilege, StepResult};
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

    let max_instructions = 200_000u64;
    let mut count = 0u64;
    let mut last_10_pcs: Vec<(u64, u32, Privilege)> = Vec::new();

    while count < max_instructions {
        // Firmware handler
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

        let pc_before = vm.cpu.pc;
        last_10_pcs.push((count, pc_before, vm.cpu.privilege));
        if last_10_pcs.len() > 20 {
            last_10_pcs.remove(0);
        }

        let step_result = vm.step();

        // Check if we jumped to 0x3FFFF000
        if vm.cpu.pc >= 0x3FF00000 && vm.cpu.pc < 0x40000000 {
            eprintln!(
                "\n=== JUMPED TO HIGH ADDRESS 0x{:08X} at count={} ===",
                vm.cpu.pc, count
            );
            eprintln!("Last 20 PCs before this:");
            for (c, pc, priv_) in &last_10_pcs {
                eprintln!("  count={} PC=0x{:08X} priv={:?}", c, pc, priv_);
            }
            eprintln!("\nRegister state:");
            for i in 0..32 {
                if vm.cpu.x[i] != 0 {
                    eprintln!("  x{} = 0x{:08X}", i, vm.cpu.x[i]);
                }
            }
            eprintln!(
                "  scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
            );
            eprintln!("  satp=0x{:08X}", vm.cpu.csr.satp);
            break;
        }

        count += 1;
    }
    eprintln!(
        "\nDone at count={}. Console: {} bytes",
        count,
        vm.bus.sbi.console_output.len()
    );
}
