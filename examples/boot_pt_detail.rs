use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image, None, 128, bootargs,
        ).unwrap();
    
    // Run to just before first fault (177567)
    for _ in 0..177_500 {
        vm.step();
    }
    
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let root_phys = (ppn as u64) << 12;
    println!("satp=0x{:08X}, root at 0x{:08X}", satp, root_phys);
    println!("PC=0x{:08X}, priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    
    // Check several key VAs
    let vas: [u32; 6] = [
        0xC0210F14, // trap handler (fetch fault)
        0xC000D5EA, // faulting load instruction
        0x00000000, // low address (fetch fault source)
        0x00000100, // stval from load fault
        0xC0000000, // kernel text start
        0x00802000, // root page table itself
    ];
    
    for va in &vas {
        println!("\n--- VA 0x{:08X} ---", va);
        let vpn1 = (va >> 22) & 0x3FF;
        let vpn0 = (va >> 12) & 0x3FF;
        let l1_addr = root_phys + (vpn1 as u64) * 4;
        
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0xDEAD);
        let is_leaf = (l1_pte & 0xE) != 0;
        println!("  VPN[1]={} VPN[0]={}", vpn1, vpn0);
        println!("  L1 PTE at PA 0x{:08X} = 0x{:08X} (leaf={})", l1_addr, l1_pte, is_leaf);
        
        if is_leaf {
            let ppn_hi = (l1_pte >> 20) & 0xFFF;
            let pa = ((ppn_hi as u64) << 22) | ((vpn0 as u64) << 12);
            println!("  -> Megapage, PA=0x{:08X}", pa);
        } else if (l1_pte & 1) != 0 {
            let l2_ppn = (l1_pte >> 10) & 0x3FFFFF;
            let l2_base = (l2_ppn as u64) << 12;
            let l2_addr = l2_base + (vpn0 as u64) * 4;
            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0xDEAD);
            let l2_leaf = (l2_pte & 0xE) != 0;
            println!("  L2 table at PA 0x{:08X}", l2_base);
            println!("  L2 PTE at PA 0x{:08X} = 0x{:08X} (leaf={})", l2_addr, l2_pte, l2_leaf);
            if l2_leaf {
                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                let pa = ((l2_ppn as u64) << 12) | ((va & 0xFFF) as u64);
                println!("  -> PA=0x{:08X}", pa);
            }
        } else {
            println!("  -> L1 not valid (V=0)");
        }
    }
}
