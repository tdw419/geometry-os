use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, info) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        50_000_000,
        "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=0 loglevel=8",
    )
    .unwrap();

    eprintln!(
        "[result] Boot completed: {} instructions, PC=0x{:08X}",
        info.instructions, vm.cpu.pc
    );

    // Check UART write statistics
    eprintln!(
        "[result] UART write_count={} tx_buf_len={}",
        vm.bus.uart.write_count,
        vm.bus.uart.tx_buf.len(),
    );
    eprintln!(
        "[result] SBI console_output={} bytes",
        vm.bus.sbi.console_output.len(),
    );

    // Check for UART output
    let uart_output = vm.bus.uart.drain_tx();
    if !uart_output.is_empty() {
        let s: String = uart_output.iter().map(|&b| b as char).collect();
        eprintln!("[result] UART output ({} bytes):", s.len());
        for line in s.lines() {
            eprintln!("  {}", line);
        }
    } else {
        eprintln!("[result] No UART output");
    }

    // Check SBI console output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("[result] SBI console output ({} bytes):", s.len());
        for line in s.lines().take(50) {
            eprintln!("  {}", line);
        }
    }
}
