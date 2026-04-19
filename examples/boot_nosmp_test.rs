// Test different bootargs to find what prevents "Attempted to kill the idle task!"
use geometry_os::riscv::RiscvVm;

fn try_boot(label: &str, bootargs: &str) {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    eprintln!("\n=== {} ===", label);
    eprintln!("bootargs: {}", bootargs);

    match RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        bootargs,
    ) {
        Ok((vm, result)) => {
            eprintln!(
                "Done: {} instructions, PC=0x{:08X} priv={:?}",
                result.instructions, vm.cpu.pc, vm.cpu.privilege
            );
            eprintln!("UART: {} chars", vm.bus.uart.tx_buf.len());
            if !vm.bus.uart.tx_buf.is_empty() {
                let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
                let preview: String = s.chars().take(2000).collect();
                eprintln!("UART:\n{}", preview);
            }
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}

fn main() {
    // Test 1: nosmp only
    try_boot("Test 1: nosmp", "console=ttyS0 earlycon=sbi panic=1 nosmp");

    // Test 2: maxcpus=1
    try_boot(
        "Test 2: maxcpus=1",
        "console=ttyS0 earlycon=sbi panic=1 maxcpus=1",
    );

    // Test 3: nosmp + nosoftlockup (disable watchdog that might kill idle)
    try_boot(
        "Test 3: nosmp + nosoftlockup",
        "console=ttyS0 earlycon=sbi panic=1 nosmp nosoftlockup",
    );

    // Test 4: nosmp + nohz=off (disable tickless, force timer ticks)
    try_boot(
        "Test 4: nosmp + nohz=off",
        "console=ttyS0 earlycon=sbi panic=1 nosmp nohz=off",
    );

    // Test 5: nr_cpus=1
    try_boot(
        "Test 5: nr_cpus=1",
        "console=ttyS0 earlycon=sbi panic=1 nr_cpus=1",
    );
}
