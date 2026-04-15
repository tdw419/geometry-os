fn main() {
    // The __memmove writes to VA 0xC1CCA520 which maps to PA 0x1CCA520.
    // But the page table at PA 0x1002000 is being corrupted.
    // Let's check: is there a SECOND concurrent __memmove or similar operation
    // that writes to a VA mapping to PA 0x1002000?
    //
    // Strategy: at the point of corruption, dump ALL megapage mappings and check
    // if any of them cover PA 0x1002000.
    
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_instr = 16_995_000u64;
    let mut count: u64 = 0;

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
            if cause_code == csr::CAUSE_ECALL_S || cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
                }
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                        | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                        | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        let _ = vm.step();
        count += 1;
    }

    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let pt_target = 0x1002000u64; // page table physical address
    let pt_end = pt_target + 4096;
    
    eprintln!("Page table at PA 0x{:X} - 0x{:X}", pt_target, pt_end);
    eprintln!("");
    
    // Check all L1 megapages to see if any cover the page table PA
    for i in 0..1024 {
        let addr = pt_base + (i as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if (pte & 1) == 0 { continue; }
        
        let rwx = (pte >> 1) & 7;
        let is_leaf = rwx != 0;
        
        if is_leaf {
            // Megapage: PA = PPN[19:10] << 22, covers 2MB
            let ppn_hi = ((pte >> 20) & 0xFFF) as u64;
            let pa_base = ppn_hi << 22;
            let pa_end = pa_base + 0x200000;
            
            // Check if this megapage covers the page table
            if pa_base < pt_end && pa_end > pt_target {
                let va_base = (i as u64) << 22;
                let va_for_pt = va_base + (pt_target - pa_base);
                eprintln!("OVERLAP: L1[{}] VA 0x{:X}-0x{:X} -> PA 0x{:X}-0x{:X}",
                    i, va_base | 0xC0000000, (va_base + 0x200000) | 0xC0000000, pa_base, pa_end);
                eprintln!("  PT at PA 0x{:X} maps to VA 0x{:08X}", pt_target, va_for_pt | 0xC0000000);
                eprintln!("  PTE = 0x{:08X}", pte);
            }
        } else {
            // Non-leaf: check L2 entries
            let l2_ppn = ((pte >> 10) & 0x3FFFFF) as u64;
            let l2_base = l2_ppn << 12;
            
            for j in 0..1024 {
                let l2_addr = l2_base + (j as u64) * 4;
                let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                if (l2_pte & 1) == 0 { continue; }
                
                let l2_ppn_val = ((l2_pte >> 10) & 0x3FFFFF) as u64;
                let pa_base = l2_ppn_val << 12;
                let pa_end = pa_base + 4096;
                
                if pa_base < pt_end && pa_end > pt_target {
                    let va_base = ((i as u64) << 22) | ((j as u64) << 12);
                    let va_for_pt = va_base + (pt_target - pa_base);
                    eprintln!("OVERLAP L2: L1[{}]+L2[{}] VA 0x{:X} -> PA 0x{:X}-0x{:X}",
                        i, j, va_base | 0xC0000000, pa_base, pa_end);
                }
            }
        }
    }
    
    eprintln!("\nPC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}", vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]);
}
