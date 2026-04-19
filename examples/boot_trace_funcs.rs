/// Diagnostic: trace whether the kernel reaches specific code addresses.
/// Checks for earlycon setup, printk, panic, and other key functions.
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=0 loglevel=8";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    // Key addresses to watch (from nm output)
    let watch_addrs: &[(&str, u32)] = &[
        ("param_setup_earlycon", 0xC04149AE),
        ("setup_earlycon", 0xC0414750),
        ("console_init", 0xC0407F22),
        ("printk", 0xC001499C),
        ("vprintk", 0xC00148C0),
        ("early_init_dt_scan_chosen_stdout", 0xC041B430),
        ("start_kernel", 0xC0409860),
        ("setup_arch", 0xC0404C00),
        ("rest_init", 0xC04049DC),
        ("panic", 0xC0002520),
    ];

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 20_000_000;
    let mut count: u64 = 0;
    let mut hit_addrs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut first_hits: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Check for ECALL at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let is_interrupt = (mcause >> 31) & 1 == 1;
            if !is_interrupt && cause_code == 9 {
                // ECALL_S
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10],
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if !is_interrupt {
                // Forward other M-mode traps to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Check watch addresses
        for (name, addr) in watch_addrs {
            if vm.cpu.pc == *addr {
                if !hit_addrs.contains(*name) {
                    hit_addrs.insert(name.to_string());
                    first_hits.insert(name.to_string(), count);
                    eprintln!("[hit] {} at count={} PC=0x{:08X}", name, count, vm.cpu.pc);
                }
            }
        }

        let _ = vm.step();
        count += 1;

        if count % 5_000_000 == 0 {
            eprintln!(
                "[{}]M PC=0x{:08X} uart_writes={} sbi_out={}",
                count / 1_000_000,
                vm.cpu.pc,
                vm.bus.uart.write_count,
                vm.bus.sbi.console_output.len(),
            );
        }
    }

    eprintln!("\n=== Results ({}M instructions) ===", count / 1_000_000);
    eprintln!("UART writes: {}", vm.bus.uart.write_count);
    eprintln!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    eprintln!("\nFunctions reached:");
    for (name, addr) in watch_addrs {
        let hit = hit_addrs.contains(*name);
        let at = first_hits.get(*name).map(|c| format!("{}K", c / 1000)).unwrap_or("never".to_string());
        eprintln!("  {:40} addr=0x{:08X} hit={} at={}", name, addr, hit, at);
    }

    // Check if any UART output appeared
    let uart = vm.bus.uart.drain_tx();
    if !uart.is_empty() {
        eprintln!("\nUART output ({} bytes):", uart.len());
        let s = String::from_utf8_lossy(&uart);
        for line in s.lines().take(20) {
            eprintln!("  {}", line);
        }
    }
}
