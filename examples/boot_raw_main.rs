use geometry_os::riscv::{cpu::StepResult, RiscvVm};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    eprintln!(
        "Setup done: entry=0x{:08X} fw_addr=0x{:X} dtb=0x{:X}",
        entry, fw_addr, dtb_addr
    );
    eprintln!(
        "Initial: PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );

    // Raw stepping -- no trap forwarding, no SATP injection
    let mut count = 0u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut sbi_count = 0;
    let mut uart_bytes = 0;

    while count < 10_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let result = vm.step();

        // Check SATP
        if vm.cpu.csr.satp != last_satp {
            let ppn = vm.cpu.csr.satp & 0x3FFFFF;
            eprintln!(
                "[{}] SATP -> 0x{:08X} (ppn=0x{:X} pa=0x{:X}) PC=0x{:08X}",
                count,
                vm.cpu.csr.satp,
                ppn,
                ppn as u64 * 4096,
                vm.cpu.pc
            );
            last_satp = vm.cpu.csr.satp;
        }

        // Count UART output
        let new_uart = vm.bus.uart.tx_buf.len() + vm.bus.sbi.console_output.len();
        if new_uart != uart_bytes {
            eprintln!("[{}] UART+SBI output now {} bytes", count, new_uart);
            uart_bytes = new_uart;
        }

        match result {
            StepResult::Ebreak => {
                eprintln!("[{}] EBREAK at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if count < 200 {
                    eprintln!(
                        "[{}] {:?} at PC=0x{:08X} priv={:?}",
                        count, result, vm.cpu.pc, vm.cpu.privilege
                    );
                }
            }
            StepResult::Ecall => {
                sbi_count += 1;
                if sbi_count <= 20 {
                    let a7 = vm.cpu.x[17];
                    let a0 = vm.cpu.x[10];
                    eprintln!("[{}] SBI ECALL: a7=0x{:X} a0=0x{:08X}", count, a7, a0);
                }
            }
            _ => {}
        }

        count += 1;

        // Print progress
        if count % 1_000_000 == 0 {
            eprintln!(
                "[{}] PC=0x{:08X} priv={:?} UART={}",
                count, vm.cpu.pc, vm.cpu.privilege, uart_bytes
            );
        }
    }

    eprintln!(
        "\nFinal: {} steps, {} SBI calls, {} UART bytes",
        count, sbi_count, uart_bytes
    );
    let uart_str: String = vm
        .bus
        .uart
        .tx_buf
        .iter()
        .chain(vm.bus.sbi.console_output.iter())
        .map(|&b| b as char)
        .collect();
    eprintln!("Output: {:?}", &uart_str[..uart_str.len().min(500)]);
}
