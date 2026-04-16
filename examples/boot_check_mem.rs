use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb_addr) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, initramfs.as_deref(), 512, bootargs,
        ).unwrap();
    
    // Check instructions at physical 0-0x20
    for addr in [0u64, 4, 8, 0xC, 0x10, 0x14, 0x18, 0x1C, 0x20] {
        if let Ok(w) = vm.bus.read_word(addr) {
            eprintln!("PA 0x{:04X}: 0x{:08X}", addr, w);
        }
    }
}
