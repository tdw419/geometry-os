/// Diagnostic: High-frequency PC trace around the panic point (count 320K-340K).
/// Also track ecall_count to verify ECALLs.
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

    while count < max {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;

        // High-frequency trace around the panic zone
        if count >= 320_000 && count <= 340_000 {
            let interval = if count < 328_000 { 5_000 } else { 500 };
            if count % interval == 0 {
                eprintln!(
                    "[trace] {} PC=0x{:08X} ecall={} uart={}",
                    count,
                    vm.cpu.pc,
                    vm.cpu.ecall_count,
                    vm.bus.uart.tx_buf.len()
                );
            }
        }

        // Catch entry to panic()
        if vm.cpu.pc == 0xC000_252E {
            eprintln!(
                "[PANIC] count={} PC=0x{:08X} ecall={} uart={}",
                count,
                vm.cpu.pc,
                vm.cpu.ecall_count,
                vm.bus.uart.tx_buf.len()
            );
            let a0 = vm.cpu.x[10];
            let ra = vm.cpu.x[1];
            eprintln!("[PANIC] a0=0x{:08X} ra=0x{:08X}", a0, ra);

            // Read format string
            let fmt_pa = if a0 >= 0xC000_0000 {
                Some((a0 - 0xC000_0000) as u64)
            } else {
                Some(a0 as u64)
            };
            if let Some(pa) = fmt_pa {
                let mut chars = Vec::new();
                for i in 0..256 {
                    if let Ok(word) = vm.bus.read_word(pa + i * 4) {
                        for b in 0..4 {
                            let byte = (word >> (b * 8)) & 0xFF;
                            if byte == 0 {
                                break;
                            }
                            chars.push(char::from_u32(byte).unwrap_or('?'));
                        }
                        if chars.last() == Some(&'\0') {
                            break;
                        }
                    }
                }
                let fmt_str: String = chars.iter().collect();
                eprintln!("[PANIC] message: {:?}", fmt_str);
            }
            break;
        }
    }

    eprintln!(
        "\n[done] {} instr, ecall={}, uart={}",
        count,
        vm.cpu.ecall_count,
        vm.bus.uart.tx_buf.len()
    );
}
