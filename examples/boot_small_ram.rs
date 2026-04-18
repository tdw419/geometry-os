//! Boot with smaller RAM (64MB) to speed up page init.
//! Also ticks CLINT faster (10x) to make udelay complete sooner.
//! Run: cargo run --example boot_small_ram

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    // Try with 64MB RAM to reduce page init time
    eprintln!("[small] Booting with 64MB RAM, 100M instruction limit...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        64, // 64MB RAM
        100_000_000,
        "console=ttyS0 earlycon=sbi loglevel=7",
    );

    match result {
        Ok((mut vm, stats)) => {
            let sbi_str: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            eprintln!("\n[small] Done: {} instructions", stats.instructions);
            eprintln!("[small] PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("[small] ecall_count={}", vm.cpu.ecall_count);
            eprintln!("[small] SBI console: {} chars", sbi_str.len());

            if !sbi_str.is_empty() {
                eprintln!("\n[small] Console output:");
                eprintln!("{}", &sbi_str[..sbi_str.len().min(5000)]);
            } else {
                eprintln!("[small] No console output!");
            }

            // Check key kernel state
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            eprintln!("[small] phys_ram_base = 0x{:08X}", prb);
            eprintln!("[small] mtime = {}", vm.bus.clint.mtime);
            eprintln!("[small] mtimecmp = {}", vm.bus.clint.mtimecmp);
        }
        Err(e) => {
            eprintln!("[small] Boot error: {:?}", e);
        }
    }
}
