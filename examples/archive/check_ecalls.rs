// Capture ALL ECALLs before the panic to understand boot flow
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        initramfs.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi",
    )
    .unwrap();

    let panic_loop: u32 = 0xC000278A;
    let mut count: u64 = 0;
    let max: u64 = 400_000;
    let mut ecall_count = 0u64;
    let mut last_ecalls: Vec<(u64, u32, u32, u32, u32)> = Vec::new(); // (count, pc, a7, a6, a0)

    while count < max {
        let pc = vm.cpu.pc;
        let _result = vm.step();
        count += 1;

        // Check for ECALLs by watching ecall_count
        if vm.cpu.ecall_count > ecall_count {
            ecall_count = vm.cpu.ecall_count;
            last_ecalls.push((count, pc, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]));
            // Print first 30 ECALLs
            if ecall_count <= 30 {
                let ext = vm.cpu.x[17];
                let func = vm.cpu.x[16];
                let ext_name = match ext {
                    0x02 => "CONSOLE",
                    0x10 => "RFENCE",
                    0x05 => "RESET",
                    0x08 => "HSM",
                    0x04 => "IPI",
                    0x00 => "SET_TIMER",
                    _ => "UNKNOWN",
                };
                println!("[ecall] #{} at count={}, PC=0x{:08X}: ext=0x{:02X}({}) func={} a0=0x{:X} a1=0x{:X}",
                    ecall_count, count, pc, ext, ext_name, func, vm.cpu.x[10], vm.cpu.x[11]);
            }
        }

        if vm.cpu.pc >= panic_loop && vm.cpu.pc <= 0xC00027A0 {
            println!(
                "\n[panic] Hit panic loop at count={}, PC=0x{:08X}",
                count, vm.cpu.pc
            );
            println!("[panic] Total ECALLs: {}", ecall_count);
            println!("[panic] Last 5 ECALLs:");
            for (ec_count, ec_pc, a7, a6, a0) in last_ecalls.iter().rev().take(5) {
                println!(
                    "  count={} PC=0x{:08X} a7=0x{:X} a6=0x{:X} a0=0x{:X}",
                    ec_count, ec_pc, a7, a6, a0
                );
            }
            break;
        }
    }

    if count >= max {
        println!(
            "[info] Total instructions: {}, ECALLs: {}, PC=0x{:08X}",
            count, ecall_count, vm.cpu.pc
        );
    }
}
