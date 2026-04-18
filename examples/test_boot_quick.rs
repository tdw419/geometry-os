use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();

    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", entry, dtb_addr);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    println!("PC: 0x{:08X}", vm.cpu.pc);

    let max = 1_000_000;
    let report_interval = 100_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;

    while count < max {
        let _ = vm.step();
        count += 1;

        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 10000 {
                println!(
                    "\nSPIN LOOP at PC=0x{:08X} after {} instructions",
                    vm.cpu.pc, count
                );
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;

        if count % report_interval == 0 {
            println!(
                "After {} instrs: PC=0x{:08X} SP=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[2]
            );
        }
    }

    // Drain UART TX buffer
    let uart_bytes = vm.bus.uart.drain_tx();
    let uart_str: String = uart_bytes.iter().map(|&b| b as char).collect();

    println!("\n=== FINAL STATE ===");
    println!("Instructions: {}", count);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!(
        "UART output ({} bytes): {}",
        uart_bytes.len(),
        &uart_str[..uart_str.len().min(500)]
    );
}
