/// Trace from 177400 to 177570, logging: SATP changes, SP changes, RA, 
/// and the actual store/load to SP+12 in print_log_buf_usage_stats.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, _fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    // Run to 177400
    for _ in 0..177400 {
        if vm.bus.sbi.shutdown_requested { break; }
        vm.step();
    }

    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut last_sp: u32 = vm.cpu.x[2];

    for i in 0..200 {
        let pc = vm.cpu.pc;
        let sp = vm.cpu.x[2];
        let ra = vm.cpu.x[1];
        let satp = vm.cpu.csr.satp;

        // Log SATP changes
        if satp != last_satp {
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X}", 177400 + i, last_satp, satp);
            last_satp = satp;
        }

        // Log SP changes
        if sp != last_sp && pc >= 0xC0000000 {
            eprintln!("[{}] SP: 0x{:08X} -> 0x{:08X} at PC=0x{:08X}", 177400 + i, last_sp, sp, pc);
            last_sp = sp;
        }

        // Log when we're in print_log_buf_usage_stats (0xC000305E - 0xC000308B)
        if pc >= 0xC000305E && pc <= 0xC000308B {
            eprintln!("[{}] PC=0x{:08X} SP=0x{:08X} RA=0x{:08X}", 177400 + i, pc, sp, ra);
        }

        vm.step();

        if vm.bus.sbi.shutdown_requested { break; }
    }
}
