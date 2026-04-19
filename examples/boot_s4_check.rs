// Check s4 value at the sw instruction
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

    // Watch PC = 0xC040AFCE (sw s4, 68(a0))
    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        
        if pc == 0xC040AFCE {
            let s4 = vm.cpu.x[20];
            let a0 = vm.cpu.x[10];
            let s8 = vm.cpu.x[24];
            let s2 = vm.cpu.x[18];
            let s7 = vm.cpu.x[23];
            let s5 = vm.cpu.x[21];
            eprintln!("[{}] At sw s4,68(a0):", count);
            eprintln!("  a0 (chunk) = 0x{:08X}", a0);
            eprintln!("  s4 (start_offset) = 0x{:08X} ({})", s4, s4 as i32);
            eprintln!("  s8 (aligned_addr) = 0x{:08X}", s8);
            eprintln!("  s2 (end_offset?) = 0x{:08X}", s2);
            eprintln!("  s7 (nr_pages?) = 0x{:08X}", s7);
            eprintln!("  s5 (map_size?) = 0x{:08X}", s5);
            
            // Also check entry params
            // a0 (tmp_addr) was saved somewhere...
            // At entry c040af6c, s8 = a0 & PAGE_MASK and s4 = a0 - s8
            // So s4 = a0 - (a0 & 0xFFFFF000)
            // If a0 = 0xCFDB6000 (page-aligned), s4 should be 0
            // But s4 = 0xCFD80000, so either a0 is wrong or the computation is wrong
            
            // s4 was set at c040af7c: sub s4, a0, s8
            // Where s8 = a0 & 0xFFFFF000
            // If a0 = 0xCFD80000 and s8 = 0x0, then s4 = 0xCFD80000
            // But s8 should be a0 & PAGE_MASK...
            // Unless a0 was already 0xCFD80000 at entry
            
            // Let me check: what was a0 at pcpu_alloc_first_chunk entry?
            break;
        }
        
        let _ = vm.step();
        step_count += 1;
        count += 1;
    }
}
