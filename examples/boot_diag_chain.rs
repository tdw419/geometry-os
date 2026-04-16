/// Chain boot diagnostic: uses production boot_linux() with long run and UART capture.
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    // Use production boot path with 50M instructions
    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        50_000_000,
        bootargs,
    );

    match result {
        Ok((mut vm, boot_result)) => {
            eprintln!("[diag] Boot result: instructions={}, entry=0x{:08X}, dtb_addr=0x{:08X}",
                boot_result.instructions, boot_result.entry, boot_result.dtb_addr);

            // Print UART output
            let sbi_output = &vm.bus.sbi.console_output;
            if !sbi_output.is_empty() {
                let s = String::from_utf8_lossy(sbi_output);
                eprintln!("[diag] SBI console output ({} bytes):\n{}", sbi_output.len(), s);
            }

            let uart_tx = vm.bus.uart.drain_tx();
            if !uart_tx.is_empty() {
                let s = String::from_utf8_lossy(&uart_tx);
                eprintln!("[diag] UART TX output ({} bytes):\n{}", uart_tx.len(), s);
            }

            // Final state
            eprintln!("[diag] Final PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("[diag] satap=0x{:08X} stvec=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
        }
        Err(e) => {
            eprintln!("[diag] Boot error: {:?}", e);
        }
    }
}
