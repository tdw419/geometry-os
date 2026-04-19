/// Diagnostic: Check what the 7 ECALLs are by reading SBI ecall_log.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel = std::fs::read(kernel_path).expect("kernel not found");
    let initramfs = std::fs::read(initramfs_path).expect("initramfs not found");

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        128,
        "console=ttyS0 earlycon=sbi panic=5 quiet nosmp",
    )
    .expect("boot setup failed");

    let mut count: u64 = 0;
    let max: u64 = 400_000;

    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;

        // Catch entry to panic()
        if vm.cpu.pc == 0xC000_252E {
            eprintln!("[PANIC] at count={}", count);
            break;
        }
    }

    eprintln!("Total ECALLs: {}", vm.cpu.ecall_count);
    eprintln!("SBI ecall_log length: {}", vm.bus.sbi.ecall_log.len());
    for (i, (a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().enumerate() {
        let ext_name = match *a7 {
            0x01 => "SBI_CONSOLE_PUTCHAR (legacy)",
            0x02 => "SBI_EXT_CONSOLE (v0.2)",
            0x10 => "SBI_EXT_BASE",
            0x44 => "SBI_EXT_HART_STATE",
            0x53524954 => "SBI_EXT_SRST",
            _ => "unknown",
        };
        eprintln!(
            "  ecall #{}: a7=0x{:08X} ({}) a6={} a0=0x{:08X} ({})",
            i,
            a7,
            ext_name,
            a6,
            a0,
            if *a7 == 0x01 {
                format!("char '{}'", *a0 as u8 as char)
            } else {
                String::new()
            }
        );
    }
    eprintln!("\nUART TX buffer: {} bytes", vm.bus.uart.tx_buf.len());
    eprintln!("UART TX: {:?}", String::from_utf8_lossy(&vm.bus.uart.tx_buf));
    eprintln!("\nConsole output: {}", String::from_utf8_lossy(&vm.bus.sbi.console_output));
}
