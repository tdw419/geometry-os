use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());

    // Print ALL ecalls
    println!("\nAll {} SBI ecalls:", vm.bus.sbi.ecall_log.len());
    let ext_names = {
        let mut m = std::collections::HashMap::new();
        m.insert(0x10u32, "BASE");
        m.insert(0x01, "LEGACY_PUTCHAR");
        m.insert(0x02, "CONSOLE_PUTCHAR");
        m.insert(0x54494D45, "TIME");
        m.insert(0x735049, "IPI");
        m.insert(0x52464E43, "RFENCE");
        m.insert(0x53525354, "SRST");
        m.insert(0x4442434E, "DBCN");
        m.insert(0x48534F, "HSM");
        m.insert(0x00, "SET_TIMER");
        m.insert(0x08, "SHUTDOWN");
        m
    };
    for (i, (a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().enumerate() {
        let name = ext_names.get(a7).unwrap_or(&"???");
        if *a7 == 0x10 && *a6 == 3 {
            let probe_name = ext_names.get(a0).unwrap_or(&"???");
            println!(
                "  [{}] a7=0x{:08X} ({}) a6={} PROBE a0=0x{:08X} ({})",
                i, a7, name, a6, a0, probe_name
            );
        } else {
            println!(
                "  [{}] a7=0x{:08X} ({}) a6={} a0=0x{:08X}",
                i, a7, name, a6, a0
            );
        }
    }

    // Show first 200 chars of console output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!(
            "\nConsole output (first 500 chars):\n{}",
            &s[..s.len().min(500)]
        );
    }
}
