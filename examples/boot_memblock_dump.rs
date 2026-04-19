// Dump all memblock reserved regions to find percpu allocations
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

    // Run until just after setup_vm_final (SATP change at ~9.7M instructions)
    // then dump memblock state to see all allocations
    let mut count: u64 = 0;
    let target = 10_000_000; // Run past setup_vm_final
    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;

    while count < target {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;

        // Break early on panic
        if vm.cpu.pc >= panic_start && vm.cpu.pc < panic_end {
            break;
        }
    }

    eprintln!("Stopped at count={}, PC=0x{:08X}", count, vm.cpu.pc);

    // Dump ALL memblock reserved regions (12-byte stride)
    let memblock_pa: u64 = 0x00803448;
    let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
    let res_cnt = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
    eprintln!("memblock: memory.cnt={}, reserved.cnt={}", mem_cnt, res_cnt);

    // Memory regions
    let mem_regions_ptr = vm.bus.read_word(memblock_pa + 20).unwrap_or(0);
    if mem_regions_ptr >= 0xC0000000 {
        let base_pa = (mem_regions_ptr - 0xC0000000) as u64;
        for ri in 0..mem_cnt.min(8) {
            let b = vm.bus.read_word(base_pa + (ri as u64) * 12).unwrap_or(0);
            let s = vm
                .bus
                .read_word(base_pa + (ri as u64) * 12 + 4)
                .unwrap_or(0);
            let f = vm
                .bus
                .read_word(base_pa + (ri as u64) * 12 + 8)
                .unwrap_or(0);
            eprintln!(
                "  memory[{}]: base=0x{:08X} size=0x{:08X} flags=0x{:08X} (end=0x{:08X})",
                ri,
                b,
                s,
                f,
                b.wrapping_add(s)
            );
        }
    }

    // Reserved regions
    let res_regions_ptr = vm.bus.read_word(memblock_pa + 40).unwrap_or(0);
    if res_regions_ptr >= 0xC0000000 {
        let base_pa = (res_regions_ptr - 0xC0000000) as u64;
        for ri in 0..res_cnt.min(30) {
            let b = vm.bus.read_word(base_pa + (ri as u64) * 12).unwrap_or(0);
            let s = vm
                .bus
                .read_word(base_pa + (ri as u64) * 12 + 4)
                .unwrap_or(0);
            let f = vm
                .bus
                .read_word(base_pa + (ri as u64) * 12 + 8)
                .unwrap_or(0);
            eprintln!(
                "  reserved[{}]: base=0x{:08X} size=0x{:08X} flags=0x{:08X} (end=0x{:08X})",
                ri,
                b,
                s,
                f,
                b.wrapping_add(s)
            );
        }
    }

    // Now check: the linear mapping covers VA 0xC0000000-0xD0000000 (PA 0-0x10000000)
    // Any memblock allocation should return a VA in this range.
    // The crash VA 0xD77BA368 is at PA 0x177BA368 -- beyond 256MB.
    // This should be impossible if memblock only knows about 256MB.

    // Let's also check the page table coverage more carefully
    let satp = vm.cpu.csr.satp;
    let ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_pa = ppn * 4096;

    eprintln!("\nPage table: pg_dir at PA 0x{:08X}", pg_dir_pa);

    // Count valid L1 entries in kernel range
    let mut valid_count = 0u32;
    let mut last_valid = 0u32;
    for i in 768..1024 {
        let entry = vm.bus.read_word(pg_dir_pa + (i as u64) * 4).unwrap_or(0);
        if (entry & 1) != 0 {
            valid_count += 1;
            last_valid = i;
        }
    }
    eprintln!(
        "Kernel L1[768..1023]: {} valid entries, last valid L1[{}]",
        valid_count, last_valid
    );
    eprintln!(
        "Linear mapping covers: {}MB ({} entries * 4MB)",
        valid_count * 4,
        valid_count
    );
}
