// Diagnostic: capture kernel panic message
// The kernel writes panic messages to the SBI console before entering the blink loop.
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64, // 20M instructions
        bootargs,
    )
    .unwrap();

    let elapsed = start.elapsed();
    println!(
        "Boot: {} instructions in {:?}",
        result.instructions, elapsed
    );
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    // SBI console output
    println!(
        "\nSBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("{}", s);
    }

    // UART output
    let tx = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("{}", s);
    }

    // Syscall log
    println!("Syscalls: {}", vm.bus.syscall_log.len());
    for (i, sc) in vm.bus.syscall_log.iter().take(10).enumerate() {
        println!(
            "  [{}] {} (nr={}) a0={:#x} a1={:#x}",
            i, sc.name, sc.nr, sc.args[0], sc.args[1]
        );
    }

    // Check MMU log for page faults
    let page_faults: Vec<_> = vm
        .bus
        .mmu_log
        .iter()
        .filter(|e| matches!(e, geometry_os::riscv::mmu::MmuEvent::PageFault { .. }))
        .collect();
    println!("\nPage faults: {}", page_faults.len());
    for (i, ev) in page_faults.iter().take(10).enumerate() {
        println!("  [{}] {:?}", i, ev);
    }

    // Check RA (return address) - if in panic, RA shows where panic was called
    println!("\nSP: 0x{:08X}", vm.cpu.x[2]);
    println!("RA: 0x{:08X}", vm.cpu.x[1]);
    println!("GP: 0x{:08X}", vm.cpu.x[3]);
    println!("TP: 0x{:08X}", vm.cpu.x[4]);
}
