use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Run 20M instructions, then trace what the CPU is doing
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    let pc_20m = vm.cpu.pc;
    println!("PC at 20M: 0x{:08X}", pc_20m);

    // Run 1000 more instructions, sample PCs
    let mut pc_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    use geometry_os::riscv::cpu::StepResult;
    for _ in 0..10000 {
        *pc_counts.entry(vm.cpu.pc).or_insert(0) += 1;
        vm.step();
    }

    let mut sorted: Vec<_> = pc_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nTop 20 PCs (10000 instructions after 20M):");
    for (pc, count) in sorted.iter().take(20) {
        println!(
            "  PC=0x{:08X}: {} times ({:.1}%)",
            pc,
            count,
            *count as f64 / 100.0
        );
    }

    println!("\nFinal PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);

    // Check if it's in WFI or similar
    println!("SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("UART tx_buf: {} bytes", vm.bus.uart.tx_buf.len());
}
