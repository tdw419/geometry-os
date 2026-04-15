use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, None, 128, bootargs,
        ).unwrap();
    
    // Run to the first fault at address 0x100 (around 177K instructions)
    let mut fault_count = 0;
    let target_count = 200_000;
    
    for i in 0..target_count {
        vm.step();
    }
    
    println!("=== State after {} instructions ===", target_count);
    println!("PC: 0x{:08X}, Priv: {:?}, satap: 0x{:08X}", 
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    println!("sepc: 0x{:08X}, scause: 0x{:08X}, stval: 0x{:08X}, stvec: 0x{:08X}",
        vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
    
    // Now manually walk the page table for address 0x100
    let fault_va = 0x100u32;
    let satp = vm.cpu.csr.satp;
    let root_ppn = satp & 0x3FFFFF;
    let root_addr = (root_ppn as u64) << 12;
    
    let vpn1 = ((fault_va >> 22) & 0x3FF) as u64;
    let vpn0 = ((fault_va >> 12) & 0x3FF) as u64;
    
    println!("\n=== Page table walk for VA 0x{:08X} ===", fault_va);
    println!("satp=0x{:08X}, root_ppn={}, root_addr=0x{:08X}", satp, root_ppn, root_addr);
    println!("vpn1={}, vpn0={}", vpn1, vpn0);
    
    let l1_addr = root_addr + vpn1 * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1 PTE at 0x{:08X}: 0x{:08X}", l1_addr, l1_pte);
    println!("  V={}, R={}, W={}, X={}, U={}, G={}, A={}, D={}, PPN={}",
        (l1_pte >> 0) & 1, (l1_pte >> 1) & 1, (l1_pte >> 2) & 1, (l1_pte >> 3) & 1,
        (l1_pte >> 4) & 1, (l1_pte >> 5) & 1, (l1_pte >> 6) & 1, (l1_pte >> 7) & 1,
        (l1_pte >> 10) & 0x3FFFFF);
    
    if (l1_pte & 1) == 0 {
        println!("  L1 entry INVALID - page fault expected");
    } else if (l1_pte & 0xE) != 0 {
        println!("  L1 entry is MEGAPAGE (leaf)");
    } else {
        let l2_ppn = ((l1_pte >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn << 12;
        let l2_addr = l2_base + vpn0 * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!("L2 PTE at 0x{:08X}: 0x{:08X}", l2_addr, l2_pte);
        println!("  V={}, R={}, W={}, X={}, U={}, G={}, A={}, D={}, PPN={}",
            (l2_pte >> 0) & 1, (l2_pte >> 1) & 1, (l2_pte >> 2) & 1, (l2_pte >> 3) & 1,
            (l2_pte >> 4) & 1, (l2_pte >> 5) & 1, (l2_pte >> 6) & 1, (l2_pte >> 7) & 1,
            (l2_pte >> 10) & 0x3FFFFF);
        
        if (l2_pte & 1) == 0 {
            println!("  L2 entry INVALID - this is the faulting PTE");
            println!("  Demand paging should patch this with identity map to PPN {}", vpn0);
        } else {
            let ppn = (l2_pte >> 10) & 0x3FFFFF;
            let phys = (ppn << 12) + (fault_va & 0xFFF);
            println!("  Maps to PA 0x{:08X}", phys);
        }
    }
    
    // Now run the demand page patch manually and see if it works
    println!("\n=== Running 500 more instructions (demand page attempts) ===");
    for i in 0..500 {
        vm.step();
    }
    
    // Check if the PTE was patched
    let l1_addr2 = root_addr + vpn1 * 4;
    let l1_pte2 = vm.bus.read_word(l1_addr2).unwrap_or(0);
    println!("L1 PTE after: 0x{:08X}", l1_pte2);
    
    if (l1_pte2 & 1) != 0 && (l1_pte2 & 0xE) == 0 {
        let l2_ppn2 = ((l1_pte2 >> 10) & 0x3FFFFF) as u64;
        let l2_addr2 = l2_ppn2 << 12 + vpn0 * 4;
        let l2_pte2 = vm.bus.read_word(l2_addr2).unwrap_or(0);
        println!("L2 PTE at 0x{:08X} after: 0x{:08X}", l2_addr2, l2_pte2);
        println!("  V={}, PPN={}", (l2_pte2 >> 0) & 1, (l2_pte2 >> 10) & 0x3FFFFF);
    }
    
    println!("\nPC: 0x{:08X}, scause: 0x{:08X}, stval: 0x{:08X}",
        vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval);
}
