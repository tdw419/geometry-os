//! Diagnostic: check if protected_addrs survive boot.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, None, 512, "console=ttyS0 earlycon=sbi nosmp")
            .expect("setup");

    // Check initial_boot_params BEFORE boot
    let ibp_pa = 0x00C7A380u64;
    let ibp_before = vm.bus.read_word(ibp_pa).unwrap_or(0);
    let ibp_byte0 = vm.bus.read_byte(ibp_pa).unwrap_or(0);
    println!(
        "BEFORE boot: initial_boot_params (word)=0x{:08X}",
        ibp_before
    );
    println!(
        "BEFORE boot: initial_boot_params (byte0)=0x{:02X}",
        ibp_byte0
    );

    // Check how many protected_addrs we have
    println!(
        "Protected addresses: {} entries",
        vm.bus.protected_addrs.len()
    );
    for &(pa, val) in &vm.bus.protected_addrs {
        println!("  PA 0x{:08X} -> 0x{:08X}", pa, val);
    }

    // Run 5M instructions
    let max = 5_000_000u64;
    for _ in 0..max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }

    // Check AFTER
    let ibp_after_word = vm.bus.read_word(ibp_pa).unwrap_or(0);
    let ibp_after_byte = vm.bus.read_byte(ibp_pa).unwrap_or(0);
    println!("\nAFTER 5M instructions:");
    println!(
        "  initial_boot_params (word)=0x{:08X} (expect DTB PA 0x{:08X})",
        ibp_after_word, dtb_addr as u32
    );
    println!("  initial_boot_params (byte0)=0x{:02X}", ibp_after_byte);

    // Check via different paths
    let ibp_via_half = vm.bus.read_half(ibp_pa).unwrap_or(0) as u32;
    let ibp_via_half_hi = vm.bus.read_half(ibp_pa + 2).unwrap_or(0) as u32;
    println!(
        "  initial_boot_params (half_lo)=0x{:04X} (half_hi)=0x{:04X}",
        ibp_via_half, ibp_via_half_hi
    );

    // Check DTB readability
    let dtb_magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    println!(
        "\n  DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)",
        dtb_addr, dtb_magic
    );

    // Check SBI output
    println!(
        "\n  SBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    println!("  UART output: checking...");
    let mut uart_count = 0;
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            _ => uart_count += 1,
        }
    }
    println!("  UART output: {} bytes", uart_count);
    println!("  ECALL count: {}", vm.cpu.ecall_count);
    println!("  PC: 0x{:08X}", vm.cpu.pc);
}
