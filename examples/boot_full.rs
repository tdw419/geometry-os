use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        10_000_000,
        bootargs,
    );

    match result {
        Ok((_vm, boot_result)) => {
            eprintln!("[diag] Boot result: {:?}", boot_result);
        }
        Err(e) => {
            eprintln!("[diag] Boot error: {:?}", e);
        }
    }
}
