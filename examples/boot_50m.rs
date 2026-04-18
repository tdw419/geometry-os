//! Long boot run with 50M instruction limit.
//! Run: cargo run --example boot_50m

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[50m] Starting boot with 50M instruction limit...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        50_000_000,
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
            eprintln!("\n[50m] Done: {} instructions", stats.instructions);
            eprintln!("[50m] PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("[50m] ecall_count={}", vm.cpu.ecall_count);
            eprintln!("[50m] SBI console: {} chars", sbi_str.len());

            // Check memblock
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            eprintln!("[50m] phys_ram_base = 0x{:08X}", prb);

            if !sbi_str.is_empty() {
                eprintln!("\n[50m] Console output:");
                let start = sbi_str.len().saturating_sub(5000);
                eprintln!("{}", &sbi_str[start..]);
            } else {
                eprintln!("[50m] No console output!");
            }

            // Check kernel_map
            let km_pa = vm.bus.read_word(0x00C79E9Cu64).unwrap_or(0);
            let km_vapo = vm.bus.read_word(0x00C79EA4u64).unwrap_or(0);
            eprintln!(
                "[50m] kernel_map: phys_addr=0x{:08X} va_pa_offset=0x{:08X}",
                km_pa, km_vapo
            );
        }
        Err(e) => {
            eprintln!("[50m] Boot error: {:?}", e);
        }
    }
}
