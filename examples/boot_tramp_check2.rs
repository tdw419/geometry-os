use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        177_400, // Stop just after first SATP change
        "loglevel=0 quiet",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    eprintln!("After boot_linux (200K instructions):");
    eprintln!("  PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("  SATP=0x{:08X}", satp);
    
    // Check trampoline_pg_dir before we run more
    let tramp_l1_ff = vm.bus.read_word(0x01484000 + 0xFF * 4).unwrap_or(0xDEAD);
    eprintln!("  trampoline_pg_dir L1[0xFF] = 0x{:08X}", tramp_l1_ff);
    
    // Count non-zero entries in trampoline_pg_dir
    let mut count = 0;
    for i in 0..1024u32 {
        let pte = vm.bus.read_word(0x01484000 + i as u64 * 4).unwrap_or(0);
        if pte != 0 { 
            count += 1;
            eprintln!("  trampoline_pg_dir L1[0x{:03X}] = 0x{:08X}", i, pte);
        }
    }
    eprintln!("  trampoline_pg_dir: {} non-zero entries", count);

    // Check if the kernel wrote to the PHYSICAL page of trampoline_pg_dir
    // The kernel should use __pa(0xC1484000) = 0x01484000
    // But maybe it's writing to a WRONG address due to our __pa() patch
    // Let's check what's at PA 0x01484000
    eprintln!("
  PA 0x01484000 content:");
    for i in 0..4 {
        let w = vm.bus.read_word(0x01484000 + i as u64 * 4).unwrap_or(0xDEAD);
        eprintln!("    +{}: 0x{:08X}", i*4, w);
    }

    // The kernel's relocate function copies relocate_enable_mmu to trampoline page
    // and creates the page table entries manually.
    // Let's check if there's any code at VA 0x3FFFF000 in early_pg_dir
    // VA 0x3FFFF000: VPN1 = 0xFF
    // Check if early_pg_dir has L1[0xFF] = something
    let early_l1_ff = vm.bus.read_word(0x00802000 + 0xFF * 4).unwrap_or(0xDEAD);
    eprintln!("
  early_pg_dir L1[0xFF] = 0x{:08X}", early_l1_ff);

    // Check kernel_map struct values
    let km_phys: u64 = 0x00C79E90;
    eprintln!("
  kernel_map struct at PA 0x{:08X}:", km_phys);
    for (name, off) in [("page_offset", 0), ("virt_addr", 4), ("virt_offset", 8),
                        ("phys_addr", 12), ("size", 16), ("va_pa_offset", 20),
                        ("va_kernel_pa_offset", 24)] {
        let val = vm.bus.read_word(km_phys + off).unwrap_or(0xDEAD);
        eprintln!("    {} (off {}): 0x{:08X}", name, off, val);
    }

    // Now check what's happening - let's trace a few steps after first SATP swap
    // The kernel wrote SATP=0x80001484 then immediately back to 0x80000802
    // This means something in the trampoline code failed
    // Let's look at what the kernel's relocate code is doing

    // Check the relocate code at PA 0x1000 more carefully
    // The first instruction at _start is the PE/COFF header skip
    // The actual start is at PA 0x1000 (relocate_enable_mmu)
    // But _start at PA 0 has the initial boot code
    
    eprintln!("
  _start code at PA 0x0:");
    for i in 0..32 {
        let w = vm.bus.read_word(i as u64 * 4).unwrap_or(0xDEAD);
        eprintln!("    PA 0x{:08X}: 0x{:08X}", i*4, w);
    }
}

