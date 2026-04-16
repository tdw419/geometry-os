use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    let boot_pt_addr = 0x0157A000u64;
    let max = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    
    while count < max {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
        
        // Detect setup_vm return: PC goes from setup_vm range to 0xC00010EE
        if pc > 0xC0400000 && pc < 0xC0500000 && vm.cpu.pc == 0xC00010EE {
            println!("[{}] setup_vm returned to PC=0x{:08X}", count, vm.cpu.pc);
            
            // Check L1[768]
            let l1_768 = vm.bus.mem.read_word(boot_pt_addr + 768 * 4).unwrap_or(0);
            println!("L1[768]: 0x{:08X} (expect 0x000000CF)", l1_768);
            if l1_768 & 1 != 0 {
                let ppn = (l1_768 >> 10) & 0x3FFFFF;
                println!("  PPN: 0x{:05X} -> PA 0x{:08X}", ppn, (ppn as u64) << 12);
            }
            
            // Check all L1 entries 768-776
            for i in 768..777u32 {
                let pte = vm.bus.mem.read_word(boot_pt_addr + i as u64 * 4).unwrap_or(0);
                let expected = 0xCF | (((i - 768) as u32) << 20);
                if pte != expected {
                    println!("  L1[{}] CHANGED: 0x{:08X} (expected 0x{:08X})", i, pte, expected);
                }
            }
            
            // Run a few more instructions to reach the auipc
            for _ in 0..20 {
                let _ = vm.step();
                count += 1;
                if vm.cpu.pc == 0xC00010DA {
                    println!("\n[{}] At auipc PC=0x{:08X}", count, vm.cpu.pc);
                    // Read instruction via bus (which goes through MMU? No, bus.read_word doesn't use MMU)
                    // The CPU fetches through MMU. Let me read what the MMU would translate to.
                    let va = vm.cpu.pc as u64;
                    let l1_idx = ((va >> 22) & 0x3FF) as u32;
                    let l1_pte = vm.bus.mem.read_word(boot_pt_addr + l1_idx as u64 * 4).unwrap_or(0);
                    println!("L1[{}] = 0x{:08X}", l1_idx, l1_pte);
                    
                    let is_leaf = (l1_pte & 0xE) != 0;
                    if is_leaf {
                        let ppn = ((l1_pte >> 10) & 0x3FFFFF) as u64;
                        let pa = (ppn << 12) | (va & 0x3FFFFF);
                        let val = vm.bus.mem.read_word(pa).unwrap_or(0);
                        println!("Megapage PA=0x{:08X}, value=0x{:08X}", pa, val);
                    } else {
                        let l2_base = ((l1_pte >> 10) & 0x3FFFFF) as u64;
                        let l2_idx = ((va >> 12) & 0x3FF) as u32;
                        let l2_addr = (l2_base << 12) + l2_idx as u64 * 4;
                        let l2_pte = vm.bus.mem.read_word(l2_addr).unwrap_or(0);
                        println!("L2 base=0x{:05X}, L2[{}]=0x{:08X}", l2_base, l2_idx, l2_pte);
                        if l2_pte & 1 != 0 {
                            let l2_ppn = ((l2_pte >> 10) & 0x3FFFFF) as u64;
                            let pa = (l2_ppn << 12) | (va & 0xFFF);
                            let val = vm.bus.mem.read_word(pa).unwrap_or(0);
                            println!("L2 leaf PA=0x{:08X}, value=0x{:08X}", pa, val);
                        }
                    }
                    break;
                }
            }
            break;
        }
        
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 {
                println!("SPIN at 0x{:08X} before setup_vm return", vm.cpu.pc);
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
    }
}
