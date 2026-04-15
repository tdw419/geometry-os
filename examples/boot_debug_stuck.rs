// Debug the stuck __memmove loop: dump registers at the stuck PC
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 256, 600_000, bootargs,
    ).unwrap();

    println!("Initial state: PC=0x{:08X} scause=0x{:08X} sepc=0x{:08X}", 
             vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc);
    println!("\n=== Stepping through __memmove loop ===");
    
    let mut prev_pc = 0u32;
    let mut same_count = 0u32;
    
    for step in 0..50 {
        let pc = vm.cpu.pc;
        let a4 = vm.cpu.x[14]; // source pointer
        let t4 = vm.cpu.x[29]; // dest pointer  
        let t5 = vm.cpu.x[30]; // end pointer
        
        let instr = vm.bus.mem.read_word(pc as u64).unwrap_or(0);
        
        // Show when PC changes or first few steps
        if step < 10 || pc != prev_pc {
            println!("Step {:>3}: PC=0x{:08X} instr=0x{:08X} a4=0x{:08X} t4=0x{:08X} t5=0x{:08X}",
                     step, pc, instr, a4, t4, t5);
            if pc != prev_pc && prev_pc != 0 {
                println!("         (PC changed from 0x{:08X}, was there {} steps)", prev_pc, same_count);
                same_count = 0;
            }
        }
        
        if pc == prev_pc {
            same_count += 1;
        } else {
            prev_pc = pc;
        }
        
        vm.step();
    }
    
    println!("\n=== Register dump at stuck point ===");
    for i in 0..32 {
        let name = ["zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2",
                     "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
                     "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
                     "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6"][i];
        println!("  x{:>2} ({:>4}): 0x{:08X}", i, name, vm.cpu.x[i]);
    }
    
    println!("\n=== CSR dump ===");
    println!("  scause:  0x{:08X}", vm.cpu.csr.scause);
    println!("  sepc:    0x{:08X}", vm.cpu.csr.sepc);
    println!("  stval:   0x{:08X}", vm.cpu.csr.stval);
    println!("  satp:    0x{:08X}", vm.cpu.csr.satp);
    println!("  sstatus: 0x{:08X}", vm.cpu.csr.mstatus); // sstatus = S-mode bits of mstatus
    println!("  stvec:   0x{:08X}", vm.cpu.csr.stvec);
    println!("  mepc:    0x{:08X}", vm.cpu.csr.mepc);
    println!("  mcause:  0x{:08X}", vm.cpu.csr.mcause);
    println!("  mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    
    // The memmove loop stores to t4. Check what physical address that maps to.
    // We can try reading from the bus to see if the write would succeed
    let t4_val = vm.cpu.x[29];
    println!("\n=== Store target check ===");
    println!("t4 = 0x{:08X} (memmove dest pointer)", t4_val);
    println!("t5 = 0x{:08X} (memmove end pointer)", vm.cpu.x[30]);
    println!("t4-t5 = {:+} (signed diff)", (t4_val as i32) - (vm.cpu.x[30] as i32));
    
    // Try a bus write to t4 to see what happens
    let test_result = vm.bus.write_word(t4_val as u64, 0xDEADBEEF);
    println!("bus.write_word(0x{:08X}) = {:?}", t4_val, test_result);
}
