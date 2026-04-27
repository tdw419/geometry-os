// Diagnostic: Boot Linux with extended instruction limit to reach userspace.
// The kernel reaches arch_cpu_idle at ~50M instructions but kernel_init
// may need more time to complete. This runs 200M instructions with
// periodic progress logging.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = match std::fs::read(kernel_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Kernel not found at {}", kernel_path);
            return;
        }
    };
    let initramfs = std::fs::read(initramfs_path).ok();

    eprintln!("Kernel: {} bytes", kernel_image.len());
    if let Some(ref ir) = initramfs {
        eprintln!("Initramfs: {} bytes", ir.len());
    }

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 loglevel=7";

    // Run with 200M instructions
    let (vm, result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        200_000_000,
        bootargs,
    )
    .expect("boot should succeed");

    eprintln!("\n=== Boot Complete ===");
    eprintln!("Instructions: {}", result.instructions);
    eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);

    // SBI console output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!(
            "\nSBI console output ({} bytes):",
            vm.bus.sbi.console_output.len()
        );
        eprintln!("{}", s);
    }

    // UART output
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("\nUART TX output ({} bytes):", vm.bus.uart.tx_buf.len());
        eprintln!("{}", s);
    }

    // Check for userspace indicators
    let sbi_str = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    let uart_str = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
    let all_output = format!("{}{}", sbi_str, uart_str);

    if all_output.contains("/init") || all_output.contains("Freeing unused") {
        eprintln!("\n*** KERNEL_INIT PROGRESSED: Found userspace indicators ***");
    }
    if all_output.contains("# ") || all_output.contains("$ ") || all_output.contains("login:") {
        eprintln!("\n*** USERSPACE REACHED: Shell prompt detected! ***");
    }

    // Last 20 SBI ecalls
    eprintln!("\nLast 20 SBI ecalls (of {}):", vm.bus.sbi.ecall_log.len());
    let total = vm.bus.sbi.ecall_log.len();
    let start = total.saturating_sub(20);
    for i in start..total {
        let (a7, a6, a0) = vm.bus.sbi.ecall_log[i];
        let ext_name = match a7 {
            0x10 => "BASE",
            0x54494D45 => "TIMER",
            0x4442434E => "DBCN",
            0x735049 => "IPI",
            0x52464E43 => "RFNC",
            0x53525354 => "SRST",
            0x48534F => "HSM",
            _ => "???",
        };
        eprintln!(
            "  [{}] a7=0x{:08X} ({}) a6={} a0=0x{:08X}",
            i, a7, ext_name, a6, a0
        );
    }

    eprintln!("CPU ecall_count: {}", vm.cpu.ecall_count);
}
