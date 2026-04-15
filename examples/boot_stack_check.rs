fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    // Run for 2M instructions and check for any output
    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        2_000_000,
        bootargs,
    )
    .unwrap();

    println!("After 2M: PC=0x{:08X} instrs={}", vm.cpu.pc, boot_result.instructions);
    println!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    println!("UART tx: {} chars", vm.bus.uart.tx_buf.len());

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        println!("SBI OUTPUT:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(500).collect();
        println!("UART OUTPUT:\n{}", preview);
    }

    // Check MMU events for S-mode faults
    let fault_count = vm.bus.mmu_log.iter().filter(|e| {
        matches!(e, geometry_os::riscv::mmu::MmuEvent::PageFault { .. })
    }).count();
    println!("Page faults in MMU log: {}", fault_count);
}
