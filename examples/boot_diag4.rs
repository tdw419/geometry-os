use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 200_000_000, bootargs,
    ).unwrap();

    // kernel_map struct at PA 0x00C79E90
    let km_phys: u64 = 0x00C79E90;
    
    // struct kernel_mapping layout (rv32, unsigned long = 4 bytes):
    //   0: page_offset (unsigned long)
    //   4: virt_addr (unsigned long)
    //   8: virt_offset (unsigned long)
    //  12: phys_addr (uintptr_t)
    //  16: size (uintptr_t)
    //  20: va_pa_offset (unsigned long)
    //  24: va_kernel_pa_offset (unsigned long)
    // Total: 28 bytes
    
    println!("=== kernel_map struct at PA 0x{:08X} ===", km_phys);
    let page_offset = vm.bus.read_word(km_phys + 0).unwrap_or(0xDEAD);
    let virt_addr = vm.bus.read_word(km_phys + 4).unwrap_or(0xDEAD);
    let virt_offset = vm.bus.read_word(km_phys + 8).unwrap_or(0xDEAD);
    let phys_addr = vm.bus.read_word(km_phys + 12).unwrap_or(0xDEAD);
    let size = vm.bus.read_word(km_phys + 16).unwrap_or(0xDEAD);
    let va_pa_offset = vm.bus.read_word(km_phys + 20).unwrap_or(0xDEAD);
    let va_kernel_pa_offset = vm.bus.read_word(km_phys + 24).unwrap_or(0xDEAD);
    
    println!("  page_offset          = 0x{:08X}", page_offset);
    println!("  virt_addr            = 0x{:08X}", virt_addr);
    println!("  virt_offset          = 0x{:08X}", virt_offset);
    println!("  phys_addr            = 0x{:08X}", phys_addr);
    println!("  size                 = 0x{:08X}", size);
    println!("  va_pa_offset         = 0x{:08X}", va_pa_offset);
    println!("  va_kernel_pa_offset  = 0x{:08X}", va_kernel_pa_offset);
    
    println!("
=== ANALYSIS ===");
    if page_offset != 0xC0000000 {
        println!("BUG: page_offset=0x{:08X} but should be 0xC0000000!", page_offset);
        println!("PAGE_OFFSET macro reads kernel_map.page_offset at runtime.");
        println!("If page_offset is wrong, all __va()/__pa() computations use the wrong offset.");
    }
    if virt_addr != 0xC0000000 {
        println!("BUG: virt_addr=0x{:08X} but should be 0xC0000000!", virt_addr);
    }
}
