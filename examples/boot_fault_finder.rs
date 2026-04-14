// Detailed fault finder: trace what happens around the fault
// cargo run --example boot_fault_finder
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::decode;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    println!("=== Boot Fault Finder ===");

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        300_000,
        bootargs,
    ).unwrap();

    println!("Instructions: {}", result.instructions);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mcause: 0x{:08X}", vm.cpu.csr.mcause);
    println!("mepc: 0x{:08X}", vm.cpu.csr.mepc);
    println!("mtval: 0x{:08X}", vm.cpu.csr.mtval);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
    println!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("scause: 0x{:08X}", vm.cpu.csr.scause);
    println!("sepc: 0x{:08X}", vm.cpu.csr.sepc);
    // sstatus is a view of mstatus
    println!("medeleg: 0x{:08X}", vm.cpu.csr.medeleg);
    println!("mideleg: 0x{:08X}", vm.cpu.csr.mideleg);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("mie: 0x{:08X}", vm.cpu.csr.mie);
    println!("mip: 0x{:08X}", vm.cpu.csr.mip);
    // sie is a view of mie

    // Decode the satp to understand page table location
    let satp = vm.cpu.csr.satp;
    let mode = (satp >> 31) & 1;
    let asid = (satp >> 22) & 0x1FF;
    let ppn = satp & 0x3FFFFF;
    println!("\nsatp decode: mode={}, asid={}, ppn=0x{:X}", mode, asid, ppn);
    let pt_phys = (ppn as u64) << 12;
    println!("Page table root physical addr: 0x{:08X}", pt_phys);
    println!("RAM base: 0x{:08X}", vm.bus.mem.ram_base);

    // Dump the first few page table entries
    if pt_phys >= vm.bus.mem.ram_base {
        println!("\n--- First page table entries at 0x{:08X} ---", pt_phys);
        for i in 0..16 {
            let addr = pt_phys + (i as u64) * 4;
            match vm.bus.read_word(addr) {
                Ok(entry) => {
                    if entry != 0 {
                        let vpn_idx = i;
                        let pte_flags = entry & 0x3FF;
                        let pte_ppn = (entry >> 10) & 0x3FFFFF;
                        let is_leaf = (entry & 0xE) != 0; // R|W|X != 0
                        println!("  PTE[{:3}]: 0x{:08X}  flags=0x{:03X} ppn=0x{:X} leaf={}",
                            vpn_idx, entry, pte_flags, pte_ppn, is_leaf);
                    }
                }
                Err(_) => {}
            }
        }
    }

    // Read the faulting instruction
    let fault_pc = vm.cpu.csr.mepc;
    println!("\n--- Faulting instruction at mepc=0x{:08X} ---", fault_pc);

    // Try to read the instruction (need to handle MMU translation)
    // For now, try direct physical access
    if u64::from(fault_pc) >= vm.bus.mem.ram_base {
        match vm.bus.read_word(fault_pc as u64) {
            Ok(word) => {
                println!("Raw word: 0x{:08X}", word);
                let hw = (word & 0xFFFF) as u16;
                if decode::is_compressed(hw) {
                    println!("Compressed instruction");
                } else {
                    println!("Decoded: {:?}", decode::decode(word));
                }
            }
            Err(e) => println!("Cannot read: {:?}", e),
        }
    }

    // Check UART output
    let mut uart_out = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => uart_out.push(b),
        }
    }
    if !uart_out.is_empty() {
        println!("\n--- UART Output ({} bytes) ---", uart_out.len());
        println!("{}", String::from_utf8_lossy(&uart_out));
    }

    // Also check SBI console output
    if !vm.bus.sbi.console_output.is_empty() {
        println!("\n--- SBI Console Output ({} bytes) ---", vm.bus.sbi.console_output.len());
        println!("{}", String::from_utf8_lossy(&vm.bus.sbi.console_output));
    }

    println!("\nsbi shutdown_requested: {}", vm.bus.sbi.shutdown_requested);
}
