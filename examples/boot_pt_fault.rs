use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, _fw_addr, _entry, _dtb) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(&kernel_image, None, 512, bootargs).unwrap();

    let fw_addr_u32 = _fw_addr as u32;

    // Run until we hit the firmware jump or S-mode fault
    for i in 0..200_000 {
        // Check for M-mode trap at fw_addr
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            println!(
                "[{}] M-TRAP: cause={} mepc={:#010x}",
                i, cause_code, vm.cpu.csr.mepc
            );
        }

        vm.step();

        // Detect S-mode fault
        if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
            if vm.cpu.csr.scause != 0 {
                let scause = vm.cpu.csr.scause;
                let cause_code = scause & !(1u32 << 31);
                if cause_code == 13 || cause_code == 12 || cause_code == 15 {
                    println!(
                        "[{}] S-mode page fault: scause={} sepc={:#010x} stval={:#010x}",
                        i, scause, vm.cpu.csr.sepc, vm.cpu.csr.stval
                    );

                    // Check the page table for the faulting address
                    let fault_vaddr = vm.cpu.csr.stval;
                    let satp = vm.cpu.csr.satp;
                    let root_ppn = (satp & 0x003FFFFF) as u64;
                    let root_phys = root_ppn * 4096;

                    println!("  satp={:#010x} root PT at phys {:#010x}", satp, root_phys);

                    // Walk page table manually
                    let vpn = [(fault_vaddr >> 22) & 0x3FF, (fault_vaddr >> 12) & 0x3FF];

                    // L1 entry
                    let l1_addr = root_phys + (vpn[0] as u64) * 4;
                    let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                    let l1_v = l1_entry & 1;
                    let l1_r = (l1_entry >> 1) & 1;
                    let l1_x = (l1_entry >> 3) & 1;
                    let l1_ppn = (l1_entry >> 10) & 0x3FFFFF;

                    println!(
                        "  L1[{:#x}] at {:#010x} = {:#010x} V={} R={} X={} PPN={:#x}",
                        vpn[0], l1_addr, l1_entry, l1_v, l1_r, l1_x, l1_ppn
                    );

                    if l1_v != 0 && l1_r == 0 && l1_x == 1 {
                        // Megapage
                        let mapped_paddr = (l1_ppn << 22) | (fault_vaddr & 0x3FFFFF);
                        println!("  -> Megapage maps to phys {:#010x}", mapped_paddr);
                    } else if l1_v != 0 {
                        // L2 lookup
                        let l2_addr = (l1_ppn as u64) * 4096 + (vpn[1] as u64) * 4;
                        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                        let l2_v = l2_entry & 1;
                        let l2_r = (l2_entry >> 1) & 1;
                        let l2_w = (l2_entry >> 2) & 1;
                        let l2_x = (l2_entry >> 3) & 1;
                        let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;

                        println!(
                            "  L2[{:#x}] at {:#010x} = {:#010x} V={} R={} W={} X={} PPN={:#x}",
                            vpn[1], l2_addr, l2_entry, l2_v, l2_r, l2_w, l2_x, l2_ppn
                        );

                        if l2_v != 0 {
                            let mapped_paddr = (l2_ppn << 12) | (fault_vaddr & 0xFFF);
                            println!("  -> 4K page maps to phys {:#010x}", mapped_paddr);
                        } else {
                            println!("  -> L2 entry NOT VALID! Page not mapped.");
                        }
                    } else {
                        println!("  -> L1 entry NOT VALID! Page not mapped.");
                    }

                    break;
                }
            }
        }

        // Also detect jump to low addresses
        if vm.cpu.pc < 0x100 && i > 1000 {
            println!("[{}] Jumped to low address {:#010x}", i, vm.cpu.pc);
            break;
        }
    }
}
