
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        bootargs,
    ).unwrap();
    
    // Step through the loop and watch x29/x30
    println!("=== Stepping through tight loop ===");
    for i in 0..20 {
        let pc = vm.cpu.pc;
        let x29 = vm.cpu.x[29];
        let x30 = vm.cpu.x[30];
        let x14 = vm.cpu.x[14];
        let x6 = vm.cpu.x[6];
        let x5 = vm.cpu.x[5];
        println!("[{}] PC=0x{:08X} x5=0x{:08X} x6=0x{:08X} x14=0x{:08X} x29=0x{:08X} x30=0x{:08X}", 
            i, pc, x5, x6, x14, x29, x30);
        vm.step();
    }
    
    // Check if x29/x30 ever change
    println!("
=== Watching x29/x30 for changes ===");
    let mut prev_x29 = vm.cpu.x[29];
    let mut prev_x30 = vm.cpu.x[30];
    let mut changes = 0;
    for _ in 0..1_000_000 {
        let x29 = vm.cpu.x[29];
        let x30 = vm.cpu.x[30];
        if x29 != prev_x29 || x30 != prev_x30 {
            changes += 1;
            println!("Change at PC=0x{:08X}: x29 0x{:08X}->0x{:08X} x30 0x{:08X}->0x{:08X}",
                vm.cpu.pc, prev_x29, x29, prev_x30, x30);
            prev_x29 = x29;
            prev_x30 = x30;
            if changes >= 5 { break; }
        }
        vm.step();
    }
    if changes == 0 {
        println!("No changes to x29/x30 in 1M instructions");
    }
}
