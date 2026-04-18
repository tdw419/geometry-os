use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut fault_count: u64 = 0;
    let mut next_report: u64 = 1_000_000;

    while count < 5_000_000 {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[boot] SBI shutdown at count={}", count);
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                sbi_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 {
                            vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                        }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let step_result = vm.step();

        if count >= next_report {
            eprintln!("[progress] {}M: PC=0x{:08X} SP=0x{:08X} SBI={} faults={}",
                count / 1_000_000, vm.cpu.pc, vm.cpu.x[2], sbi_count, fault_count);
            next_report += 1_000_000;
        }

        match step_result {
            StepResult::Ebreak => {
                eprintln!("[boot] EBREAK at count={}", count);
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                fault_count += 1;
                if fault_count <= 5 {
                    let ft = match step_result {
                        StepResult::FetchFault => "FETCH",
                        StepResult::LoadFault => "LOAD",
                        _ => "STORE",
                    };
                    eprintln!("[fault] #{} count={}: {} PC=0x{:08X} stval=0x{:08X}",
                        fault_count, count, ft, vm.cpu.pc, vm.cpu.csr.stval);
                }
            }
            _ => {}
        }
        count += 1;
    }
    eprintln!("[boot] Done: count={} SBI_calls={} faults={}", count, sbi_count, fault_count);
    
    let uart_buf = &vm.bus.uart.tx_buf;
    if !uart_buf.is_empty() {
        let text: String = uart_buf.iter().filter(|&&c| c >= 0x20 && c < 0x7F).map(|&c| c as char).collect();
        eprintln!("[boot] UART ({} bytes): {}", uart_buf.len(), &text[..text.len().min(2000)]);
    } else {
        eprintln!("[boot] No UART output");
    }
}
