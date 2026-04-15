fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) = 
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let table_addr = 0xC1400AE8u64;
    let dst_addr = 0xC1CCA520u64;
    
    // Run to 11.765M
    for i in 0..11_765_000u64 {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mc = vm.cpu.csr.mcause & !(1u32<<31);
            if mc == csr::CAUSE_ECALL_S {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mc != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc; vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec; vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
            } else {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            continue;
        }
        let _ = vm.step();
    }
    
    // Check TLB entries for the two addresses
    let table_vpn = (table_addr >> 12) as u32;
    let dst_vpn = (dst_addr >> 12) as u32;
    let asid: u16 = ((vm.cpu.csr.satp >> 16) & 0xFFFF) as u16;
    
    eprintln!("SATP=0x{:08X} ASID={}", vm.cpu.csr.satp, asid);
    eprintln!();
    
    // Table address translation
    if let Some((ppn, flags)) = vm.cpu.tlb.lookup(table_vpn, asid) {
        let pa = ((ppn as u64) << 12) | (table_addr & 0xFFF);
        eprintln!("TLB: table VA 0xC1400AE8 -> VPN={} -> PPN=0x{:X} flags=0x{:X} PA=0x{:X}",
            table_vpn, ppn, flags, pa);
    } else {
        eprintln!("TLB MISS for table VA 0xC1400AE8 (VPN={})", table_vpn);
    }
    
    // Dst address translation
    if let Some((ppn, flags)) = vm.cpu.tlb.lookup(dst_vpn, asid) {
        let pa = ((ppn as u64) << 12) | (dst_addr & 0xFFF);
        eprintln!("TLB: dst   VA 0xC1CCA520 -> VPN={} -> PPN=0x{:X} flags=0x{:X} PA=0x{:X}",
            dst_vpn, ppn, flags, pa);
    } else {
        eprintln!("TLB MISS for dst VA 0xC1CCA520 (VPN={})", dst_vpn);
    }
    
    // Check if they map to the same physical page
    if let (Some((ppn1, _)), Some((ppn2, _))) = (
        vm.cpu.tlb.lookup(table_vpn, asid),
        vm.cpu.tlb.lookup(dst_vpn, asid)
    ) {
        eprintln!("\nSame physical page? {}", ppn1 == ppn2);
        if ppn1 != ppn2 {
            eprintln!("Different PPNs: table=0x{:X} dst=0x{:X} diff=0x{:X}",
                ppn1, ppn2, (ppn1 as i64 - ppn2 as i64).abs());
        }
    }
    
    // Also check: is the dst address close to the table address in physical memory?
    // If there's a page table bug, the physical address might be wrong
    // Let's manually walk the page table for both addresses
    eprintln!("\n--- Manual page table walk ---");
    eprintln!("Table VA 0xC1400AE8:");
    let satp_val = vm.cpu.csr.satp;
    let pt_base = ((satp_val & 0x3FFFFF) as u64) << 12;
    eprintln!("  Page table root: 0x{:X}", pt_base);
    
    // L1 index for 0xC1400AE8
    let l1_idx_table = ((0xC1400AE8u32 >> 22) & 0x3FF) as u64;
    let l1_addr_table = pt_base + l1_idx_table * 4;
    let l1_pte_table = vm.bus.read_word(l1_addr_table).unwrap_or(0);
    eprintln!("  L1[{}] at PA 0x{:X} = 0x{:08X}", l1_idx_table, l1_addr_table, l1_pte_table);
    
    let l1_idx_dst = ((0xC1CCA520u32 >> 22) & 0x3FF) as u64;
    let l1_addr_dst = pt_base + l1_idx_dst * 4;
    let l1_pte_dst = vm.bus.read_word(l1_addr_dst).unwrap_or(0);
    eprintln!("  L1[{}] at PA 0x{:X} = 0x{:08X}", l1_idx_dst, l1_addr_dst, l1_pte_dst);
    
    // Check if they share the same L1 entry (same megapage)
    eprintln!("\nSame L1 index? table_idx={} dst_idx={}", l1_idx_table, l1_idx_dst);
}
