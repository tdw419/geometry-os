use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (vm, br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    println!("\n=== Final state ===");
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("Instructions: {}", br.instructions);
    println!("SATP=0x{:08X}", vm.cpu.csr.satp);
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    if !sbi_str.is_empty() {
        println!("\nSBI console output ({} bytes):", sbi_str.len());
        println!("{}", sbi_str);
    }

    let uart_str: String = vm.bus.uart.tx_buf.iter().map(|&b| b as char).collect();
    if !uart_str.is_empty() {
        println!("\nUART output ({} bytes):", uart_str.len());
        // Show first 500 chars
        let preview: String = uart_str.chars().take(500).collect();
        println!("{}", preview);
    }

    println!("\n=== Key CSRs ===");
    println!("mstatus=0x{:08X}", vm.cpu.csr.mstatus);
    println!("mepc=0x{:08X}", vm.cpu.csr.mepc);
    println!("mcause=0x{:08X}", vm.cpu.csr.mcause);
    println!("sepc=0x{:08X}", vm.cpu.csr.sepc);
    println!("scause=0x{:08X}", vm.cpu.csr.scause);
    println!("stvec=0x{:08X}", vm.cpu.csr.stvec);
}
