use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        178_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let km_phys: u64 = 0x00C79E90;
    let km: Vec<u32> = (0..8).map(|i| vm.bus.read_word(km_phys + i as u64 * 4).unwrap_or(0xDEAD)).collect();
    println!("kernel_map at PA 0x{:08X}:", km_phys);
    println!("  page_offset (off 0)  = 0x{:08X}", km[0]);
    println!("  virt_addr (off 4)    = 0x{:08X}", km[1]);
    println!("  virt_offset (off 8)  = 0x{:08X}", km[2]);
    println!("  phys_addr (off 12)   = 0x{:08X}", km[3]);
    println!("  size (off 16)        = 0x{:08X}", km[4]);
    println!("  va_pa_offset (off 20)= 0x{:08X}", km[5]);
    println!("  va_kernel_pa_offset (off 24)= 0x{:08X}", km[6]);
    
    println!("\n_start = 0xC0000000");
    println!("virt_addr - _start = 0x{:08X}", km[1].wrapping_sub(0xC0000000));
}
