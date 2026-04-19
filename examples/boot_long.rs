use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    // Suppress boot.rs eprintln by redirecting
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 512, bootargs,
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 50_000_000; // 50M instructions
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut prev_pc: u32 = 0;
    let mut same_pc_count: u64 = 0;
    let mut reported_loops: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut next_report: u64 = 5_000_000;
    let mut prev_report_pc: u32 = 0;
    let start = std::time::Instant::now();

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                sbi_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();

        // Detect PC loops
        if vm.cpu.pc == prev_pc {
            same_pc_count += 1;
            if same_pc_count == 1000 && !reported_loops.contains(&vm.cpu.pc) {
                eprintln!("[LOOP] Stuck at PC=0x{:08X} for 1000+ instructions at count={}", vm.cpu.pc, count);
                reported_loops.insert(vm.cpu.pc);
            }
        } else {
            same_pc_count = 0;
        }

        count += 1;

        if count == next_report {
            let elapsed = start.elapsed();
            let ips = count as f64 / elapsed.as_secs_f64();
            let progress = if vm.cpu.pc != prev_report_pc { "NEW" } else { "SAME" };
            eprintln!("[{}M] PC=0x{:08X} SBI={} elapsed={:.1}s ips={:.0} {}",
                count / 1_000_000, vm.cpu.pc, sbi_count, elapsed.as_secs_f64(), ips, progress);
            prev_report_pc = vm.cpu.pc;
            next_report += 5_000_000;
        }

        prev_pc = vm.cpu.pc;
    }

    eprintln!("\n=== Final ===");
    eprintln!("Instructions: {}", count);
    eprintln!("SBI calls: {}", sbi_count);
    eprintln!("PC loops found: {}", reported_loops.len());
    for pc in &reported_loops {
        eprintln!("  0x{:08X}", pc);
    }

    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(2000)]);
    }
}
