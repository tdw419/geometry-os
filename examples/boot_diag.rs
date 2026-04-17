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

    // Dump instructions around hot PCs
    for &base in &[0xC00010B0u32, 0xC0002780u32, 0xC020B0D0u32, 0xC006ADD0u32] {
        println!("
--- Instructions around 0x{:08X} ---", base);
        for off in (0u32..32).step_by(2) {
            let addr = base + off;
            if let Ok(hw) = vm.bus.read_half(addr as u64) {
                let is_c = (hw & 0x3) != 0x3;
                if is_c {
                    println!("  {:08X}: {:04X}  (compressed)", addr, hw);
                } else if let Ok(w) = vm.bus.read_word(addr as u64) {
                    println!("  {:08X}: {:08X}", addr, w);
                }
            }
        }
    }
    
    // Check page table for stval=0x804046B4
    // satp=0x80000802 => PPN=0x802 => pgdir at PA 0x802000
    // VA 0x804046B4: VPN[1] = (0x804046B4 >> 22) & 0x3FF = 0x201
    //                VPN[0] = (0x804046B4 >> 12) & 0x3FF = 0x004
    let stval: u32 = 0x804046B4;
    let vpn1 = (stval >> 22) & 0x3FF;
    let vpn0 = (stval >> 12) & 0x3FF;
    let offset = stval & 0xFFF;
    println!("
--- Page table walk for VA 0x{:08X} ---", stval);
    println!("VPN[1]={:#x} VPN[0]={:#x} offset={:#x}", vpn1, vpn0, offset);
    
    let satp = vm.cpu.csr.satp;
    let pgdir = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("satp=0x{:08X} pgdir PA=0x{:08X}", satp, pgdir);
    
    // L1 entry
    let l1_addr = pgdir + (vpn1 as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1[{}] @ PA 0x{:08X} = 0x{:08X}", vpn1, l1_addr, l1_pte);
    
    if l1_pte & 1 != 0 {
        let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
        let is_megapage = (l1_pte & 0xE) != 0; // R|W|X != 0 means leaf
        if is_megapage {
            let megapa = (l1_ppn as u64) * 4096;
            println!("  Megapage -> PA 0x{:08X}", megapa);
            let final_pa = megapa + ((stval & 0x3FFFFF) as u64);
            println!("  Final PA for 0x{:08X} = 0x{:08X}", stval, final_pa);
        } else {
            // L2 table
            let l2_base = (l1_ppn as u64) * 4096;
            let l2_addr = l2_base + (vpn0 as u64) * 4;
            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
            println!("  L2 table at PA 0x{:08X}", l2_base);
            println!("L2[{}] @ PA 0x{:08X} = 0x{:08X}", vpn0, l2_addr, l2_pte);
            if l2_pte & 1 != 0 {
                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                let final_pa = (l2_ppn as u64) * 4096 + (offset as u64);
                println!("  -> PA 0x{:08X}", final_pa);
            } else {
                println!("  INVALID (not mapped!)");
            }
        }
    } else {
        println!("  L1 INVALID (not mapped!)");
    }
    
    // Also check what the stuck PC is doing
    println!("
--- Around stuck PC 0xC020B0F8 ---");
    for off in (-16i32..=16).step_by(2) {
        let addr = (0xC020B0F8 as i64 + off as i64) as u64;
        if let Ok(hw) = vm.bus.read_half(addr) {
            let is_c = (hw & 0x3) != 0x3;
            if is_c {
                println!("  {:08X}: {:04X}  (compressed)", addr as u32, hw);
            } else if let Ok(w) = vm.bus.read_word(addr) {
                println!("  {:08X}: {:08X}", addr as u32, w);
            }
        }
    }
}
