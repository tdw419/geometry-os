use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    let boot_pt_addr = 0x0157A000u64; // Our boot page table
    
    // Check L1 entries right after setup (before running)
    println!("=== Boot Page Table at PA 0x{:08X} ===", boot_pt_addr);
    println!("SATP PPN: 0x{:05X}", vm.cpu.csr.satp & 0x3FFFFF);
    
    // L1[768] should be megapage mapping VA 0xC0000000 -> PA 0x00000000
    let l1_768 = vm.bus.mem.read_word(boot_pt_addr + 768 * 4).unwrap_or(0);
    println!("L1[768] (VA 0xC0000000): 0x{:08X} (expect 0x000000CF)", l1_768);
    if l1_768 & 1 != 0 {
        let ppn = (l1_768 >> 10) & 0x3FFFFF;
        println!("  PPN: 0x{:05X} -> PA 0x{:08X}", ppn, (ppn as u64) << 12);
    }
    
    // L1[0] should be identity mapping
    let l1_0 = vm.bus.mem.read_word(boot_pt_addr + 0 * 4).unwrap_or(0);
    println!("L1[0] (VA 0x00000000): 0x{:08X} (expect 0x000000CF)", l1_0);
    
    // Run to just before setup_vm returns
    let max = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    
    while count < max {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
        
        // Stop right after setup_vm returns (PC should be 0xC00010EE)
        if pc == 0xC00010EA && vm.cpu.pc != 0xC00010EA {
            // setup_vm just returned
            println!("\n=== After setup_vm returned (count={}) ===", count);
            println!("PC: 0x{:08X}", vm.cpu.pc);
            
            // Re-check L1[768]
            let l1_768 = vm.bus.mem.read_word(boot_pt_addr + 768 * 4).unwrap_or(0);
            println!("L1[768] after setup_vm: 0x{:08X} (expect 0x000000CF)", l1_768);
            if l1_768 & 1 != 0 {
                let ppn = (l1_768 >> 10) & 0x3FFFFF;
                println!("  PPN: 0x{:05X} -> PA 0x{:08X}", ppn, (ppn as u64) << 12);
            }
            
            // Check if setup_vm modified our boot page table
            // Look for any changes in L1[768..776]
            for i in 768..777u32 {
                let pte = vm.bus.mem.read_word(boot_pt_addr + i as u64 * 4).unwrap_or(0);
                if pte != 0xCF | (((i - 768) as u32) << 20) {
                    println!("  L1[{}] CHANGED: 0x{:08X}", i, pte);
                }
            }
            
            // Check early_pg_dir (PA 0x00802000) L1[768]
            let early_l1_768 = vm.bus.mem.read_word(0x00802000 + 768 * 4).unwrap_or(0);
            println!("early_pg_dir L1[768]: 0x{:08X}", early_l1_768);
            
            // Try manual MMU translation
            println!("\n=== Manual MMU check ===");
            let va = 0xC00010DAu64;
            let l1_idx = ((va >> 22) & 0x3FF) as u32;
            let l1_pte = vm.bus.mem.read_word(boot_pt_addr + l1_idx as u64 * 4).unwrap_or(0);
            println!("VA 0x{:08X}: L1[{}] = 0x{:08X}", va, l1_idx, l1_pte);
            if l1_pte & 1 != 0 {
                let is_leaf = (l1_pte & 0xE) != 0;
                if is_leaf {
                    let ppn = ((l1_pte >> 10) & 0x3FFFFF) as u64;
                    let pa = (ppn << 12) | (va & 0x3FFFFF);
                    println!("  Megapage: PPN=0x{:05X}, PA=0x{:08X}", ppn, pa);
                    let val = vm.bus.mem.read_word(pa).unwrap_or(0);
                    println!("  Value at PA 0x{:08X}: 0x{:08X}", pa, val);
                } else {
                    let l2_base = ((l1_pte >> 10) & 0x3FFFFF) as u64;
                    let l2_idx = ((va >> 12) & 0x3FF) as u32;
                    let l2_pte = vm.bus.mem.read_word((l2_base << 12) + l2_idx as u64 * 4).unwrap_or(0);
                    println!("  L2 base PPN=0x{:05X}, L2[{}]=0x{:08X}", l2_base, l2_idx, l2_pte);
                }
            }
            break;
        }
        
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 {
                println!("SPIN at 0x{:08X}", vm.cpu.pc);
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
    }
}
