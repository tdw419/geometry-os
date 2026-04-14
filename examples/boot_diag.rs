
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (vm, r) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        500_000,
        bootargs,
    ).unwrap();
    
    println!("=== Register dump ===");
    for i in 0..32 {
        println!("x{:02} = 0x{:08X}", i, vm.cpu.x[i]);
    }
    println!("\nsepc = 0x{:08X}", vm.cpu.csr.sepc);
    println!("scause = 0x{:08X}", vm.cpu.csr.scause);
    println!("stval = 0x{:08X}", vm.cpu.csr.stval);
    // sscratch not yet implemented in CsrBank
    
    // x4 = 0x{:08X} -- this is tp (thread pointer)
    // The SW x2, 8(x4) at PC=0xC093ADA4 stores to x4+8
    let addr = vm.cpu.x[4].wrapping_add(8);
    println!("\nSW target: x4+8 = 0x{:08X}", addr);
}
