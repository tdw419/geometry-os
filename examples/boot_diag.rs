// Diagnostic: boot Linux and trace trap behavior.
// cargo run --example boot_diag

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    println!("=== Linux Boot Diagnostic (500K insns) ===\n");

    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        500_000,
        bootargs,
    );

    match result {
        Ok((mut vm, r)) => {
            println!("Instructions: {}", r.instructions);
            println!("Entry: 0x{:08X}", r.entry);
            println!("DTB:   0x{:08X}", r.dtb_addr);
            println!("Final PC: 0x{:08X}", vm.cpu.pc);
            println!("Final privilege: {:?}", vm.cpu.privilege);

            println!("\nKey CSRs:");
            println!("  mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
            println!("  mepc:    0x{:08X}", vm.cpu.csr.mepc);
            println!("  mcause:  0x{:08X}", vm.cpu.csr.mcause);
            println!("  mtval:   0x{:08X}", vm.cpu.csr.mtval);
            println!("  sepc:    0x{:08X}", vm.cpu.csr.sepc);
            println!("  scause:  0x{:08X}", vm.cpu.csr.scause);
            println!("  stval:   0x{:08X}", vm.cpu.csr.stval);
            println!("  stvec:   0x{:08X}", vm.cpu.csr.stvec);
            println!("  satp:    0x{:08X}", vm.cpu.csr.satp);
            println!("  medeleg: 0x{:08X}", vm.cpu.csr.medeleg);
            println!("  mideleg: 0x{:08X}", vm.cpu.csr.mideleg);

            // UART
            let mut bridge = geometry_os::riscv::bridge::UartBridge::new();
            let mut canvas = vec![0u32; 128 * 80];
            bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);

            let mut has_uart = false;
            println!("\n--- UART Output ---");
            for row in 0..128 {
                let mut line = String::new();
                let mut has_content = false;
                for col in 0..80 {
                    let ch = canvas[row * 80 + col];
                    if ch != 0 {
                        has_content = true;
                        if ch >= 32 && ch < 127 {
                            line.push(char::from_u32(ch).unwrap_or('.'));
                        } else {
                            line.push('.');
                        }
                    } else {
                        line.push(' ');
                    }
                }
                if has_content {
                    has_uart = true;
                    println!("  {:3}: {}", row, line.trim_end());
                }
            }
            if !has_uart {
                println!("  (no UART output)");
            }
        }
        Err(e) => {
            eprintln!("Boot failed: {}", e);
            std::process::exit(1);
        }
    }
}
