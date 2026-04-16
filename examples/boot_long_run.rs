/// Run for 50M instructions, check for ECALLs and timer interrupts.
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

    let max = 50_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut trap_count: u64 = 0;
    let mut sbi_m_count: u64 = 0;
    let mut interrupt_count: u64 = 0;
    let mut report_interval = 5_000_000u64;
    let mut next_report = report_interval;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[boot] Shutdown at count={}", count);
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                sbi_m_count += 1;
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
                        trap_count += 1;
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let prev_ecall = vm.cpu.ecall_count;
        vm.step();
        if vm.cpu.ecall_count > prev_ecall {
            println!("[boot] ECALL #{} at count={} PC_before=0x{:08X} a7=0x{:X} priv={:?}",
                vm.cpu.ecall_count, count,
                vm.cpu.last_step.as_ref().map(|ls| ls.pc).unwrap_or(0),
                vm.cpu.x[17], vm.cpu.privilege);
        }

        if vm.cpu.csr.satp != last_satp {
            println!("[boot] SATP: 0x{:08X} -> 0x{:08X} at count={}", last_satp, vm.cpu.csr.satp, count);
            last_satp = vm.cpu.csr.satp;
        }

        // Count S-mode timer interrupts (scause bit 31 set, cause 5 = timer)
        if let Some(ref ls) = vm.cpu.last_step {
            // Detect interrupt delivery by checking if PC jumped to stvec
            // This is a rough heuristic
        }

        count += 1;
        if count >= next_report {
            println!("[boot] count={} PC=0x{:08X} ecall={} traps_m={} fw_trap={} uart={} sbi_out={}",
                count, vm.cpu.pc, vm.cpu.ecall_count, sbi_m_count, trap_count,
                vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
            next_report += report_interval;
        }
    }

    println!("\n[boot] FINAL: count={} ecall_count={} fw_traps={} uart={} sbi_out={}",
        count, vm.cpu.ecall_count, trap_count, 
        vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
    
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(3000).collect();
        println!("\n[boot] SBI console output:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(3000).collect();
        println!("\n[boot] UART output:\n{}", preview);
    }
}
