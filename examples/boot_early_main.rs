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

    // Step manually and print key events
    let mut count = 0u64;
    let mut last_satp = vm.cpu.csr.satp;
    let mut last_priv = format!("{:?}", vm.cpu.privilege);
    let mut satp_change_count = 0;
    let mut s_mode_entry_count = 0;
    
    while count < 2_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        
        let pc_before = vm.cpu.pc;
        let priv_before = vm.cpu.privilege;
        let result = vm.step();
        
        if vm.cpu.csr.satp != last_satp {
            let mode = (vm.cpu.csr.satp >> 31) & 1;
            let ppn = vm.cpu.csr.satp & 0x3FFFFF;
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X} (mode={} ppn=0x{:X} pgdir_pa=0x{:X}) at PC=0x{:08X} priv={:?}",
                count, last_satp, vm.cpu.csr.satp, mode, ppn, ppn as u64 * 4096, pc_before, priv_before);
            last_satp = vm.cpu.csr.satp;
            satp_change_count += 1;
            if satp_change_count > 8 { break; }
        }
        
        // Detect privilege changes
        let priv_now = format!("{:?}", vm.cpu.privilege);
        if priv_now != last_priv {
            eprintln!("[{}] Privilege: {} -> {} at PC=0x{:08X}", count, last_priv, priv_now, vm.cpu.pc);
            last_priv = priv_now;
        }
        
        // Detect S-mode execution starting
        if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            if s_mode_entry_count == 0 {
                eprintln!("[{}] First S-mode PC=0x{:08X}", count, vm.cpu.pc);
            }
            s_mode_entry_count += 1;
        }
        
        count += 1;
    }
    
    eprintln!("\nTotal: {} steps, {} SATP changes, {} S-mode steps", count, satp_change_count, s_mode_entry_count);
    eprintln!("Final: PC=0x{:08X} priv={:?} SATP=0x{:08X}", vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    eprintln!("UART: {} bytes, SBI: {} bytes", vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());
}
