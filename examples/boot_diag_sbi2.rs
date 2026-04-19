// Diagnostic: Check SBI calls and UART output during Linux boot (longer run)
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let max_count: u64 = 10_000_000;
    let mut count: u64 = 0;
    let mut next_report: u64 = 1_000_000;
    let start = Instant::now();

    // Track unique SBI extension types
    let mut sbi_types: std::collections::HashMap<(u32,u32), u32> = std::collections::HashMap::new();

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        let _ = vm.step();

        // Handle DBCN pending writes
        if let Some((phys_addr, num_bytes)) = vm.bus.sbi.dbcn_pending_write.take() {
            for i in 0..num_bytes {
                if let Ok(b) = vm.bus.read_byte(phys_addr + i as u64) {
                    if b != 0 {
                        vm.bus.uart.write_byte(0, b);
                        vm.bus.sbi.console_output.push(b);
                    }
                }
            }
            vm.cpu.x[10] = 0; // SBI_SUCCESS
            vm.cpu.x[11] = num_bytes as u32;
        }

        count += 1;

        if count == next_report {
            let elapsed = start.elapsed();
            let ips = count as f64 / elapsed.as_secs_f64();
            eprintln!("[{}M] PC=0x{:08X} SBI_calls={} UART_bytes={} priv={:?} ({:.0} IPS)",
                count / 1_000_000, vm.cpu.pc, vm.bus.sbi.ecall_log.len(),
                vm.bus.sbi.console_output.len(), vm.cpu.privilege, ips);
            next_report += 1_000_000;
        }
    }

    let elapsed = start.elapsed();

    // Categorize SBI calls
    for &(a7, a6, _a0) in &vm.bus.sbi.ecall_log {
        *sbi_types.entry((a7, a6)).or_insert(0) += 1;
    }

    eprintln!("\n=== Results ({}M instructions, {:.1}s) ===", count / 1_000_000, elapsed.as_secs_f64());
    eprintln!("Total SBI calls: {}", vm.bus.sbi.ecall_log.len());
    eprintln!("SBI call types:");
    for ((a7, a6), cnt) in &sbi_types {
        eprintln!("  a7=0x{:08X} a6={}: {} calls", a7, a6, cnt);
    }

    // Show first few non-PROBE calls
    eprintln!("\nFirst non-BASE calls:");
    for (i, &(a7, a6, a0)) in vm.bus.sbi.ecall_log.iter().enumerate() {
        if a7 != 0x10 || a6 != 3 {
            eprintln!("  SBI #{}: a7=0x{:08X} a6=0x{:08X} a0=0x{:08X}", i, a7, a6, a0);
        }
    }

    // UART output
    let tx = vm.bus.uart.drain_tx();
    eprintln!("\nUART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(5000)]);
    }

    eprintln!("\nCPU: PC=0x{:08X} SATP=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
}
