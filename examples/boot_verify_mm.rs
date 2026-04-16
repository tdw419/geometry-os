/// Diagnostic: verify that the kernel's virtual addresses are properly translated.
/// Read instructions at the kernel's current PC using both VA (through bus which does MMU)
/// and PA (by computing PA from the page table manually).

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 5_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    // Run for a while
    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _ = vm.step();
        count += 1;
    }

    // Now check the current state
    println!("[state] count={} PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    println!("[state] ecall_count={} mcause={} sepc=0x{:08X} scause=0x{:08X}",
        vm.cpu.ecall_count, vm.cpu.csr.mcause, vm.cpu.csr.sepc, vm.cpu.csr.scause);
    println!("[state] stvec=0x{:08X} sie={} spie={}",
        vm.cpu.csr.stvec,
        (vm.cpu.csr.mstatus >> 1) & 1,
        (vm.cpu.csr.mstatus >> 5) & 1);

    // Check register state
    println!("[regs] SP(x2)=0x{:08X} GP(x3)=0x{:08X} TP(x4)=0x{:08X} RA(x1)=0x{:08X}",
        vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4], vm.cpu.x[1]);
    println!("[regs] a0(x10)=0x{:08X} a7(x17)=0x{:08X}", vm.cpu.x[10], vm.cpu.x[17]);

    // Check CLINT timer state
    println!("[clint] mtime=0x{:016X} mtimecmp=0x{:016X} mip_timer={}",
        vm.bus.clint.mtime, vm.bus.clint.mtimecmp,
        (vm.cpu.csr.mip >> 7) & 1);

    // Try to read instruction at current PC through the bus
    let pc = vm.cpu.pc as u64;
    match vm.bus.read_word(pc) {
        Ok(word) => println!("[read] VA 0x{:08X} -> word 0x{:08X}", pc as u32, word),
        Err(e) => println!("[read] VA 0x{:08X} -> ERROR: {:?}", pc as u32, e),
    }

    // Manually walk the page table for the current PC
    let satp_val = vm.cpu.csr.satp;
    let pt_root_ppn = (satp_val & 0x003F_FFFF) as u64;
    let pt_root_phys = pt_root_ppn * 4096;
    println!("[pt] SATP=0x{:08X} root_ppn=0x{:X} root_phys=0x{:08X}",
        satp_val, pt_root_ppn, pt_root_phys as u32);

    // Walk L1
    let vpn1 = ((pc >> 22) & 0x3FF) as u64;
    let l1_entry_addr = pt_root_phys + vpn1 * 4;
    match vm.bus.read_word(l1_entry_addr) {
        Ok(l1_pte) => {
            let l1_ppn = ((l1_pte >> 10) & 0x003F_FFFF) as u64;
            let l1_v = l1_pte & 1;
            let l1_r = (l1_pte >> 1) & 1;
            let l1_w = (l1_pte >> 2) & 1;
            let l1_x = (l1_pte >> 3) & 1;
            let is_leaf = (l1_pte & 0xE) != 0;
            println!("[pt] L1[{}] at PA 0x{:08X}: PTE=0x{:08X} V={} R={} W={} X={} PPN=0x{:X} is_leaf={}",
                vpn1 as u32, l1_entry_addr as u32, l1_pte,
                l1_v, l1_r, l1_w, l1_x, l1_ppn, is_leaf);

            if is_leaf {
                // Megapage: PA = (PPN << 12) | offset
                let offset = pc & 0x3FFFFF;
                let pa = (l1_ppn << 12) | offset;
                println!("[pt] Megapage PA = 0x{:08X} (PPN=0x{:X} << 12 | offset=0x{:X})",
                    pa as u32, l1_ppn, offset as u32);
                match vm.bus.read_word(pa) {
                    Ok(word) => println!("[pt] PA 0x{:08X} -> word 0x{:08X}", pa as u32, word),
                    Err(e) => println!("[pt] PA 0x{:08X} -> ERROR: {:?}", pa as u32, e),
                }
            } else {
                // L2 walk
                let vpn0 = ((pc >> 12) & 0x3FF) as u64;
                let l2_entry_addr = (l1_ppn << 12) + vpn0 * 4;
                match vm.bus.read_word(l2_entry_addr) {
                    Ok(l2_pte) => {
                        let l2_ppn = ((l2_pte >> 10) & 0x003F_FFFF) as u64;
                        let l2_v = l2_pte & 1;
                        let l2_r = (l2_pte >> 1) & 1;
                        let l2_w = (l2_pte >> 2) & 1;
                        let l2_x = (l2_pte >> 3) & 1;
                        let offset = pc & 0xFFF;
                        let pa = (l2_ppn << 12) | offset;
                        println!("[pt] L2[{}] at PA 0x{:08X}: PTE=0x{:08X} V={} R={} W={} X={} PPN=0x{:X}",
                            vpn0 as u32, l2_entry_addr as u32, l2_pte,
                            l2_v, l2_r, l2_w, l2_x, l2_ppn);
                        println!("[pt] L2 page PA = 0x{:08X} (PPN=0x{:X} << 12 | offset=0x{:X})",
                            pa as u32, l2_ppn, offset as u32);
                        match vm.bus.read_word(pa) {
                            Ok(word) => println!("[pt] PA 0x{:08X} -> word 0x{:08X}", pa as u32, word),
                            Err(e) => println!("[pt] PA 0x{:08X} -> ERROR: {:?}", pa as u32, e),
                        }
                    }
                    Err(e) => println!("[pt] L2[{}] at PA 0x{:08X} -> ERROR reading L2: {:?}",
                        ((pc >> 12) & 0x3FF) as u32, l2_entry_addr as u32, e),
                }
            }
        }
        Err(e) => println!("[pt] L1[{}] at PA 0x{:08X} -> ERROR: {:?}",
            vpn1 as u32, l1_entry_addr as u32, e),
    }

    // Check what the kernel should be at this PC using objdump
    println!("\n[kernel] To disassemble: riscv64-linux-gnu-objdump -d -m riscv:rv32imac --start-address=0x{:08X} --stop-address=0x{:08X} .geometry_os/build/linux-6.14/vmlinux",
        vm.cpu.pc, vm.cpu.pc + 32);

    // Also check the UART and SBI output
    println!("\n[uart] tx_buf len={}", vm.bus.uart.tx_buf.len());
    println!("[sbi] console_output len={}", vm.bus.sbi.console_output.len());
    println!("[sbi] shutdown={}", vm.bus.sbi.shutdown_requested);
}
