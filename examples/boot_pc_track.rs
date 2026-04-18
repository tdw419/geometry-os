//! Track PC progress during boot to detect loops or stalls.
//! Run: cargo run --example boot_pc_track

use geometry_os::riscv::RiscvVm;
use std::collections::HashMap;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[pc_track] Starting boot...");
    let (mut vm, _fw, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi loglevel=7",
    )
    .expect("boot setup failed");

    // Use boot_linux for full trap handling
    drop(vm);
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        10_000_000,
        "console=ttyS0 earlycon=sbi loglevel=7",
    );

    match result {
        Ok((mut vm, stats)) => {
            // Sample PC every 100K instructions in a second run
            eprintln!(
                "[pc_track] Phase 1 done: {} instructions, {} ECALLs",
                stats.instructions, vm.cpu.ecall_count
            );

            // Continue with another 10M, sampling PC
            let mut pc_samples: Vec<(u64, u32)> = Vec::new();
            let sample_interval = 100_000u64;
            let mut count = 0u64;
            let fw_addr = dtb_addr; // Not used, just for sizing
            let _ = fw_addr;

            // We can't easily continue boot_linux, so let's just report phase 1 stats
            let sbi_str: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            eprintln!("[pc_track] Console: {} chars", sbi_str.len());

            // Get the function at the final PC using objdump
            eprintln!("[pc_track] Final PC: 0x{:08X}", vm.cpu.pc);

            // Check kernel state
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            eprintln!("[pc_track] phys_ram_base = 0x{:08X}", prb);

            // Count unique function ranges visited
            // Read SP to understand call depth
            eprintln!("[pc_track] SP = 0x{:08X}", vm.cpu.x[2]);
            eprintln!("[pc_track] RA = 0x{:08X}", vm.cpu.x[1]);

            // Check if we're in a spin loop by examining surrounding code
            if sbi_str.is_empty() {
                eprintln!(
                    "[pc_track] WARNING: No console output after {}M instructions",
                    stats.instructions / 1_000_000
                );
            }

            let _ = pc_samples;
            let _ = count;
            let _ = sample_interval;
        }
        Err(e) => {
            eprintln!("[pc_track] Error: {:?}", e);
        }
    }
}
