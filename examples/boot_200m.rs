
use std::fs;
use std::collections::HashMap;
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
        200_000_000,
        bootargs,
    ).unwrap();
    
    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    let uart_out: Vec<u8> = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", uart_out.len());
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mcause: 0x{:08X}", vm.cpu.csr.mcause);
    println!("mepc: 0x{:08X}", vm.cpu.csr.mepc);
    
    // Read instruction at mepc
    if vm.cpu.csr.mepc != 0 {
        if let Ok(word) = vm.bus.read_word(vm.cpu.csr.mepc as u64) {
            println!("Instruction at mepc: 0x{:08X} (opcode 0x{:02X})", word, word & 0x7F);
        }
    }
    
    // Count traps by type - use the mmu_log and syscall_log for info
    println!("
Syscalls: {}", vm.bus.syscall_log.len());
    println!("MMU events: {}", vm.bus.mmu_log.len());
    println!("Sched events: {}", vm.bus.sched_log.len());
}
