use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = std::fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").unwrap();

    let (mut vm, _result) = RiscvVm::boot_linux(
        &kernel,
        Some(&initramfs),
        64,
        5_000_000,
        "console=ttyS0 earlycon=sbi",
    )
    .unwrap();

    println!(
        "Final medeleg: 0x{:08X} (bit 9/ECALL_S = {})",
        vm.cpu.csr.medeleg,
        (vm.cpu.csr.medeleg >> 9) & 1
    );
    println!("Final mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
    println!("Final stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!(
        "SBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("First 500 chars: {}", &s[..s.len().min(500)]);
    }
}
