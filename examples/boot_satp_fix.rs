/// Quick diagnostic: test the virtual SATP fixup.
/// Uses boot_linux() which has the full trap forwarding + SATP change detection.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;

    match RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        bootargs,
    ) {
        Ok((vm, result)) => {
            eprintln!("=== RESULT ===");
            eprintln!("Instructions: {}", result.instructions);
            eprintln!("Entry: 0x{:08X}", result.entry);
            eprintln!("DTB addr: 0x{:08X}", result.dtb_addr);
            eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);
            eprintln!("Final priv: {:?}", vm.cpu.privilege);
            eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
            eprintln!("STVEC: 0x{:08X}", vm.cpu.csr.stvec);
            eprintln!("SP: 0x{:08X}", vm.cpu.x[2]);
            eprintln!("RA: 0x{:08X}", vm.cpu.x[1]);
            eprintln!("GP: 0x{:08X}", vm.cpu.x[3]);

            // Print UART output
            let uart_output: String = vm
                .bus
                .sbi
                .console_output
                .iter()
                .map(|&b| b as char)
                .collect();
            if !uart_output.is_empty() {
                eprintln!("\n=== UART OUTPUT (first 2000 chars) ===");
                eprintln!("{}", &uart_output[..uart_output.len().min(2000)]);
            }

            // Print SBI stats
            eprintln!("\n=== SBI STATS ===");
            eprintln!("Shutdown requested: {}", vm.bus.sbi.shutdown_requested);
            eprintln!("Console output bytes: {}", vm.bus.sbi.console_output.len());

            // Print MMU log summary
            let satp_writes: Vec<_> = vm
                .bus
                .mmu_log
                .iter()
                .filter_map(|e| {
                    if let geometry_os::riscv::mmu::MmuEvent::SatpWrite { old, new } = e {
                        Some((*old, *new))
                    } else {
                        None
                    }
                })
                .collect();
            eprintln!("\n=== SATP WRITES ({}) ===", satp_writes.len());
            for (i, (old, new)) in satp_writes.iter().enumerate().take(20) {
                eprintln!("  [{}] 0x{:08X} -> 0x{:08X}", i, old, new);
            }
        }
        Err(e) => {
            eprintln!("Boot failed: {:?}", e);
        }
    }
}
