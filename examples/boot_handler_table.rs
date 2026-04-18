use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::csr;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run to 177500
    for i in 0..177500 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            handle_trap(&mut vm);
            continue;
        }
        vm.step();
    }

    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) << 12;
    eprintln!(
        "At 177500: PC=0x{:08X} satp=0x{:08X} pg_dir=0x{:08X}",
        vm.cpu.pc, satp, pg_dir_phys
    );

    // Check L1[773] mapping for VA 0xC1400000+
    let l1_773 = vm.bus.read_word(pg_dir_phys + 773 * 4).unwrap_or(0);
    eprintln!("L1[773] = 0x{:08X} leaf={}", l1_773, (l1_773 & 0xE) != 0);

    // Read data at PA 0x01400A00-0x01400C00 (the exception handler table area)
    eprintln!("\nData at PA 0x01400A00-0x01400C00:");
    for off in (0xA00..0xC00).step_by(4) {
        let val = vm.bus.read_word(0x01400000 + off as u64).unwrap_or(0);
        if val != 0 {
            eprintln!("  PA 0x{:08X}: 0x{:08X}", 0x01400000 + off, val);
        }
    }

    // Now trace to the first transition
    eprintln!("\nTracing to fault...");
    for i in 0..3000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc_before = vm.cpu.pc;
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            handle_trap(&mut vm);
            continue;
        }
        let result = vm.step();

        if pc_before >= 0xC0000000 && vm.cpu.pc < 0xC0000000 && vm.cpu.pc != fw_addr_u32 {
            eprintln!("\n*** TRANSITION at 177500+{} ***", i);
            eprintln!("PC: 0x{:08X} -> 0x{:08X}", pc_before, vm.cpu.pc);
            eprintln!(
                "RA=0x{:08X} SP=0x{:08X} T0=0x{:08X} T1=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[5], vm.cpu.x[6]
            );
            eprintln!(
                "A0=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[3], vm.cpu.x[4]
            );
            // Read instruction at transition PC
            let inst = vm.bus.read_word(pc_before as u64).unwrap_or(0);
            eprintln!("Inst: 0x{:08X} opcode=0x{:02X}", inst, inst & 0x7F);
            break;
        }
        if let StepResult::FetchFault = result {
            if vm.cpu.privilege == Privilege::Supervisor {
                eprintln!("\n*** FETCH FAULT at 177500+{} ***", i);
                eprintln!(
                    "PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} scause=0x{:08X}",
                    vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.scause
                );
                eprintln!(
                    "RA=0x{:08X} T0=0x{:08X} T1=0x{:08X} SP=0x{:08X}",
                    vm.cpu.x[1], vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[2]
                );
                break;
            }
        }
    }
}

fn handle_trap(vm: &mut RiscvVm) {
    let mcause = vm.cpu.csr.mcause;
    let cause_code = mcause & !(1u32 << 31);
    if cause_code == csr::CAUSE_ECALL_M {
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
    } else {
        let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
        if cause_code == csr::CAUSE_ECALL_S {
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
        } else if mpp != 3 {
            let stvec = vm.cpu.csr.stvec & !0x3u32;
            if stvec != 0 {
                vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                vm.cpu.csr.scause = vm.cpu.csr.mcause;
                vm.cpu.csr.stval = vm.cpu.csr.mtval;
                let spp = if mpp == 1 { 1u32 } else { 0u32 };
                vm.cpu.csr.mstatus =
                    (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                vm.cpu.csr.mstatus =
                    (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                vm.cpu.pc = stvec;
                vm.cpu.privilege = Privilege::Supervisor;
                vm.cpu.tlb.flush_all();
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        } else {
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
    }
}
