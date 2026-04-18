//! Run the actual boot_linux() with a high instruction count to see how far we get.
//! Uses the full M-mode trap handling from boot.rs.
//! Run: cargo run --example boot_long_run

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[long_run] Starting boot with 5M instruction limit...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        5_000_000,
        "console=ttyS0 earlycon=sbi loglevel=7",
    );

    match result {
        Ok((vm, stats)) => {
            let sbi_str: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            eprintln!(
                "\n[long_run] Boot completed: {} instructions",
                stats.instructions
            );
            eprintln!(
                "[long_run] PC=0x{:08X} priv={:?}",
                vm.cpu.pc, vm.cpu.privilege
            );
            eprintln!("[long_run] DTB addr=0x{:08X}", stats.dtb_addr);
            if !sbi_str.is_empty() {
                eprintln!("\n[long_run] Console output ({} chars):", sbi_str.len());
                // Print last 3000 chars
                let start = sbi_str.len().saturating_sub(3000);
                eprintln!("{}", &sbi_str[start..]);
            } else {
                eprintln!("[long_run] No console output!");
            }
        }
        Err(e) => {
            eprintln!("[long_run] Boot error: {:?}", e);
        }
    }
}
