fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    
    use geometry_os::riscv::RiscvVm;
    
    let result = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 5_000_000, bootargs);
    match result {
        Ok((mut vm, r)) => {
            let mut bridge = geometry_os::riscv::bridge::UartBridge::new();
            let mut canvas = vec![0u32; 128 * 80];
            bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
            
            println!("{} instructions, PC=0x{:08X} priv={:?}", r.instructions, vm.cpu.pc, vm.cpu.privilege);
            let mcause = vm.cpu.csr.read(geometry_os::riscv::csr::MCAUSE);
            let mepc = vm.cpu.csr.read(geometry_os::riscv::csr::MEPC);
            println!("mcause=0x{:X} mepc=0x{:08X}", mcause, mepc);
            
            let mut found = false;
            for row in 0..128 {
                let mut line = String::new();
                let mut has = false;
                for col in 0..80 {
                    let ch = canvas[row * 80 + col];
                    if ch != 0 { has = true; line.push(char::from_u32(ch).unwrap_or('.')); }
                    else { line.push(' '); }
                }
                if has { found = true; println!("UART {:3}: {}", row, line.trim_end()); }
            }
            if !found { println!("NO UART OUTPUT"); }
        }
        Err(e) => eprintln!("FAILED: {}", e),
    }
}
