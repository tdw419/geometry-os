use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256,
            "console=ttyS0 earlycon=sbi panic=1 quiet").unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 5_000_000u64;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;
    let mut satp_changes = 0u32;
    let mut trace_mode = false;
    let mut trace_remaining = 0u32;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        // Watch for SATP changes
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            println!("\n[SATP #{}] count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X} SP=0x{:08X}",
                satp_changes, count, last_satp, vm.cpu.csr.satp, vm.cpu.pc, vm.cpu.x[2]);
            last_satp = vm.cpu.csr.satp;
            trace_mode = true;
            trace_remaining = 50;
        }

        // Trap forwarding (simplified)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }

        // Trace around SATP changes
        if trace_mode && trace_remaining > 0 {
            let instr = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            println!("  [{:3}] PC=0x{:08X} instr=0x{:08X} SP=0x{:08X} RA=0x{:08X}",
                trace_remaining, vm.cpu.pc, instr, vm.cpu.x[2], vm.cpu.x[1]);
            trace_remaining -= 1;
            if trace_remaining == 0 { trace_mode = false; }
        }

        match vm.step() {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if trace_remaining == 0 && satp_changes <= 6 {
                    println!("  [FAULT] count={} PC=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                        count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                }
            }
            _ => {}
        }

        count += 1;
    }
    println!("\nFinal: count={} satp_changes={}", count, satp_changes);
}
