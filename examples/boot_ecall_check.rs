//! Check if ECALLs are happening during boot by reading ecall_count.
//! Run: cargo run --example boot_ecall_check

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[ecall] Starting boot with 5M instruction limit...");
    let result = RiscvVm::boot_linux(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        5_000_000,
        "console=ttyS0 earlycon=sbi loglevel=7",
    );

    match result {
        Ok((mut vm, _stats)) => {
            eprintln!("[ecall] ecall_count = {}", vm.cpu.ecall_count);
            eprintln!(
                "[ecall] SBI console_output len = {}",
                vm.bus.sbi.console_output.len()
            );
            eprintln!("[ecall] syscall_log len = {}", vm.bus.syscall_log.len());
            eprintln!(
                "[ecall] PC = 0x{:08X} priv = {:?}",
                vm.cpu.pc, vm.cpu.privilege
            );

            // Print first 20 chars of console output if any
            if !vm.bus.sbi.console_output.is_empty() {
                let s: String = vm
                    .bus
                    .sbi
                    .console_output
                    .iter()
                    .map(|&b| b as char)
                    .collect();
                eprintln!("[ecall] First 200 chars: {}", &s[..s.len().min(200)]);
            }

            // Print syscall log summary
            if !vm.bus.syscall_log.is_empty() {
                eprintln!("[ecall] First 10 syscalls:");
                for (i, evt) in vm.bus.syscall_log.iter().take(10).enumerate() {
                    eprintln!(
                        "  [{}] nr={} name={} ret={:?}",
                        i, evt.nr, evt.name, evt.ret
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("[ecall] Boot error: {:?}", e);
        }
    }
}
