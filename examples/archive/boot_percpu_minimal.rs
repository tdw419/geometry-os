// Minimal diagnostic: capture the first fault forwarded to S-mode
// to find the REAL stval that leads to "Attempted to kill the idle task!"
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
        20_000_000,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    println!("Instructions: {}", br.instructions);
    println!("Entry: 0x{:08X}", br.entry);
    println!("DTB addr: 0x{:016X}", br.dtb_addr);
    println!("ECALLs: {}", vm.cpu.ecall_count);
    println!("Final PC: 0x{:08X}", vm.cpu.pc);
    println!("UART output: {} bytes", vm.bus.sbi.console_output.len());
}
