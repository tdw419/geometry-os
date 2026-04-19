// Diagnostic: Check what's causing the page fault at 0xCF7DCB68
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs_data = fs::read(initramfs_path).unwrap();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, Some(&initramfs_data), 128, bootargs).unwrap();

    // Run until we hit the fault or 8M instructions
    let max_count: u64 = 8_000_000;
    let mut count: u64 = 0;
    let fault_va: u32 = 0xCF7DCB68;
    let fault_vpn1: u32 = fault_va >> 22;
    let fault_vpn0: u32 = (fault_va >> 12) & 0x3FF;

    eprintln!("Fault VA: 0x{:08X} VPN1={} VPN0={}", fault_va, fault_vpn1, fault_vpn0);

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        count += 1;

        // Check if we hit the fault region (PC near 0xC0080A46 or 0xC0221F64)
        if count % 500_000 == 0 {
            eprintln!("[{}] PC=0x{:08X} SATP=0x{:08X}", count / 1_000_000, vm.cpu.pc, vm.cpu.csr.satp);
        }

        // Check for S-mode fault (sepc gets set)
        if vm.cpu.csr.scause != 0 && count > 7_000_000 {
            eprintln!("\n[{}] S-mode fault! scause=0x{:X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                count, vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);

            // Check the page table for the fault address
            let satp = vm.cpu.csr.satp;
            let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
            let pg_dir_phys = pg_dir_ppn * 4096;
            eprintln!("Page table at PA 0x{:08X}", pg_dir_phys);

            // Read L1 entry
            let l1_addr = pg_dir_phys + fault_vpn1 as u64 * 4;
            let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
            eprintln!("L1[{}] at PA 0x{:08X} = 0x{:08X}", fault_vpn1, l1_addr, l1_entry);

            if l1_entry & 1 != 0 {
                let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64 * 4096;
                let l2_addr = l2_ppn + fault_vpn0 as u64 * 4;
                let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                eprintln!("L2[{}] at PA 0x{:08X} = 0x{:08X}", fault_vpn0, l2_addr, l2_entry);
            } else {
                eprintln!("L1 entry not valid - page not mapped!");
                // Show surrounding L1 entries
                for i in (fault_vpn1.saturating_sub(2))..=(fault_vpn1 + 2) {
                    let addr = pg_dir_phys + i as u64 * 4;
                    let entry = vm.bus.read_word(addr).unwrap_or(0);
                    let marker = if i == fault_vpn1 { " <<<" } else { "" };
                    eprintln!("  L1[{}] = 0x{:08X}{}", i, entry, marker);
                }
            }

            // Also check what instruction is at sepc
            let sepc = vm.cpu.csr.sepc;
            if sepc >= 0xC0000000 {
                let sepc_pa = (sepc - 0xC0000000) as u64;
                let instr = vm.bus.read_word(sepc_pa).unwrap_or(0);
                eprintln!("Instruction at sepc=0x{:08X} (PA 0x{:08X}): 0x{:08X}", sepc, sepc_pa, instr);
            }

            // Show all non-zero L1 entries in range 768..1024
            eprintln!("\nNon-zero L1 entries [768..900]:");
            for i in 768..900u32 {
                let addr = pg_dir_phys + i as u64 * 4;
                let entry = vm.bus.read_word(addr).unwrap_or(0);
                if entry != 0 {
                    let is_l2 = (entry & 0xE) == 0x4; // leaf with R/W/X
                    let is_mega = (entry & 0xE) == 0xE && (entry & 0xFFFFF) != 0;
                    let pte_type = if is_mega { "MEGA" } else if is_l2 { "L2ptr" } else { "???" };
                    eprintln!("  L1[{}] = 0x{:08X} ({})", i, entry, pte_type);
                }
            }

            break;
        }
    }
}
