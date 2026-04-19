use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 nosmp maxcpus=1 loglevel=8";
    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let max_count: u64 = 15_000_000;
    let mut count: u64 = 0;

    while count < max_count && !vm.bus.sbi.shutdown_requested {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Watch for make_task_dead entry (0xC00148B8)
        if vm.cpu.pc == 0xC00148B8 {
            let sp = vm.cpu.x[2];
            let ra = vm.cpu.x[1];
            eprintln!(
                "[make_task_dead] count={} SP=0x{:08X} RA=0x{:08X} a0=0x{:08X}",
                count, sp, ra, vm.cpu.x[10]
            );
            // Read saved RA from stack (SP+12)
            if let Ok(saved_ra) = vm.bus.read_word(sp as u64 + 12) {
                eprintln!("[make_task_dead] caller RA=0x{:08X}", saved_ra);
            }
            // Dump stack frames
            let mut frame_sp = sp as u64;
            for i in 0..8 {
                if let Ok(val) = vm.bus.read_word(frame_sp + 12) {
                    eprintln!("  frame[{}]: SP+12=0x{:08X}", i, val);
                }
                // Walk up: read s0 from current frame, use as next frame's sp
                if let Ok(s0) = vm.bus.read_word(frame_sp + 8) {
                    frame_sp = s0 as u64;
                } else {
                    break;
                }
            }
        }

        vm.step();
        count += 1;

        if count == 5_000_000 || count == 10_000_000 {
            eprintln!("[{}M] PC=0x{:08X}", count / 1_000_000, vm.cpu.pc);
        }
    }

    eprintln!("Done: {} instructions", count);
    eprintln!("Final PC=0x{:08X}", vm.cpu.pc);
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(3000)]);
    }
}
