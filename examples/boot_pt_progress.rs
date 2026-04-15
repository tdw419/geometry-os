fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    use geometry_os::riscv::RiscvVm;

    // Check page tables at various points
    for max_instr in [100000, 200000, 250000, 300000] {
        let (mut vm, _) = RiscvVm::boot_linux(
            &kernel_image,
            initramfs.as_deref(),
            256,
            max_instr,
            bootargs,
        )
        .unwrap();
        
        let satp = vm.cpu.csr.satp;
        let pt_enabled = (satp >> 31) & 1;
        let ppn = satp & 0x003F_FFFF;
        let pt_root = (ppn as u64) << 12;
        
        println!("=== {} instr: PC=0x{:08X} SATP=0x{:08X} PT_root=0x{:08X} ===",
            max_instr, vm.cpu.pc, satp, pt_root);
        
        if pt_root != 0 {
            // Check a few key L1 entries
            for idx in [768, 769, 770, 771, 772, 773] {
                let pte_addr = pt_root + (idx as u64) * 4;
                let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
                let v = pte & 1;
                let rwx = (pte >> 1) & 7;
                if v != 0 {
                    if rwx != 0 {
                        let ppn_hi = (pte >> 20) & 0xFFF;
                        println!("  L1[{}] = 0x{:08X} V=1 MEGAPAGE PA=0x{:06X}00000 RWX={:03b}",
                            idx, pte, ppn_hi, rwx);
                    } else {
                        let l2 = ((pte as u64) >> 10) << 12;
                        println!("  L1[{}] = 0x{:08X} V=1 NON-LEAF L2=0x{:08X}", idx, pte, l2);
                    }
                }
            }
        }
    }
}
