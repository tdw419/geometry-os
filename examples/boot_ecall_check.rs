/// Diagnostic: Check if the kernel makes any ECALL/SBI calls.
/// Also check console_output from SBI and UART tx_buf.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 10_000_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u64 = 0;
    let mut trap_count: u64 = 0;
    let mut last_report: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown requested at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 11 {
                // ECALL_M -> SBI call (shouldn't happen in S-mode, but handle anyway)
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        trap_count += 1;
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // SATP change detection + fixup (same as boot_linux)
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            println!("[diag] SATP change #{}: 0x{:08X} -> 0x{:08X} at count={} PC=0x{:08X}",
                satp_changes, last_satp, cur_satp, count, vm.cpu.pc);
            // Note: In boot_linux(), SATP changes trigger fixup_kernel_page_table
            // and identity mapping injection. This diagnostic doesn't do that,
            // but the boot page table already has the right mappings.
            last_satp = cur_satp;
        }

        let _step_result = vm.step();

        count += 1;

        if count - last_report >= 1_000_000 {
            println!("[diag] count={} PC=0x{:08X} priv={:?} ecall_count={} \
                     sbi_console={} uart_tx={} traps={} satp_changes={}",
                count, vm.cpu.pc, vm.cpu.privilege,
                vm.cpu.ecall_count,
                vm.bus.sbi.console_output.len(),
                vm.bus.uart.tx_buf.len(),
                trap_count, satp_changes);
            last_report = count;

            // If we got any output, show it
            if !vm.bus.sbi.console_output.is_empty() {
                let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
                let preview: String = s.chars().take(500).collect();
                println!("[diag] SBI console output:\n{}", preview);
            }
            if !vm.bus.uart.tx_buf.is_empty() {
                let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
                let preview: String = s.chars().take(500).collect();
                println!("[diag] UART tx_buf:\n{}", preview);
            }
        }
    }

    println!("\n[diag] Final: count={} PC=0x{:08X} priv={:?} ecall_count={}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.ecall_count);
    println!("[diag] SBI console_output: {} bytes", vm.bus.sbi.console_output.len());
    println!("[diag] UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("[diag] SBI output:\n{}", s);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("[diag] UART output:\n{}", s);
    }

    // Show key registers
    println!("[diag] SP=0x{:08X} GP=0x{:08X} TP=0x{:08X} RA=0x{:08X}",
        vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4], vm.cpu.x[1]);
    println!("[diag] scause=0x{:08X} sepc=0x{:08X} stvec=0x{:08X} sstatus=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stvec, vm.cpu.csr.mstatus);
}
