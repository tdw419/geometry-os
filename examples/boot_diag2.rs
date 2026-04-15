// Diagnostic: count SBI calls and trap types during Linux boot.
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, r) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        500_000,
        bootargs,
    ).unwrap();
    
    // Count SBI console output
    let sbi_output = &vm.bus.sbi.console_output;
    println!("SBI console output: {} bytes", sbi_output.len());
    if !sbi_output.is_empty() {
        let s = String::from_utf8_lossy(sbi_output);
        println!("{}", s);
    }
    
    // Check UART output
    let uart_output: Vec<u8> = vm.bus.uart.drain_tx();
    println!("\nUART TX buffer: {} bytes", uart_output.len());
    if !uart_output.is_empty() {
        let s = String::from_utf8_lossy(&uart_output);
        println!("{}", s);
    }
    
    // Print key CSRs
    println!("\n=== Key State ===");
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("scause: 0x{:08X}", vm.cpu.csr.scause);
    println!("sepc: 0x{:08X}", vm.cpu.csr.sepc);
    println!("stval: 0x{:08X}", vm.cpu.csr.stval);
    
    // CLINT state
    println!("\n=== CLINT ===");
    println!("mtime: {}", vm.bus.clint.mtime);
    println!("mtimecmp: {}", vm.bus.clint.mtimecmp);
    
    // Check syscall log
    println!("\n=== Syscalls: {} ===", vm.bus.syscall_log.len());
    for (i, sc) in vm.bus.syscall_log.iter().enumerate() {
        if i >= 20 { println!("... and {} more", vm.bus.syscall_log.len() - 20); break; }
        println!("  [{}] {} ({}) args={:?} ret={:?}", i, sc.name, sc.nr, sc.args, sc.ret);
    }
    
    // Check MMU log for SATP writes
    println!("\n=== MMU Events: {} ===", vm.bus.mmu_log.len());
    for (i, evt) in vm.bus.mmu_log.iter().enumerate() {
        if i >= 10 { println!("... and {} more", vm.bus.mmu_log.len() - 10); break; }
        println!("  [{}] {:?}", i, evt);
    }
    
    // SBI shutdown check
    println!("\n=== SBI ===");
    println!("shutdown_requested: {}", vm.bus.sbi.shutdown_requested);
    
    // Trace the last few instructions around the fault
    println!("\n=== Last instruction before stop ===");
    if let Some(ref last) = vm.cpu.last_step {
        println!("  PC: 0x{:08X}", last.pc);
        println!("  word: 0x{:08X}", last.word);
        println!("  op: {:?}", last.op);
        println!("  result: {:?}", last.result);
    }
}
