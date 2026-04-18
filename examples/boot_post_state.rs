use geometry_os::riscv::cpu::StepResult;
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
        200_000,
        "loglevel=0 quiet",
    )
    .unwrap();

    eprintln!("Post-boot state:");
    eprintln!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("  SATP=0x{:08X}", vm.cpu.csr.satp);
    eprintln!(
        "  scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
    );
    eprintln!(
        "  x2(sp)=0x{:08X} x3(gp)=0x{:08X} x4(tp)=0x{:08X}",
        vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
    );
    eprintln!("  ra=0x{:08X}", vm.cpu.x[1]);

    // The boot loop already handled everything. The faults at 0x3FFFF000
    // happen WITHIN the boot_linux loop. Let me modify to catch them.
    //
    // Actually, looking at the boot output:
    // - S-mode fetch faults at 0x3FFFF000 happen during the boot loop
    // - After 200K steps, PC is 0x10B8 (physical)
    //
    // This means: during the boot loop, the kernel's exception handler
    // at stvec=0xC0210F14 runs, and it somehow ends up at PC 0x10B8.
    // 0x10B8 is in the low identity-mapped area (PA).
    //
    // 0x10B8 is within the first 2MB, which IS mapped by early_pg_dir L1[0].
    // But in S-mode with MMU on, PC=0x10B8 means VA 0x10B8 which maps to
    // PA 0x10B8 via L1[0] identity mapping.
    //
    // What's at PA 0x10B8?
    let w = vm.bus.read_word(0x10B8).unwrap_or(0xDEAD);
    eprintln!(
        "
  PA 0x10B8: 0x{:08X}",
        w
    );

    // Check a range of instructions around 0x10B8
    eprintln!("  Code around PA 0x10B8:");
    for off in (-16i32..32).step_by(4) {
        let addr = (0x10B8i64 + off as i64) as u64;
        let word = vm.bus.read_word(addr).unwrap_or(0xDEAD);
        eprintln!("    PA 0x{:08X}: 0x{:08X}", addr, word);
    }

    // The kernel is stuck. Let me check what the handle_exception code does
    // stvec=0xC0210F14 -> PA 0x210F14
    eprintln!(
        "
  handle_exception at PA 0x210F14:"
    );
    for off in (0..48).step_by(4) {
        let word = vm.bus.read_word(0x210F14 + off).unwrap_or(0xDEAD);
        eprintln!("    PA 0x{:08X}: 0x{:08X}", 0x210F14 + off, word);
    }
}
