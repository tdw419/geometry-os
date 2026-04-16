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
    
    // Dump memory around 0x6600 (where kernel expects OpenSBI firmware)
    eprintln!("=== Memory around 0x6600 (OpenSBI firmware area) ===");
    for addr in (0x6400..0x6700).step_by(4) {
        let val = vm.bus.read_word(addr as u64).unwrap_or(0);
        if val != 0 {
            eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
        }
    }
    
    // Dump memory at 0x0..0x20 (entry point area)
    eprintln!("\n=== Memory at 0x0000-0x0020 ===");
    for addr in (0x0..0x20).step_by(4) {
        let val = vm.bus.read_word(addr as u64).unwrap_or(0);
        eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
    }
    
    // Check what the kernel entry point is
    eprintln!("\n=== Entry point ===");
    eprintln!("  entry (physical): 0x{:08X}", _entry);
    
    // Disassemble a few instructions from entry
    eprintln!("\n=== Instructions at entry (0x{:08X}) ===", _entry);
    for i in 0..8 {
        let addr = _entry as u64 + (i * 4);
        let instr = vm.bus.read_word(addr).unwrap_or(0);
        eprintln!("  0x{:08X}: 0x{:08X}", addr, instr);
    }
    
    // Check what instruction jumps to 0x6600
    // The kernel's _start typically does: la ra, _start; call 0x6600
    // Let's see what's at the physical entry point after setup_vm
    eprintln!("\n=== Memory at 0x0000_1200-0x0000_1240 (firmware stub at 0x12) ===");
    for addr in (0x1200..0x1240).step_by(4) {
        let val = vm.bus.read_word(addr as u64).unwrap_or(0);
        if val != 0 {
            eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
        }
    }
    
    eprintln!("\n=== fw_addr (M-mode trap handler): 0x{:08X} ===", _fw_addr);
    let fw_instr = vm.bus.read_word(_fw_addr).unwrap_or(0);
    eprintln!("  instruction: 0x{:08X} (should be 0x30200073 = MRET)", fw_instr);
}
