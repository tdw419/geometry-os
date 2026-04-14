// Dump the page table entries for VA 0xC0001048
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::mmu::{translate, AccessType, satp_ppn, va_vpn1, va_vpn0, PTE_V, PTE_R, PTE_W, PTE_X};

const PTE_A: u32 = 1 << 6;
const PTE_D: u32 = 1 << 7;

fn pte_ppn(pte: u32) -> u32 {
    (pte & 0xFFFF_FC00) >> 10
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 512, 0, bootargs,
    ).unwrap();

    // Run until satp changes
    let max_instr = 300_000u64;
    let mut count = 0u64;
    let mut prev_satap = vm.cpu.csr.satp;

    while count < max_instr {
        vm.step();
        count += 1;
        if vm.cpu.csr.satp != prev_satap {
            prev_satap = vm.cpu.csr.satp;
            break;
        }
    }

    let satp = vm.cpu.csr.satp;
    let root_ppn = satp_ppn(satp);
    let root_addr = (root_ppn as u64) << 12;
    
    println!("satp = 0x{:08X}, root PPN = 0x{:06X}, root PA = 0x{:08X}", satp, root_ppn, root_addr);
    
    // Check VA 0xC0001048
    let va = 0xC0001048u32;
    let vpn1 = va_vpn1(va);
    let vpn0 = va_vpn0(va);
    println!("\nVA = 0x{:08X}, VPN1 = {} (0x{:03X}), VPN0 = {} (0x{:03X})", va, vpn1, vpn1, vpn0, vpn0);
    
    // Level 1 PTE
    let l1_addr = root_addr + (vpn1 as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("\nL1 PTE[{}] at PA 0x{:08X} = 0x{:08X}", vpn1, l1_addr, l1_pte);
    println!("  V={} R={} W={} X={} G={} A={} D={}", 
             (l1_pte >> 0) & 1, (l1_pte >> 1) & 1, (l1_pte >> 2) & 1,
             (l1_pte >> 3) & 1, (l1_pte >> 5) & 1, (l1_pte >> 6) & 1, (l1_pte >> 7) & 1);
    
    let l1_ppn = pte_ppn(l1_pte);
    println!("  PPN = 0x{:06X}", l1_ppn);
    
    let is_leaf = (l1_pte & (PTE_R | PTE_W | PTE_X)) != 0;
    println!("  Is leaf: {}", is_leaf);
    
    if !is_leaf {
        // Level 2
        let l2_base = (l1_ppn as u64) << 12;
        let l2_addr = l2_base + (vpn0 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!("\nL2 PTE[{}] at PA 0x{:08X} = 0x{:08X}", vpn0, l2_addr, l2_pte);
        println!("  V={} R={} W={} X={} G={} A={} D={}", 
                 (l2_pte >> 0) & 1, (l2_pte >> 1) & 1, (l2_pte >> 2) & 1,
                 (l2_pte >> 3) & 1, (l2_pte >> 5) & 1, (l2_pte >> 6) & 1, (l2_pte >> 7) & 1);
        
        let l2_ppn = pte_ppn(l2_pte);
        println!("  PPN = 0x{:06X}", l2_ppn);
        
        let offset = va & 0xFFF;
        let pa = ((l2_ppn as u64) << 12) | (offset as u64);
        println!("\n  => PA = (PPN << 12) | offset = 0x{:08X}", pa);
        println!("  Expected PA = 0x{:08X} (identity)", va);
    }
    
    // Also check: what PTE value would produce correct identity mapping?
    println!("\n--- For correct identity mapping of 0xC0001000 ---");
    println!("  L2 PTE should have PPN = 0x{:06X}", (va >> 12) & 0xFFFFF);
    
    // Check a few other page table entries in the L2 table
    println!("\n--- L2 table entries (L1 PPN = 0x{:06X}) ---", l1_ppn);
    let l2_base = (l1_ppn as u64) << 12;
    for i in 0..16 {
        let l2_pte = vm.bus.read_word(l2_base + (i as u64) * 4).unwrap_or(0);
        if l2_pte != 0 {
            let ppn = pte_ppn(l2_pte);
            let va_base = (vpn1 << 22) | (i << 12);
            let pa_base = (ppn as u64) << 12;
            println!("  L2[{}] VA=0x{:08X} PTE=0x{:08X} PPN=0x{:06X} PA=0x{:08X} {}",
                     i, va_base, l2_pte, ppn, pa_base,
                     if va_base as u64 == pa_base { "IDENTITY" } else { "MISMATCH" });
        }
    }
    
    // Check what's at trampoline_pg_dir 
    println!("\n--- trampoline_pg_dir at 0xC1CCF000 (first 16 entries) ---");
    for i in 0..16 {
        let pte = vm.bus.read_word(0xC1CCF000_u64 + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let ppn = pte_ppn(pte);
            let is_leaf = (pte & (PTE_R | PTE_W | PTE_X)) != 0;
            println!("  PTE[{}] = 0x{:08X} PPN=0x{:06X} leaf={}", i, pte, ppn, is_leaf);
        }
    }
    
    // The key question: does the kernel's relocate_enable_mmu expect the
    // page tables to be set up by setup_vm()? The kernel runs setup_vm() first,
    // then relocate_enable_mmu. The trampoline_pg_dir is a temporary page table
    // used DURING the relocation.
    // Let's check the early_pg_dir too
    println!("\n--- early_pg_dir at 0xC1002000 (first 16 entries) ---");
    for i in 0..16 {
        let pte = vm.bus.read_word(0xC1002000_u64 + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let ppn = pte_ppn(pte);
            let is_leaf = (pte & (PTE_R | PTE_W | PTE_X)) != 0;
            println!("  PTE[{}] = 0x{:08X} PPN=0x{:06X} leaf={}", i, pte, ppn, is_leaf);
        }
    }
}
