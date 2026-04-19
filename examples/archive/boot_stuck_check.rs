use geometry_os::riscv::RiscvVm;
use std::fs;

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
        10_000_000,
        bootargs,
    )
    .unwrap();

    // Disassemble around the stuck PC
    let stuck_pc = vm.cpu.pc;
    println!("Stuck PC: 0x{:08X}", stuck_pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("mtime: {}", vm.bus.clint.mtime);
    println!();

    // Try to read and disassemble around the stuck area
    println!("=== Code around stuck PC ===");
    for offset in -8..=8i32 {
        let addr = (stuck_pc as i64 + (offset * 4) as i64) as u64;
        if let Ok(word) = vm.bus.read_word(addr) {
            let marker = if addr as u32 == stuck_pc {
                ">>>"
            } else {
                "   "
            };
            println!("{} 0x{:08X}: 0x{:08X}", marker, addr, word);
        }
    }

    // Check register values involved in the BNE
    println!("\n=== Key registers ===");
    println!("x29 (t4): 0x{:08X}", vm.cpu.x[29]);
    println!("x30 (t5): 0x{:08X}", vm.cpu.x[30]);

    // Check SBI calls
    let sbi = &vm.bus.sbi;
    println!("\n=== SBI ===");
    println!("console_output: {} bytes", sbi.console_output.len());
    println!("shutdown: {}", sbi.shutdown_requested);
    if !sbi.console_output.is_empty() {
        println!(
            "First 200 chars: {}",
            String::from_utf8_lossy(&sbi.console_output[..sbi.console_output.len().min(200)])
        );
    }

    // Check UART
    let uart_out: Vec<u8> = vm.bus.uart.drain_tx();
    println!("\n=== UART ===");
    println!("TX buffer: {} bytes", uart_out.len());
    if !uart_out.is_empty() {
        println!(
            "First 200 chars: {}",
            String::from_utf8_lossy(&uart_out[..uart_out.len().min(200)])
        );
    }

    // Check traps
    println!("\n=== Trap counts ===");
    println!("mcause: 0x{:08X}", vm.cpu.csr.mcause);
    println!("scause: 0x{:08X}", vm.cpu.csr.scause);

    // Count unique PCs visited in last 100K steps (sample)
    // This tells us if it's a tight loop
    println!("\n=== Checking for tight loop ===");
    let mut pc_set = std::collections::HashSet::new();
    for _ in 0..100_000 {
        let pc = vm.cpu.pc;
        pc_set.insert(pc);
        let result = vm.step();
        if result != geometry_os::riscv::cpu::StepResult::Ok {
            println!("Non-OK result: {:?}", result);
            break;
        }
    }
    println!("Unique PCs in last 100K instructions: {}", pc_set.len());
}
