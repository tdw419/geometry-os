// Dump the DTB to verify it's well-formed
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (vm, result) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 256, 0, bootargs,
    ).unwrap();

    println!("DTB addr: 0x{:08X}", result.dtb_addr);
    
    // Read DTB magic
    let magic = vm.bus.mem.read_word(result.dtb_addr).unwrap_or(0);
    println!("DTB magic: 0x{:08X} (expected 0xD00DFEED)", magic);
    
    let totalsize = vm.bus.mem.read_word(result.dtb_addr + 4).unwrap_or(0);
    let off_dt_struct = vm.bus.mem.read_word(result.dtb_addr + 8).unwrap_or(0);
    let off_dt_strings = vm.bus.mem.read_word(result.dtb_addr + 12).unwrap_or(0);
    println!("Total size: {}", totalsize);
    println!("Struct offset: {}", off_dt_struct);
    println!("Strings offset: {}", off_dt_strings);
    
    // Dump raw DTB as hex for first 256 bytes
    println!("\nFirst 256 bytes of DTB:");
    for i in 0..64 {
        let word = vm.bus.mem.read_word(result.dtb_addr + i * 4).unwrap_or(0);
        print!("{:08X} ", word);
        if (i + 1) % 8 == 0 { println!(); }
    }
    println!();
    
    // Verify memory node
    println!("\nMemory node check:");
    println!("  ram_base: 0x{:08X}", 0xC0000000u32);
    println!("  RAM size: 256MB = 0x{:08X}", 256 * 1024 * 1024);
}
