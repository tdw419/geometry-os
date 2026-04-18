use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    for bootargs in [
        "console=ttyS0 earlycon=sbi panic=1 quiet",
        "console=ttyS0 earlycon=sbi panic=1",
    ] {
        eprintln!("\n=== {} ===", bootargs);
        match RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            5_000_000,
            bootargs,
        ) {
            Ok((vm, br)) => {
                eprintln!(
                    "  instr={} ecall={} console={}",
                    br.instructions,
                    vm.cpu.ecall_count,
                    vm.bus.sbi.console_output.len()
                );
                if !vm.bus.sbi.console_output.is_empty() {
                    let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
                    eprintln!("  out: {}", &s[..s.len().min(300)]);
                }
            }
            Err(e) => eprintln!("  err: {:?}", e),
        }
    }
}
