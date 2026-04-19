// Watch s8 register before and after memblock_alloc_or_panic call
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
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    // Watch for the jal to memblock_alloc_or_panic at 0xC040AFBE
    // and the return at 0xC040AFC2 (the instruction after)
    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        // Before jal to memblock_alloc_or_panic (chunk struct alloc)
        if pc == 0xC040AFBE {
            let s8 = vm.cpu.x[24];
            let s4 = vm.cpu.x[20];
            eprintln!(
                "[{}] Before jal memblock_alloc_or_panic (chunk alloc):",
                count
            );
            eprintln!("  s8=0x{:08X} s4=0x{:08X}", s8, s4);
        }

        // After return from memblock_alloc_or_panic (chunk alloc)
        if pc == 0xC040AFC2 {
            let s8 = vm.cpu.x[24];
            let s4 = vm.cpu.x[20];
            let a0 = vm.cpu.x[10];
            eprintln!("[{}] After return (chunk alloc):", count);
            eprintln!("  s8=0x{:08X} s4=0x{:08X} a0=0x{:08X}", s8, s4, a0);
        }

        // Before jal to memblock_alloc_or_panic (alloc_map alloc)
        if pc == 0xC040AFF4 {
            let s8 = vm.cpu.x[24];
            eprintln!(
                "[{}] Before jal memblock_alloc_or_panic (alloc_map): s8=0x{:08X}",
                count, s8
            );
        }
        if pc == 0xC040AFF8 {
            let s8 = vm.cpu.x[24];
            eprintln!("[{}] After return (alloc_map): s8=0x{:08X}", count, s8);
        }

        // Before pcpu_init_md_blocks
        if pc == 0xC040B034 {
            let s8 = vm.cpu.x[24];
            eprintln!("[{}] Before pcpu_init_md_blocks: s8=0x{:08X}", count, s8);
        }
        if pc == 0xC040B038 {
            let s8 = vm.cpu.x[24];
            eprintln!("[{}] After pcpu_init_md_blocks: s8=0x{:08X}", count, s8);
        }

        if vm.cpu.csr.scause != 0 {
            break;
        }
    }
}
