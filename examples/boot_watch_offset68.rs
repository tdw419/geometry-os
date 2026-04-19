// Watch start_offset field get corrupted
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    let mut count: u64 = 0;
    while count < 15_560_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        count += 1;
    }

    // Now watch specific PCs and check chunk[68]
    let mut last_so: u32 = 0;
    let mut checked = false;
    
    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        // After chunk is allocated at 0xC040AFC6, start checking
        if pc == 0xC040AFC6 && !checked {
            let chunk = vm.cpu.x[10];
            eprintln!("[{}] Chunk allocated at 0x{:08X}", count, chunk);
            checked = true;
        }
        
        if checked {
            // Check chunk[68] at key points
            let chunk = 0xCFDBFCC0u32; // we know the address from earlier
            let pa = (chunk - 0xC0000000) as u64;
            let so = vm.bus.read_word(pa + 68).unwrap_or(0);
            
            if so != last_so {
                eprintln!("[{}] chunk[68] changed: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})",
                    count, last_so, so, pc);
                last_so = so;
            }
        }
        
        // Stop after the crash
        if vm.cpu.csr.scause != 0 {
            break;
        }
    }
}
