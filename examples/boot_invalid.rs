
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
    
    // Check last_step for Invalid operations
    if let Some(ref last) = vm.cpu.last_step {
        println!("Last step:");
        println!("  PC: 0x{:08X}", last.pc);
        println!("  word: 0x{:08X}", last.word);
        println!("  op: {:?}", last.op);
        println!("  result: {:?}", last.result);
        println!("  inst_len: {}", last.inst_len);
        
        // Check if op is Invalid
        match &last.op {
            geometry_os::riscv::decode::Operation::Invalid(w) => {
                println!("  *** INVALID INSTRUCTION: 0x{:08X} ***", w);
                println!("  opcode: 0x{:02X}", w & 0x7F);
            }
            _ => {}
        }
    }
    
    // Now step and watch for Invalid operations
    let mut invalid_count = 0u64;
    let mut last_invalid_pc = 0u32;
    let mut last_invalid_word = 0u32;
    
    for _ in 0..5_000_000 {
        vm.step();
        if let Some(ref last) = vm.cpu.last_step {
            if let geometry_os::riscv::decode::Operation::Invalid(w) = last.op {
                invalid_count += 1;
                last_invalid_pc = last.pc;
                last_invalid_word = w;
            }
        }
    }
    
    println!("
Invalid instructions in 5M steps: {}", invalid_count);
    if invalid_count > 0 {
        println!("Last invalid at PC: 0x{:08X}, word: 0x{:08X} (opcode 0x{:02X})", 
            last_invalid_pc, last_invalid_word, last_invalid_word & 0x7F);
    }
}
