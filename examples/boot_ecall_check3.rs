use geometry_os::riscv::cpu::Privilege;
/// Detailed PC trace after SATP change to understand kernel boot flow.
use geometry_os::riscv::RiscvVm;

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
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut prev_ecall_count: u64 = 0;
    let mut prev_uart_len: usize = 0;
    let mut satp_changed_at: u64 = 0;
    let mut started_tracing = false;
    let mut trace_count: u64 = 0;
    let trace_max = 100_000u64;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[boot] Shutdown at count={}", count);
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
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

        vm.step();

        if vm.cpu.ecall_count > prev_ecall_count {
            println!(
                "[ecall] count={} ecall={} a7=0x{:X} a6=0x{:X} a0=0x{:X}",
                count, vm.cpu.ecall_count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
            );
            prev_ecall_count = vm.cpu.ecall_count;
        }

        let uart_len = vm.bus.uart.tx_buf.len();
        if uart_len > prev_uart_len {
            let text: String = vm.bus.uart.tx_buf[prev_uart_len..]
                .iter()
                .map(|&b| {
                    if b >= 0x20 && b < 0x7F {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            println!("[uart] count={} text={:?}", count, text);
            prev_uart_len = uart_len;
        }

        if vm.cpu.csr.satp != last_satp {
            println!(
                "[satp] count={} SATP: 0x{:08X} -> 0x{:08X}",
                count, last_satp, vm.cpu.csr.satp
            );
            last_satp = vm.cpu.csr.satp;
            if !started_tracing {
                satp_changed_at = count;
                started_tracing = true;
                println!("[trace] Starting detailed trace from count={}", count);
            }
        }

        // After second SATP change, trace PCs
        if started_tracing && trace_count < trace_max {
            // Sample every 100th instruction
            if (count - satp_changed_at) % 100 == 0 {
                println!(
                    "[pc] +{} count={} PC=0x{:08X} priv={:?} sp=0x{:08X} ra=0x{:08X}",
                    count - satp_changed_at,
                    count,
                    vm.cpu.pc,
                    vm.cpu.privilege,
                    vm.cpu.x[2],
                    vm.cpu.x[1]
                );
                trace_count += 1;
            }
        }

        count += 1;
    }

    println!(
        "[boot] FINAL: count={} ecall={} uart={}",
        count,
        vm.cpu.ecall_count,
        vm.bus.uart.tx_buf.len()
    );
}
