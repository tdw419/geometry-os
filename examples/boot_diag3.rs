use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 200_000_000, bootargs,
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let pgdir = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("satp=0x{:08X} pgdir PA=0x{:08X}", satp, pgdir);

    // Dump ALL non-zero L1 entries
    println!("
=== All non-zero L1 entries ===");
    for idx in 0..1024u32 {
        let addr = pgdir + (idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte != 0 {
            let is_leaf = (pte & 0xE) != 0; // R|W|X
            let pa_bits = (pte >> 10) & 0x3FFFFF;
            let flags = pte & 0x3FF;
            let va_start: u32 = idx << 22;
            let pa_start: u64 = (pa_bits as u64) * 4096;
            println!("L1[{}] PTE=0x{:08X} VA=0x{:08X} -> PA=0x{:08X} flags=0x{:03X} {}",
                idx, pte, va_start, pa_start, flags,
                if is_leaf { "MEGAPAGE" } else { "PT_PTR" });
        }
    }
    
    println!("
PC=0x{:08X}", vm.cpu.pc);
    println!("sepc=0x{:08X}", vm.cpu.csr.sepc);
    println!("stval=0x{:08X}", vm.cpu.csr.stval);
    println!("scause=0x{:08X}", vm.cpu.csr.scause);
    
    // The stval=0x804046B4 is at VPN[1]=0x201=513
    // This is the kernel's linear mapping at VA 0x80400000
    // The kernel setup_arch() uses __va() to convert PA addresses
    // But with va_pa_offset=0xC0000000, __va(pa) = pa + 0xC0000000
    // So PA 0x004046B4 should be accessed as VA 0xC04046B4, not 0x804046B4!
    //
    // 0x804046B4 = PA 0x804046B4 + ??? 
    // If we subtract va_pa_offset: 0x804046B4 - 0xC0000000 wraps to 0x40406B4
    // That's nonsensical.
    //
    // UNLESS the kernel is computing __va() differently:
    // 0x804046B4 could be from PAGE_OFFSET=0x80000000 (rv32 standard)
    // vs our patched PAGE_OFFSET=0xC0000000
    //
    // Check: does the kernel have PAGE_OFFSET baked in at 0x80000000?
    // That would explain everything -- the kernel code uses VA 0x80000000+ 
    // but we told it va_pa_offset=0xC0000000
    
    // What's at the faulting instruction sepc=0xC006ADD8?
    // That's PA 0x0006ADD8 which we dumped above:
    // C006ADD8: 00E91023  which is: SH a4, 0(a3)
    // So it's storing a4 to the address in a3. And stval=0x804046B4
    // So a3=0x804046B4 at this point.
    //
    // 0x804046B4 - where does it come from?
    // Most likely: a global variable at VA 0x80404XXX 
    // This is PAGE_OFFSET=0x80000000 style addressing!
    // The kernel is using __va(0x00404XXX) = 0x80000000 + 0x00404XXX = 0x80404XXX
    // BUT the page tables map VA 0xC0000000+ to PA 0x00000000+
    // So 0x80404XXX falls into an unmapped region!
    
    println!("
=== ROOT CAUSE ANALYSIS ===");
    println!("The kernel accesses VA 0x804046B4 (stval).");
    println!("L1[513] = 0 (not mapped). VA range 0x80400000-0x807FFFFF is unmapped.");
    println!("The kernel likely uses PAGE_OFFSET=0x80000000 for some symbols,");
    println!("but page tables only map VA 0xC0000000+ (our patched va_pa_offset).");
    println!();
    println!("This means the kernel_map patching is INCOMPLETE:");
    println!("We patched va_pa_offset to 0xC0000000 in the kernel_map struct,");
    println!("but the kernel has PAGE_OFFSET baked into its compiled code,");
    println!("and some addresses use the compile-time constant 0x80000000.");
}
