use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 512, 2_000_000, bootargs,
    );
    
    match result {
        Ok((_vm, boot_result)) => {
            eprintln!("[diag] Boot result: {:?}", boot_result);
            eprintln!("[diag] UART bytes: {}", _vm.bus.uart.tx_buf.len());
            if !_vm.bus.uart.tx_buf.is_empty() {
                let s = String::from_utf8_lossy(&_vm.bus.uart.tx_buf);
                eprintln!("[UART] {}", s);
            }
        }
        Err(e) => {
            eprintln!("[diag] Boot error: {:?}", e);
        }
    }
}
