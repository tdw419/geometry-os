/// Detect when kernel enters die_kernel_fault (0xC0002350) and log the state.
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

    let max = 10_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut die_fault_count: u64 = 0;
    let mut ecall_count: u64 = 0;

    // Key addresses (from objdump)
    let die_kernel_fault: u32 = 0xC0002350;
    let _printk: u32 = 0xC000300E;
    let sbi_ecall: u32 = 0xC0000068; // approximate, need to find actual ECALL location

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                ecall_count += 1;
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
        vm.step();

        // Detect entry into die_kernel_fault
        if vm.cpu.pc == die_kernel_fault && die_fault_count < 5 {
            die_fault_count += 1;
            println!("[fault] die_kernel_fault #{} at count={} (ecall_count={})",
                die_fault_count, count, vm.cpu.ecall_count);
            // Print register state
            println!("[fault]   ra=0x{:08X} sp=0x{:08X} gp=0x{:08X} tp=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]);
            println!("[fault]   a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]);
            println!("[fault]   sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
            println!("[fault]   satp=0x{:08X} mstatus=0x{:08X} mip=0x{:08X}",
                vm.cpu.csr.satp, vm.cpu.csr.mstatus, vm.cpu.csr.mip);
        }

        // Detect ECALL
        if vm.cpu.ecall_count > prev_ecall && ecall_count < 5 {
            println!("[ecall] ECALL #{} at count={} PC=0x{:08X} a7=0x{:X} a0=0x{:X}",
                vm.cpu.ecall_count, count,
                vm.cpu.last_step.as_ref().map(|ls| ls.pc).unwrap_or(0),
                vm.cpu.x[17], vm.cpu.x[10]);
        }

        count += 1;
    }

    println!("\n[boot] Done: count={} die_faults={} cpu_ecalls={} fw_ecalls={}",
        count, die_fault_count, vm.cpu.ecall_count, ecall_count);
    println!("[boot] UART: {} chars, SBI console: {} chars",
        vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
}
