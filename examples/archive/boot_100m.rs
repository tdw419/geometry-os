use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 loglevel=8";

    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256, // 256MB instead of 512MB to reduce init_unavailable_range time
        100_000_000u64,
        bootargs,
    )
    .unwrap();

    let elapsed = start.elapsed();
    println!(
        "Boot: {} instructions in {:?} ({:.1} Mips)",
        result.instructions,
        elapsed,
        result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    let tx = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("{}\n", s);
    }

    let sbi_out = &vm.bus.sbi.console_output;
    println!("SBI console: {} bytes", sbi_out.len());
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        println!("{}\n", s);
    }
}
