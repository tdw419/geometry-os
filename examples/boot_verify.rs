fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Check reserved regions after pre-populate
    let res_regions_pa: u64 = 0x00803A8C;
    println!("Reserved regions at PA 0x{:08X}:", res_regions_pa);
    for i in 0..5 {
        let base = vm.bus.read_word(res_regions_pa + i * 8).unwrap_or(0);
        let size = vm.bus.read_word(res_regions_pa + i * 8 + 4).unwrap_or(0);
        if base == 0 && size == 0 {
            println!("  [{}]: empty", i);
        } else {
            println!(
                "  [{}]: base=0x{:08X} size=0x{:08X} ({}KB)",
                i,
                base,
                size,
                size / 1024
            );
        }
    }
    println!(
        "reserved.cnt = {}",
        vm.bus.read_word(0x00803448 + 28).unwrap_or(0)
    );

    // Now check what happens at the boot_quick_check's 20M instructions
    // Let's do a quick boot test with more instructions
    let (mut vm2, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        50_000_000,
        bootargs,
    )
    .unwrap();

    println!(
        "\nBoot: {} instr, PC=0x{:08X}",
        boot_result.instructions, vm2.cpu.pc
    );
    println!("UART: {} chars", vm2.bus.uart.tx_buf.len());
    println!("SBI console: {} chars", vm2.bus.sbi.console_output.len());

    // Show first SBI output
    if !vm2.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm2.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        println!("\nSBI output:\n{}", preview);
    }
}
