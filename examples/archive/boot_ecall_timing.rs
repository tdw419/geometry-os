/// Diagnostic: Track exactly when each ECALL happens and what the kernel is doing.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel = std::fs::read(kernel_path).expect("kernel not found");
    let initramfs = std::fs::read(initramfs_path).expect("initramfs not found");

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        128,
        "console=ttyS0 earlycon=sbi panic=5 quiet nosmp",
    )
    .expect("boot setup failed");

    let mut count: u64 = 0;
    let max: u64 = 400_000;
    let mut prev_ecall_count: u64 = 0;

    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;

        // Detect new ECALLs
        if vm.cpu.ecall_count > prev_ecall_count {
            let new_ecalls = vm.cpu.ecall_count - prev_ecall_count;
            for i in 0..new_ecalls {
                let log_idx = (prev_ecall_count + i as u64) as usize;
                if log_idx < vm.bus.sbi.ecall_log.len() {
                    let (a7, a6, a0) = vm.bus.sbi.ecall_log[log_idx];
                    eprintln!(
                        "[ECALL] count={} PC_before=0x{:08X} a7=0x{:02X} a6={} a0=0x{:08X}",
                        count, vm.cpu.pc, a7, a6, a0
                    );
                }
            }
            prev_ecall_count = vm.cpu.ecall_count;
        }

        // Catch panic
        if vm.cpu.pc == 0xC000_252E {
            eprintln!(
                "[PANIC] count={} ecall={} uart={}",
                count,
                vm.cpu.ecall_count,
                vm.bus.uart.tx_buf.len()
            );
            break;
        }
    }

    eprintln!("[done] {} instr, ecall={}", count, vm.cpu.ecall_count);
}
