use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, _fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            None,
            128,
            bootargs,
        ).unwrap();
    
    // Run for 177000 instructions (just before the first fault at 177717)
    for _ in 0..177000 {
        vm.step();
    }
    
    let satp = vm.cpu.csr.satp;
    println!("satp = 0x{:08X}", satp);
    
    // SV32: mode=bit[31], asid=bits[30:22], ppn=bits[21:0]
    let mode = (satp >> 31) & 1;
    let asid = (satp >> 22) & 0xFF;
    let ppn = satp & 0x3FFFFF;
    println!("mode={}, asid={}, ppn=0x{:06X}", mode, asid, ppn);
    println!("root page table at physical 0x{:08X}", (ppn as u64) << 12);
    
    // Check L1 entry for VA 0x100
    let vpn1 = (0x100u32 >> 22) & 0x3FF;
    let vpn0 = (0x100u32 >> 12) & 0x3FF;
    println!("\nVA 0x100: VPN[1]={}, VPN[0]={}", vpn1, vpn0);
    
    let root_phys = (ppn as u64) << 12;
    let l1_addr = root_phys + (vpn1 as u64) * 4;
    println!("L1 PTE addr = 0x{:08X}", l1_addr);
    
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1 PTE = 0x{:08X}", l1_pte);
    println!("  V={} R={} W={} X={} U={}", 
        (l1_pte >> 0) & 1, (l1_pte >> 1) & 1, (l1_pte >> 2) & 1,
        (l1_pte >> 3) & 1, (l1_pte >> 4) & 1);
    
    let is_leaf = (l1_pte & 0xE) != 0;
    println!("  is_leaf={}", is_leaf);
    
    if !is_leaf && (l1_pte & 1) != 0 {
        let l2_ppn = (l1_pte >> 10) & 0x3FFFFF;
        let l2_addr = (l2_ppn as u64) << 12;
        let l2_pte_addr = l2_addr + (vpn0 as u64) * 4;
        println!("L2 table at phys 0x{:08X}", l2_addr);
        println!("L2 PTE addr = 0x{:08X}", l2_pte_addr);
        let l2_pte = vm.bus.read_word(l2_pte_addr).unwrap_or(0);
        println!("L2 PTE = 0x{:08X}", l2_pte);
        println!("  V={} R={} W={} X={} U={}", 
            (l2_pte >> 0) & 1, (l2_pte >> 1) & 1, (l2_pte >> 2) & 1,
            (l2_pte >> 3) & 1, (l2_pte >> 4) & 1);
    }
    
    // Check VA 0xC0000000 (kernel text start)
    println!("\n--- VA 0xC0000000 (kernel text) ---");
    let vpn1 = (0xC0000000u32 >> 22) & 0x3FF;
    let vpn0 = (0xC0000000u32 >> 12) & 0x3FF;
    println!("VPN[1]={}, VPN[0]={}", vpn1, vpn0);
    let l1_addr = root_phys + (vpn1 as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1 PTE = 0x{:08X} V={}", l1_pte, l1_pte & 1);
    
    // Print register state
    println!("\n--- Register state ---");
    println!("PC = 0x{:08X}", vm.cpu.pc);
    println!("Priv = {:?}", vm.cpu.privilege);
    println!("x6  = 0x{:08X}", vm.cpu.x[6]);
    println!("x7  = 0x{:08X}", vm.cpu.x[7]);
    println!("x8  = 0x{:08X}", vm.cpu.x[8]);
    println!("x9  = 0x{:08X}", vm.cpu.x[9]);
    println!("x15 = 0x{:08X}", vm.cpu.x[15]);
    println!("SP  = 0x{:08X}", vm.cpu.x[2]);
    println!("stvec = 0x{:08X}", vm.cpu.csr.stvec);
}
