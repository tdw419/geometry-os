use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let max = 500_000u64;
    let sample_interval = 50_000u64;
    let mut count = 0u64;
    let mut last_pcs: Vec<(u64, u32)> = Vec::new();

    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;

        if count % sample_interval == 0 {
            eprintln!("[{}] PC=0x{:08X} priv={:?} scause=0x{:08X} sepc=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.scause, vm.cpu.csr.sepc);
        }
        if count > max - 20 {
            last_pcs.push((count, vm.cpu.pc));
        }
    }

    eprintln!("\nFinal 20 PCs:");
    for (c, pc) in &last_pcs {
        eprintln!("  [{}] PC=0x{:08X}", c, pc);
    }

    eprintln!("\nUART: {} bytes, SBI: {} bytes", vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
}
