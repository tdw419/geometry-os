use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Use 1M instructions and capture diagnostics
    let (mut vm, br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        1_000_000,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    println!("\n=== Final state ===");
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP=0x{:08X}", vm.cpu.csr.satp);
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
    println!("UART TX: {} bytes", vm.bus.uart.tx_buf.len());
    println!("SBI calls: {}", vm.bus.sbi.console_output.len());

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    println!("SBI output text: {:?}", sbi_str);

    let uart_str: String = vm.bus.uart.tx_buf.iter().map(|&b| b as char).collect();
    println!("UART output text: {:?}", uart_str);

    println!("\n=== Register dump (first 32) ===");
    for i in 0..32 {
        print!("x{:02}=0x{:08X} ", i, vm.cpu.x[i]);
        if (i + 1) % 4 == 0 {
            println!();
        }
    }

    println!("\n=== Key CSRs ===");
    println!("mstatus=0x{:08X}", vm.cpu.csr.mstatus);
    println!("mepc=0x{:08X}", vm.cpu.csr.mepc);
    println!("mcause=0x{:08X}", vm.cpu.csr.mcause);
    println!("mtval=0x{:08X}", vm.cpu.csr.mtval);
    println!("sstatus=0x{:08X}", vm.cpu.csr.mstatus & 0x0002DEAA);
    println!("sepc=0x{:08X}", vm.cpu.csr.sepc);
    println!("scause=0x{:08X}", vm.cpu.csr.scause);
    println!("stval=0x{:08X}", vm.cpu.csr.stval);
    println!("stvec=0x{:08X}", vm.cpu.csr.stvec);
    println!("satp=0x{:08X}", vm.cpu.csr.satp);
}
