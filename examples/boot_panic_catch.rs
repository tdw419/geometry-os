use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = std::fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").unwrap();
    
    let (mut vm, fw_addr, _entry, _dtb) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        64,
        "console=ttyS0 earlycon=sbi",
    ).unwrap();
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max = 30_000_000;
    let panic_addr = 0xC000252Eu32;
    let mut panic_caught = false;
    
    while count < max {
        // Check for panic entry BEFORE stepping
        if vm.cpu.pc == panic_addr && !panic_caught {
            panic_caught = true;
            let fmt_ptr = vm.cpu.x[10]; // a0 = format string
            println!("*** PANIC at count={} PC=0x{:08X} ***", count, vm.cpu.pc);
            println!("  RA=0x{:08X} SP=0x{:08X}", vm.cpu.x[1], vm.cpu.x[2]);
            println!("  A0 (fmt) = 0x{:08X}", fmt_ptr);
            
            // Read the format string
            if fmt_ptr >= 0xC0000000 {
                let pa = (fmt_ptr - 0xC0000000) as u64;
                let mut chars = Vec::new();
                for j in 0..300 {
                    let b = vm.bus.read_byte(pa + j as u64).unwrap_or(0);
                    if b == 0 { break; }
                    if b >= 0x20 && b < 0x7f { chars.push(b as char); }
                    else { chars.push('?'); }
                }
                let s: String = chars.into_iter().collect();
                println!("  fmt string: {}", s);
            }
            
            // Also print a1-a7 (varargs)
            println!("  A1=0x{:08X} A2=0x{:08X} A3=0x{:08X}", 
                vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]);
        }
        
        // Handle firmware trap handler (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        if vm.step() == geometry_os::riscv::cpu::StepResult::Ebreak { break; }
        count += 1;
    }
    
    if !panic_caught {
        println!("No panic detected in {} instructions. Final PC=0x{:08X}", count, vm.cpu.pc);
    }
}
