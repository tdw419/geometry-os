use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    let max = 250_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    
    // Watch for specific events
    let mut stvec_write_seen = false;
    let mut setup_vm_call_seen = false;
    let mut nop_hit_seen = false;
    
    while count < max {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
        
        // Check if PC passed through stvec setup
        if !stvec_write_seen && pc >= 0xC00010E2 && pc <= 0xC00010E6 {
            stvec_write_seen = true;
            println!("[{}] csrw stvec at PC=0x{:08X}, a3=0x{:08X}, stvec now=0x{:08X}", 
                count, pc, vm.cpu.x[13], vm.cpu.csr.read(0x005));
        }
        
        // Check for setup_vm call
        if !setup_vm_call_seen && pc == 0xC00010EA {
            setup_vm_call_seen = true;
            println!("[{}] jalr setup_vm at PC=0x{:08X}, ra=0x{:08X}", count, pc, vm.cpu.x[1]);
        }
        
        // Check for NOP at 0xC00010F6
        if !nop_hit_seen && pc == 0xC00010F6 {
            nop_hit_seen = true;
            println!("[{}] NOP (was jal relocate_enable_mmu) at PC=0x{:08X}", count, pc);
            println!("    STVEC=0x{:08X} GP=0x{:08X} SP=0x{:08X} TP=0x{:08X}", 
                vm.cpu.csr.read(0x005), vm.cpu.x[3], vm.cpu.x[2], vm.cpu.x[4]);
        }
        
        // Detect spin loops
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 5000 {
                println!("\n[{}] SPIN at PC=0x{:08X}", count, vm.cpu.pc);
                println!("    STVEC=0x{:08X} RA=0x{:08X} SP=0x{:08X}", 
                    vm.cpu.csr.read(0x005), vm.cpu.x[1], vm.cpu.x[2]);
                // Show last 20 unique PCs
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
        
        // Report at intervals
        if count % 50_000 == 0 {
            println!("[{}] PC=0x{:08X} SP=0x{:08X} stvec=0x{:08X} a3=0x{:08X}", 
                count, vm.cpu.pc, vm.cpu.x[2], vm.cpu.csr.read(0x005), vm.cpu.x[13]);
        }
    }
    
    println!("\n=== SUMMARY ===");
    println!("stvec_write_seen: {}", stvec_write_seen);
    println!("setup_vm_call_seen: {}", setup_vm_call_seen);
    println!("nop_hit_seen: {}", nop_hit_seen);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("STVEC: 0x{:08X}", vm.cpu.csr.read(0x005));
}
