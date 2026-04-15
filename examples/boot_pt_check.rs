/// Check page table mappings for key addresses
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();
    
    // Run for a short time to get past setup
    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        256,
        1_000_000,
        "console=ttyS0 earlycon",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let ppn = (satp >> 0) & 0x3FFFFF; // bits 21:0
    let root_pt_addr = (ppn as u64) << 12;
    
    eprintln!("SATP: 0x{:08X}, PPN: 0x{:06X}, root PT: 0x{:08X}", satp, ppn, root_pt_addr);
    
    // Check addresses around the fault
    let addrs = [
        0xC08E5D6Au32, // __memmove (was working)
        0xC08EFF1Cu32, // handle_exception (fetch fault)
        0xC08BDFFCu32, // load fault stval
        0xC08E5000u32, // __memmove page
        0xC08EF000u32, // handle_exception page
        0xC08BD000u32, // load fault page
    ];
    
    for &va in &addrs {
        let page = va & !0xFFF;
        let vpn2 = (va >> 22) & 0x3FF;
        let vpn1 = (va >> 12) & 0x3FF;
        let offset = va & 0xFFF;
        
        // Read superpage PTE (level 2)
        let l2_pte_addr = root_pt_addr + (vpn2 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_pte_addr).unwrap_or(0);
        let l2_valid = l2_pte & 1;
        let l2_leaf = (l2_pte >> 4) & 1;
        let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
        
        eprintln!("\nVA 0x{:08X} (page 0x{:08X}, vpn2={}, vpn1={}, off=0x{:03X}):", va, page, vpn2, vpn1, offset);
        eprintln!("  L2 PTE at PA 0x{:08X}: 0x{:08X} (valid={}, leaf={}, ppn=0x{:06X})",
            l2_pte_addr, l2_pte, l2_valid, l2_leaf, l2_ppn);
        
        if l2_valid != 0 && l2_leaf == 0 {
            // Non-leaf: read level 1 PTE
            let l1_pt_addr = (l2_ppn as u64) << 12;
            let l1_pte_addr = l1_pt_addr + (vpn1 as u64) * 4;
            let l1_pte = vm.bus.read_word(l1_pte_addr).unwrap_or(0);
            let l1_valid = l1_pte & 1;
            let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
            let l1_pa = (l1_ppn as u64) << 12;
            eprintln!("  L1 PTE at PA 0x{:08X}: 0x{:08X} (valid={}, ppn=0x{:06X}, PA=0x{:08X})",
                l1_pte_addr, l1_pte, l1_valid, l1_ppn, l1_pa);
        } else if l2_valid != 0 && l2_leaf != 0 {
            // Leaf (superpage)
            let pa = (l2_ppn as u64) << 12 | (va & 0x3FFFFF) as u64;
            eprintln!("  SUPERPAGE -> PA 0x{:08X}", pa);
        } else {
            eprintln!("  NOT MAPPED (L2 PTE invalid)");
        }
    }
}
