use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs =
        "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=1 loglevel=8";

    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        128,
        200_000_000,
        bootargs,
    )
    .unwrap();

    eprintln!(
        "Executed {} instructions, entry=0x{:08X}, dtb=0x{:08X}",
        result.instructions, result.entry, result.dtb_addr
    );
    eprintln!("Final PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SATP=0x{:08X} stvec=0x{:08X} sepc=0x{:08X} scause=0x{:08X}",
        vm.cpu.csr.satp, vm.cpu.csr.stvec, vm.cpu.csr.sepc, vm.cpu.csr.scause);
    eprintln!("mepc=0x{:08X} mcause=0x{:08X} medeleg=0x{:04X}",
        vm.cpu.csr.mepc, vm.cpu.csr.mcause, vm.cpu.csr.medeleg);
    eprintln!("MIE=0x{:08X} MIP=0x{:08X}",
        vm.cpu.csr.mie, vm.cpu.csr.mip);
    eprintln!("SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
        vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]);
    eprintln!("SBI console_output: {} bytes", vm.bus.sbi.console_output.len());
    eprintln!("CLINT mtime=0x{:016X} mtimecmp=0x{:016X}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp);

    // Check if UART at 0x10000000 was written to directly (earlycon)
    // by reading the UART LSR (Line Status Register at offset 5)
    if let Ok(lsr) = vm.bus.read_byte(0x10000005) {
        eprintln!("UART LSR=0x{:02X} (bit 5=THRE, bit 6=TEMT)", lsr);
    }

    let uart = vm.bus.uart.drain_tx();
    eprintln!("UART drain: {} bytes", uart.len());
    eprintln!("UART write_count: {}", vm.bus.uart.write_count);
    eprintln!("SBI ecall_log: {} entries", vm.bus.sbi.ecall_log.len());
    for (i, &(a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().enumerate().take(30) {
        eprintln!("  SBI #{}: a7=0x{:08X} a6={} a0=0x{:08X}", i, a7, a6, a0);
    }
    if !uart.is_empty() {
        let s = String::from_utf8_lossy(&uart);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }
}
