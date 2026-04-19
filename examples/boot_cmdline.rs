//! Quick check of key boot state after 5M instructions.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data, None, 512,
        "console=ttyS0 earlycon=sbi nosmp",
    ).expect("setup");

    let sbi_dbcn_pa: u64 = 0x014820A0;
    println!("BEFORE: sbi_debug_console_available = {}", vm.bus.read_word(sbi_dbcn_pa).unwrap_or(0));

    // Run 5M instructions
    for _ in 0..5_000_000u64 {
        if vm.bus.sbi.shutdown_requested { break; }
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }

    println!("AFTER 5M: sbi_debug_console_available = {}", vm.bus.read_word(sbi_dbcn_pa).unwrap_or(0));
    println!("ECALLs: {}, SBI output: {} bytes, PC: 0x{:08X}", 
        vm.cpu.ecall_count, vm.bus.sbi.console_output.len(), vm.cpu.pc);
    
    // Check cmdline_buffer -- Linux stores parsed cmdline at saved_command_line (BSS)
    // For RV32, it's typically a static buffer
    // Search for "console" in DTB region to verify bootargs are accessible
    println!("\nSearching bootargs in DTB at PA 0x{:08X}:", dtb_addr);
    for off in 0..4096u64 {
        let b = vm.bus.read_byte(dtb_addr + off).unwrap_or(0);
        if b == b'c' {
            let mut s = Vec::new();
            for j in 0..60 {
                let cb = vm.bus.read_byte(dtb_addr + off + j).unwrap_or(0);
                s.push(cb);
                if cb == 0 { break; }
            }
            let str_s = String::from_utf8_lossy(&s);
            if str_s.contains("console") {
                println!("  offset {}: {:?}", off, str_s);
            }
        }
    }
    
    // Check if the kernel's cmdline_buffer was populated
    // saved_command_line is typically in .data or .bss
    // Let's just check if the kernel has the string somewhere near 0xC014 area
    println!("\nChecking earlycon struct (earlycon at VA 0xC0804368, PA 0x00804368):");
    let earlycon_pa = 0x00804368u64;
    // earlycon struct is: write fn ptr + read fn ptr
    let write_fn = vm.bus.read_word(earlycon_pa).unwrap_or(0);
    let index = vm.bus.read_word(earlycon_pa + 4).unwrap_or(0);
    println!("  earlycon->write = 0x{:08X}", write_fn);
    println!("  earlycon->index = {}", index);
}
