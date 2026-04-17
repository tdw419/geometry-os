use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        177_400,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    // Dump trampoline_pg_dir (PA 0x01484000)
    println!("trampoline_pg_dir (PA 0x01484000):");
    for i in 0..1024 {
        let pte = vm.bus.read_word(0x01484000 + i as u64 * 4).unwrap_or(0);
        if pte != 0 {
            let v = pte & 1;
            let rwx = (pte >> 1) & 7;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let pa = (ppn as u64) << 12;
            let va_start = (i as u32) * 2 * 1024 * 1024;
            println!("  L1[{}] = 0x{:08X} V={} RWX={:03b} PPN=0x{:06X} PA=0x{:08X} VA=0x{:08X}",
                i, pte, v, rwx, ppn, pa, va_start);
        }
    }

    println!("\nswapper_pg_dir (PA 0x01485000):");
    for i in 0..1024 {
        let pte = vm.bus.read_word(0x01485000 + i as u64 * 4).unwrap_or(0);
        if pte != 0 {
            let v = pte & 1;
            let rwx = (pte >> 1) & 7;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let pa = (ppn as u64) << 12;
            let va_start = (i as u32) * 2 * 1024 * 1024;
            println!("  L1[{}] = 0x{:08X} V={} RWX={:03b} PPN=0x{:06X} PA=0x{:08X} VA=0x{:08X}",
                i, pte, v, rwx, ppn, pa, va_start);
        }
    }

    println!("\nearly_pg_dir (PA 0x00802000):");
    for i in 0..1024 {
        let pte = vm.bus.read_word(0x00802000 + i as u64 * 4).unwrap_or(0);
        if pte != 0 {
            let v = pte & 1;
            let rwx = (pte >> 1) & 7;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let pa = (ppn as u64) << 12;
            let va_start = (i as u32) * 2 * 1024 * 1024;
            println!("  L1[{}] = 0x{:08X} V={} RWX={:03b} PPN=0x{:06X} PA=0x{:08X} VA=0x{:08X}",
                i, pte, v, rwx, ppn, pa, va_start);
        }
    }

    println!("\nCurrent SATP = 0x{:08X}", vm.cpu.csr.satp);
    println!("Current PC = 0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X}", 
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
}
