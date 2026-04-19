//! Check _dtb_early_va and _dtb_early_pa during boot.
use geometry_os::riscv::RiscvVm;
fn main() {
    let kernel_data = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").expect("kernel");
    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, None, 512, "console=ttyS0 earlycon=sbi nosmp")
            .expect("setup");

    // _dtb_early_va at VA 0xC0801008 -> PA 0x00801008
    // _dtb_early_pa at VA 0xC080100C -> PA 0x0080100C
    let dtb_early_va_pa: u64 = 0x00801008;
    let dtb_early_pa_pa: u64 = 0x0080100C;
    let ibp_pa: u64 = 0x00C7A380;
    let ibp_pa2: u64 = 0x00C7A3B0;

    println!("BEFORE boot:");
    println!(
        "  _dtb_early_va (PA 0x{:08X}): 0x{:08X}",
        dtb_early_va_pa,
        vm.bus.read_word(dtb_early_va_pa).unwrap_or(0)
    );
    println!(
        "  _dtb_early_pa (PA 0x{:08X}): 0x{:08X}",
        dtb_early_pa_pa,
        vm.bus.read_word(dtb_early_pa_pa).unwrap_or(0)
    );
    println!(
        "  initial_boot_params (PA 0x{:08X}): 0x{:08X}",
        ibp_pa,
        vm.bus.read_word(ibp_pa).unwrap_or(0)
    );
    println!(
        "  initial_boot_params_pa (PA 0x{:08X}): 0x{:08X}",
        ibp_pa2,
        vm.bus.read_word(ibp_pa2).unwrap_or(0)
    );
    println!("  DTB at PA 0x{:08X}", dtb_addr);

    // Add _dtb_early_va and _dtb_early_pa to protected_addrs too
    vm.bus
        .protected_addrs
        .push((dtb_early_va_pa, dtb_addr as u32)); // _dtb_early_va = DTB VA (use PA, identity mapped)
    vm.bus
        .protected_addrs
        .push((dtb_early_pa_pa, dtb_addr as u32)); // _dtb_early_pa = DTB PA

    // Run 5M instructions
    for _ in 0..5_000_000u64 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        vm.bus.tick_clint_n(100);
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }

    println!("\nAFTER 5M instructions:");
    println!(
        "  _dtb_early_va (PA 0x{:08X}): 0x{:08X}",
        dtb_early_va_pa,
        vm.bus.read_word(dtb_early_va_pa).unwrap_or(0)
    );
    println!(
        "  _dtb_early_pa (PA 0x{:08X}): 0x{:08X}",
        dtb_early_pa_pa,
        vm.bus.read_word(dtb_early_pa_pa).unwrap_or(0)
    );
    println!(
        "  initial_boot_params (PA 0x{:08X}): 0x{:08X}",
        ibp_pa,
        vm.bus.read_word(ibp_pa).unwrap_or(0)
    );
    println!(
        "  SBI console: {} bytes, ECALLs: {}",
        vm.bus.sbi.console_output.len(),
        vm.cpu.ecall_count
    );
}
