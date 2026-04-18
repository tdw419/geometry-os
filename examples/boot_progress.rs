fn main() {
    // Boot test: more instructions to get past page init
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        100_000_000, // 100M instructions
        bootargs,
    )
    .unwrap();

    println!(
        "Boot: {} instr, PC=0x{:08X}, priv={:?}",
        boot_result.instructions, vm.cpu.pc, vm.cpu.privilege
    );
    println!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("UART:\n{}", preview);
    }

    println!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(2000).collect();
        println!("SBI:\n{}", preview);
    }
}
