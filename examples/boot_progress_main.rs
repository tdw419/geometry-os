use geometry_os::riscv::{RiscvVm, cpu::StepResult};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();
    
    let mut count = 0u64;
    let mut last_mie = vm.cpu.csr.mie;
    let mut last_stvec = vm.cpu.csr.stvec;
    let mut last_sie = 0u32;
    
    while count < 50_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        let result = vm.step();
        
        // Track changes in interrupt-related state
        if vm.cpu.csr.mie != last_mie {
            eprintln!("[{}] MIE changed: 0x{:08X} -> 0x{:08X}", count, last_mie, vm.cpu.csr.mie);
            last_mie = vm.cpu.csr.mie;
        }
        if vm.cpu.csr.stvec != last_stvec {
            eprintln!("[{}] stvec changed: 0x{:08X} -> 0x{:08X}", count, last_stvec, vm.cpu.csr.stvec);
            last_stvec = vm.cpu.csr.stvec;
        }
        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
        if sie != last_sie {
            eprintln!("[{}] SIE changed: {} -> {}", count, last_sie, sie);
            last_sie = sie;
        }
        
        match result {
            StepResult::Ebreak => { eprintln!("[{}] EBREAK", count); break; }
            StepResult::Ecall => {
                eprintln!("[{}] ECALL a7=0x{:X} PC=0x{:08X}", count, vm.cpu.x[17], vm.cpu.pc);
            }
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if count < 500 {
                    eprintln!("[{}] {:?} PC=0x{:08X}", count, result, vm.cpu.pc);
                }
            }
            _ => {}
        }
        
        count += 1;
        if count % 5_000_000 == 0 {
            let mie = vm.cpu.csr.mie;
            let mip = vm.cpu.csr.mip;
            let uart_total = vm.bus.uart.tx_buf.len() + vm.bus.sbi.console_output.len();
            eprintln!("[{}] PC=0x{:08X} MIE=0x{:08X} MIP=0x{:08X} stvec=0x{:08X} sie={} UART={}", 
                count, vm.cpu.pc, mie, mip, vm.cpu.csr.stvec, sie, uart_total);
        }
    }
    
    let uart_total = vm.bus.uart.tx_buf.len() + vm.bus.sbi.console_output.len();
    eprintln!("\nFinal: {} steps, {} UART bytes", count, uart_total);
    if uart_total > 0 {
        let uart_str: String = vm.bus.uart.tx_buf.iter().chain(vm.bus.sbi.console_output.iter()).map(|&b| b as char).collect();
        eprintln!("Output: {:?}", &uart_str[..uart_str.len().min(500)]);
    }
}
