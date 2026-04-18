/// Diagnostic: add a write watchpoint at UART base (0x10000000) to see if
/// the kernel ever writes to the UART MMIO registers.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Set a write watchpoint at UART base
    vm.bus.write_watch_addr = Some(0x1000_0000);

    let max = 20_000_000u64;
    let mut count: u64 = 0;
    let mut uart_write_count: u64 = 0;
    let mut last_report: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

            if cause_code == 9 || cause_code == 11 {
                // ECALL_S or ECALL_M -> SBI
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
                println!(
                    "[diag] ECALL cause={} a7=0x{:X} a6=0x{:X} at count={} PC=0x{:08X}",
                    cause_code, vm.cpu.x[17], vm.cpu.x[16], count, vm.cpu.csr.mepc
                );
            } else if mpp != 3 {
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
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Check for UART write watchpoint hit
        if vm.bus.write_watch_hit {
            uart_write_count += 1;
            println!(
                "[diag] UART write #{}: val=0x{:08X} at count={} PC=0x{:08X} priv={:?}",
                uart_write_count, vm.bus.write_watch_val, count, vm.cpu.pc, vm.cpu.privilege
            );
            // Reset watchpoint for next hit
            vm.bus.write_watch_hit = false;
        }

        let _ = vm.step();

        // Periodic report
        if count - last_report >= 2_000_000 {
            println!(
                "[diag] count={} PC=0x{:08X} priv={:?} uart_writes={} tx_buf={}",
                count,
                vm.cpu.pc,
                vm.cpu.privilege,
                uart_write_count,
                vm.bus.uart.tx_buf.len()
            );
            last_report = count;
        }

        count += 1;
    }

    println!("\n[diag] === FINAL ===");
    println!(
        "Instructions: {} UART writes: {} TX buf: {} chars",
        count,
        uart_write_count,
        vm.bus.uart.tx_buf.len()
    );
    println!(
        "PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );

    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("\nUART output:\n{}", preview);
    }
}
