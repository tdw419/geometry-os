fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    // Boot to 200K
    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 200_000, bootargs).unwrap();

    println!("After 200K instructions:");
    println!("  ecall_count: {}", vm.cpu.ecall_count);
    println!("  PC: 0x{:08X}", vm.cpu.pc);
    println!("  privilege: {:?}", vm.cpu.privilege);
    println!("  SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("  UART tx: {} bytes", vm.bus.uart.tx_buf.len());
    println!("  satp: 0x{:08X}", vm.cpu.csr.read(csr::SATP));
    println!("  stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("  mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!(
        "  sstatus (mstatus & mask): 0x{:08X}",
        vm.cpu.csr.mstatus & 0x0007E1DE
    );
    println!("  mepc: 0x{:08X}", vm.cpu.csr.mepc);
    println!("  scause: 0x{:08X}", vm.cpu.csr.scause);
    println!("  sp (x2): 0x{:08X}", vm.cpu.x[2]);

    // Continue to 1M
    for count in 200_000..1_000_000 {
        let step_result = vm.step();
        match step_result {
            StepResult::Ebreak => {
                println!("\nEBREAK at count={}", count);
                break;
            }
            _ => {}
        }
    }

    println!("\nAfter 1M instructions:");
    println!("  ecall_count: {}", vm.cpu.ecall_count);
    println!("  PC: 0x{:08X}", vm.cpu.pc);
    println!("  privilege: {:?}", vm.cpu.privilege);
    println!("  SBI console: {} bytes", vm.bus.sbi.console_output.len());
    println!("  UART tx: {} bytes", vm.bus.uart.tx_buf.len());
    println!("  satp: 0x{:08X}", vm.cpu.csr.read(csr::SATP));
}
