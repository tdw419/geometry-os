use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_data = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs_data = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 loglevel=7";
    let start = Instant::now();

    let (vm, result) = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        100_000_000, // 100M instructions
        bootargs,
    )
    .expect("boot should succeed");

    let elapsed = start.elapsed();
    let mips = result.instructions as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    eprintln!(
        "{} instructions in {:?} = {:.2} MIPS",
        result.instructions, elapsed, mips
    );
    eprintln!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);

    // Full SBI + UART output
    let sbi = String::from_utf8_lossy(&vm.bus.sbi.console_output);
    let uart = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
    let combined = format!("{}{}", sbi, uart);

    // Print the FULL output
    eprintln!("=== CONSOLE OUTPUT ({} bytes) ===", combined.len());
    eprintln!("{}", combined);
    eprintln!("=== END CONSOLE OUTPUT ===");

    eprintln!("SBI ecall count: {}", vm.bus.sbi.ecall_log.len());
    eprintln!("CPU ecall count: {}", vm.cpu.ecall_count);

    // Timer state
    let mip = vm.cpu.csr.mip;
    let mie = vm.cpu.csr.mie;
    eprintln!("MIP: 0x{:08X}, MIE: 0x{:08X}", mip, mie);
    eprintln!(
        "mtime: {}, mtimecmp: {}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp
    );
    eprintln!("mideleg: 0x{:08X}", vm.cpu.csr.mideleg);

    // Check for key boot messages
    let output_lower = combined.to_lowercase();
    let milestones = [
        ("kernel_init", "kernel_init"),
        ("freeing unused", "freeing unused kernel"),
        ("run /init", "run /init"),
        ("init", "/init"),
        ("shell", "# "),
        ("busybox", "busybox"),
        ("panic", "panic"),
        ("idle", "cpu_idle"),
        ("sched_clock", "sched_clock"),
    ];
    for (name, needle) in &milestones {
        let found = output_lower.contains(needle);
        eprintln!(
            "Milestone '{}': {}",
            name,
            if found { "FOUND" } else { "not found" }
        );
    }
}
