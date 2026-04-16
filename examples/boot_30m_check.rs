use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, boot_result) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, 30_000_000, bootargs).unwrap();

    println!("Boot: {} instr, PC=0x{:08X}", boot_result.instructions, vm.cpu.pc);
    println!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(1000).collect();
        println!("UART:\n{}", preview);
    }
    println!("TLB entries: {}", vm.cpu.tlb.valid_count());

    // Check final state
    let satp = vm.cpu.csr.read(csr::SATP);
    let stvec = vm.cpu.csr.read(csr::STVEC);
    println!("SATP=0x{:08X} STVEC=0x{:08X}", satp, stvec);
    println!("Privilege: {:?}", vm.cpu.privilege);

    // Check a few key registers
    println!("SP=0x{:08X} TP=0x{:08X} RA=0x{:08X}", vm.cpu.x[2], vm.cpu.x[4], vm.cpu.x[1]);
}
