/// Check if ECALLs from S-mode are happening and what a7 contains.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 5_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_ecall_count: u64 = 0;
    let mut ecall_a7_log: Vec<(u64, u32, u32, u32)> = Vec::new(); // (count, pc, a7, a0)
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M -> SBI call
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
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
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

        let prev_ecall = vm.cpu.ecall_count;
        let prev_sbi = vm.bus.sbi.console_output.len();
        let prev_uart = vm.bus.uart.tx_buf.len();

        vm.step();

        // Detect ECALL events
        if vm.cpu.ecall_count > prev_ecall {
            let a7 = vm.cpu.x[17];
            let a0 = vm.cpu.x[10];
            ecall_a7_log.push((count, vm.cpu.pc, a7, a0));
        }

        // Check for SATP changes
        if vm.cpu.csr.satp != last_satp {
            println!("[diag] SATP: 0x{:08X} -> 0x{:08X} at count={}", last_satp, vm.cpu.csr.satp, count);
            last_satp = vm.cpu.csr.satp;
        }

        count += 1;
    }

    println!("\n[diag] Total ECALLs (cpu.ecall_count): {}", vm.cpu.ecall_count);
    println!("[diag] SBI console_output len: {}", vm.bus.sbi.console_output.len());
    println!("[diag] UART tx_buf len: {}", vm.bus.uart.tx_buf.len());

    if !ecall_a7_log.is_empty() {
        println!("\n[diag] First 30 ECALLs:");
        for (cnt, pc, a7, a0) in ecall_a7_log.iter().take(30) {
            println!("  count={} pc=0x{:08X} a7=0x{:X} a0=0x{:X}", cnt, pc, a7, a0);
        }
    } else {
        println!("[diag] NO ECALLs detected in {} instructions!", max);
    }

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(2000).collect();
        println!("\n[diag] SBI console output:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("\n[diag] UART output:\n{}", preview);
    }
}
