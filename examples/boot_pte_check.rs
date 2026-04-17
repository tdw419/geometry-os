use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Boot with 200K instructions to get past setup_vm
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    println!("\n=== Post-setup_vm state ===");
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP=0x{:08X}", vm.cpu.csr.satp);
    println!("SP=0x{:08X}", vm.cpu.x[2]);

    // Check kernel_map values
    let km_phys: u64 = 0x00C79E90;
    let page_offset = vm.bus.read_word(km_phys).unwrap_or(0);
    let virt_addr = vm.bus.read_word(km_phys + 4).unwrap_or(0);
    let virt_offset = vm.bus.read_word(km_phys + 8).unwrap_or(0);
    let phys_addr = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let size = vm.bus.read_word(km_phys + 16).unwrap_or(0);
    let va_pa_offset = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let va_kernel_pa_offset = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    println!("\nkernel_map struct:");
    println!("  page_offset       = 0x{:08X}", page_offset);
    println!("  virt_addr         = 0x{:08X}", virt_addr);
    println!("  virt_offset       = 0x{:08X}", virt_offset);
    println!("  phys_addr         = 0x{:08X}", phys_addr);
    println!("  size              = 0x{:08X}", size);
    println!("  va_pa_offset      = 0x{:08X}", va_pa_offset);
    println!("  va_kernel_pa_offset = 0x{:08X}", va_kernel_pa_offset);

    // Check page table entries for the linear mapping
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("\nPage table root at PA 0x{:08X}", pg_dir_phys);

    // Check L1[768] through L1[775] (kernel linear mapping)
    for i in 768..776 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        let v = (pte >> 0) & 1;
        let r = (pte >> 1) & 1;
        let w = (pte >> 2) & 1;
        let x = (pte >> 3) & 1;
        let u = (pte >> 4) & 1;
        let g = (pte >> 5) & 1;
        let a = (pte >> 6) & 1;
        let d = (pte >> 7) & 1;
        let ppn = pte >> 10;
        println!("  L1[{}] = 0x{:08X} V={} R={} W={} X={} U={} G={} A={} D={} PPN=0x{:X}", 
            i, pte, v, r, w, x, u, g, a, d, ppn);
    }

    // Check specific VA translation: 0xC003F9CC
    let test_va = 0xC003F9CC;
    let vpn2 = (test_va >> 22) & 0x3FF;
    let vpn1 = (test_va >> 12) & 0x3FF;
    let offset = test_va & 0xFFF;
    println!("\nTranslation of VA 0x{:08X}:", test_va);
    println!("  VPN2={} VPN1={} offset=0x{:03X}", vpn2, vpn1, offset);
    
    let l1_pte = vm.bus.read_word(pg_dir_phys + (vpn2 as u64) * 4).unwrap_or(0);
    let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
    let l1_v = l1_pte & 1;
    let l1_rwx = (l1_pte >> 1) & 0x7;
    println!("  L1[{}] = 0x{:08X} V={} RWX={} PPN=0x{:X}", vpn2, l1_pte, l1_v, l1_rwx, l1_ppn);
    
    if l1_rwx != 0 && l1_v != 0 {
        // Megapage
        let pa = (l1_ppn << 22) | (offset as u32);
        println!("  -> MEGAPAGE: PA=0x{:08X}", pa);
    } else if l1_v != 0 {
        // L2 page table
        let l2_addr = (l1_ppn as u64) * 4096 + (vpn1 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
        let l2_v = l2_pte & 1;
        println!("  L2[{}] at PA 0x{:08X} = 0x{:08X} V={} PPN=0x{:X}", 
            vpn1, l2_addr, l2_pte, l2_v, l2_ppn);
        let pa = (l2_ppn << 12) | offset;
        println!("  -> 4KB PAGE: PA=0x{:08X}", pa);
        
        // Read the actual instruction at that PA
        let inst = vm.bus.read_word(pa as u64).unwrap_or(0);
        println!("  Instruction at PA 0x{:08X}: 0x{:08X}", pa, inst);
    }

    // Also check SP area
    let sp = vm.cpu.x[2];
    let sp_pa = if sp >= 0xC0000000 { sp - 0xC0000000 } else { sp };
    println!("\nStack area (SP=0x{:08X}, PA=0x{:08X}):", sp, sp_pa);
    for off in [88, 92, 96, 100, 104] {
        let addr = sp_pa as u64 + off as u64;
        let val = vm.bus.read_word(addr).unwrap_or(0);
        let reg = match off {
            88 => "s0",
            92 => "ra",
            96 => "??",
            100 => "??",
            104 => "??",
            _ => "??",
        };
        println!("  [SP+{}] = 0x{:08X} ({})", off, val, reg);
    }

    // SBI output
    let sbi_str: String = vm.bus.sbi.console_output.iter().map(|&b| b as char).collect();
    if !sbi_str.is_empty() {
        println!("\nSBI console output ({} bytes):", sbi_str.len());
        println!("{}", sbi_str);
    }
}
