use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Try with different bootargs - maybe the kernel needs specific SBI setup
    for bootargs in [
        "console=ttyS0 earlycon=sbi panic=1 quiet",
        "console=ttyS0 earlycon=sbi panic=1",
        "console=ttyS0 earlycon=htif panic=1",
    ] {
        eprintln!("\n=== Testing: {} ===", bootargs);
        let result = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 5_000_000, bootargs);
        match result {
            Ok((vm, br)) => {
                eprintln!("  Instructions: {}", br.instructions);
                eprintln!("  ECALL count: {}", vm.cpu.ecall_count);
                eprintln!("  Console output: {} bytes", vm.bus.sbi.console_output.len());
                if !vm.bus.sbi.console_output.is_empty() {
                    let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
                    let preview: String = s.chars().take(300).collect();
                    eprintln!("  Console: {}", preview);
                }
            }
            Err(e) => eprintln!("  Error: {:?}", e),
        }
    }
}
