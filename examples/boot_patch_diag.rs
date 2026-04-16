use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256,
            "console=ttyS0 earlycon=sbi panic=1 quiet").unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 500_000u64;
    let mut count: u64 = 0;
    let mut last_satp = vm.cpu.csr.satp;
    let mut last_stvec = vm.cpu.csr.stvec;
    let mut satp_changes = 0u32;
    let mut stvec_changes = 0u32;

    while count < max_count {
        // Check for SBI shutdown
        if vm.bus.sbi.shutdown_requested { break; }

        // Detect trap at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
            
            if cause_code == 9 { // ECALL_S
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
                // Forward to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 1)) | 0;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Watch SATP and stvec changes
        if vm.cpu.csr.satp != last_satp {
            satp_changes += 1;
            println!("[SATP CHANGE #{}] count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                satp_changes, count, last_satp, vm.cpu.csr.satp, vm.cpu.pc);
            last_satp = vm.cpu.csr.satp;
        }
        if vm.cpu.csr.stvec != last_stvec {
            stvec_changes += 1;
            println!("[STVEC CHANGE #{}] count={}: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                stvec_changes, count, last_stvec, vm.cpu.csr.stvec, vm.cpu.pc);
            last_stvec = vm.cpu.csr.stvec;
        }

        let _ = vm.step();
        count += 1;

        // Spin detection
        if count % 100_000 == 0 {
            println!("Progress: count={} PC=0x{:08X} SP=0x{:08X} SATP=0x{:08X} stvec=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[2], vm.cpu.csr.satp, vm.cpu.csr.stvec);
        }
    }

    let uart_bytes = vm.bus.uart.drain_tx();
    let uart_str: String = uart_bytes.iter().map(|&b| b as char).collect();
    println!("\n=== DONE === count={} SATP_changes={} STVEC_changes={}", 
        count, satp_changes, stvec_changes);
    println!("UART: {}", &uart_str[..uart_str.len().min(500)]);
}
