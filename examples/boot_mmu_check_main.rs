use geometry_os::riscv::RiscvVm;

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
    )
    .unwrap();

    // Run to 5M
    for _ in 0..5_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
    }

    // Check what's at the page table
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pgdir_pa = ppn as u64 * 4096;
    eprintln!(
        "SATP=0x{:08X} ppn=0x{:X} pgdir_pa=0x{:X}",
        satp, ppn, pgdir_pa
    );

    // Dump L1 entries for kernel virtual region
    for l1_idx in [768, 769, 770, 771, 772, 800, 801, 900, 901, 1020, 1021] {
        let addr = pgdir_pa + (l1_idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        let v = (pte >> 0) & 1;
        let r = (pte >> 1) & 1;
        let w = (pte >> 2) & 1;
        let x = (pte >> 3) & 1;
        let ppn_pte = (pte >> 10) & 0x3FFFFF;
        let pa_of_page = ppn_pte as u64 * 4096;
        let va_start = l1_idx as u64 * (2 << 20); // 2MB megapage
        eprintln!(
            "L1[{}] VA 0x{:08X}: PTE=0x{:08X} V={}R={}W={}X={} PPN=0x{:X}->PA 0x{:X}",
            l1_idx, va_start as u32, pte, v, r, w, x, ppn_pte, pa_of_page
        );
        if v == 1 && r == 0 {
            // Pointer to L2 page table
            eprintln!("  -> Points to L2 at PA 0x{:X}", pa_of_page);
        }
    }

    // Check if the kernel's physical pages still have data
    // The kernel starts at PA 0x0, entry is VA 0xC0000000
    eprintln!("\nPhysical memory at key locations:");
    for pa in [
        0x0u64, 0x1000, 0x10000, 0x100000, 0x200000, 0x400000, 0x800000,
    ] {
        let w = vm.bus.read_word(pa).unwrap_or(0xDEAD);
        eprintln!("PA 0x{:08X}: 0x{:08X}", pa, w);
    }
}
