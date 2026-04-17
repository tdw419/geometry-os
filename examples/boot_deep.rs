// Diagnostic: breakpoint at panic() entry, capture register state
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        50_000_000u64, // 50M instructions to give more time
        bootargs,
    ).unwrap();
    
    println!("Boot: {} instructions", result.instructions);
    println!("PC: 0x{:08X} Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);
    
    // Check if we ever saw an SBI call or UART output
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("\nUART output ({} bytes):\n{}", tx.len(), s);
    }
    let sbi_out = &vm.bus.sbi.console_output;
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        println!("\nSBI output ({} bytes):\n{}", sbi_out.len(), s);
    }
    
    // The panic() function at 0xC000252E first does: addi sp, sp, -96
    // Then saves registers. The format string is in a0.
    // Since the kernel is stuck in the udelay loop inside panic,
    // the original panic arguments are lost.
    // 
    // But: panic() writes to a global "panic_cpu" and sets a flag.
    // Let's check if we can find the panic message by scanning .rodata
    // for strings that look like common early boot panics.
    
    // Better approach: look at the kernel log buffer.
    // Linux stores log messages in a ring buffer at __log_buf.
    // We can find it by looking at the symbol table.
    println!("\n=== Scanning for kernel panic messages ===");
    
    // The panic blink loop is at 0xC0002790 (li a0, 1000; auipc ra; jalr udelay)
    // Before that, panic() calls __bust_spinlocks(1) and printk.
    // The printk output goes to the console (SBI or UART).
    // Since we have 0 SBI calls, either:
    // 1. printk never executed (panic called very early, before console init)
    // 2. printk executed but the console wasn't set up yet
    
    // Check: did the kernel ever write to UART MMIO directly?
    // The UART is at 0x10000000. Check if any UART registers are non-default.
    println!("UART LSR: 0x{:02X}", vm.bus.read_byte(0x10000005).unwrap_or(0));
    println!("UART LCR: 0x{:02X}", vm.bus.read_byte(0x10000003).unwrap_or(0));
    println!("UART FCR: 0x{:02X}", vm.bus.read_byte(0x10000002).unwrap_or(0));
    
    // Check current SATP and page table
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("\nSATP: 0x{:08X}, page dir PA: 0x{:08X}", satp, pg_dir_phys);
    
    // Check all L1 entries 768-864
    let mut mapped = 0;
    let mut unmapped = 0;
    for i in 768..864 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        if entry != 0 {
            mapped += 1;
        } else {
            unmapped += 1;
            let va = (i as u32) << 22;
            // Only print first few
            if unmapped <= 10 {
                println!("  L1[{}] VA=0x{:08X} UNMAPPED", i, va);
            }
        }
    }
    println!("Kernel L1: {} mapped, {} unmapped", mapped, unmapped);
    
    // Check: is the kernel reading from correct physical addresses?
    // Read the first few instructions of _start_kernel at PA 0x00001084
    println!("\nVerification: _start_kernel at PA 0x00001084:");
    for i in 0..4 {
        let w = vm.bus.read_word(0x1084 + (i * 4) as u64).unwrap_or(0);
        println!("  PA[0x{:04X}] = 0x{:08X}", 0x1084 + i * 4, w);
    }
    
    // Check: is the .data segment correctly loaded?
    // init_task is at VA 0xC1405340 -> PA 0x01405340
    println!("\ninit_task at PA 0x01405340:");
    for i in 0..4 {
        let w = vm.bus.read_word(0x01405340 + (i * 4) as u64).unwrap_or(0);
        println!("  PA[0x{:08X}] = 0x{:08X}", 0x01405340 + i * 4, w);
    }
    
    // Try to find __log_buf symbol
    // nm vmlinux | grep log_buf
    // For now, scan .bss for non-zero data (should be zero if properly cleared)
    println!("\nScanning .bss for non-zero data (first 100 non-zero words):");
    let bss_start = 0x01481200u64; // .data end / .bss start
    let bss_end = 0x014BB8B8u64;
    let mut nonzero_count = 0;
    let mut addr = bss_start;
    while addr < bss_end && nonzero_count < 100 {
        let w = vm.bus.read_word(addr).unwrap_or(0);
        if w != 0 {
            let offset = addr - bss_start;
            let va = 0xC0000000u32 + (addr as u32);
            println!("  .bss+0x{:05X} (VA 0x{:08X}, PA 0x{:08X}) = 0x{:08X}", offset, va, addr, w);
            nonzero_count += 1;
        }
        addr += 4;
    }
    if nonzero_count == 0 {
        println!("  .bss is entirely zero (expected)");
    }
}
