/// Dump early_pg_dir AFTER boot has progressed (run 200K steps first)
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run 200K steps with trap handling
    let max_count = 200_000u64;
    let mut count: u64 = 0;
    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M {
                let r = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                } else if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP)) | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE)) | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }
        vm.step();
        count += 1;
    }

    // Now dump the current page table
    let satp = vm.cpu.csr.satp;
    let ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_phys = ppn * 4096;
    
    eprintln!("\n=== Page table dump at count={} ===", count);
    eprintln!("satp=0x{:08X} pg_dir_phys=0x{:08X}", satp, pg_dir_phys);
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);

    let mut l1_count = 0;
    let mut l2_count = 0;
    let mut bad_ppn_count = 0;
    
    for i in 0..1024u32 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte == 0 { continue; }
        l1_count += 1;
        
        let ppn_val = (pte >> 10) & 0x3FFFFF;
        let v = pte & 1;
        let r = (pte >> 1) & 1;
        let w = (pte >> 2) & 1;
        let x = (pte >> 3) & 1;
        
        let is_leaf = r == 1 || w == 1 || x == 1;
        let is_bad_ppn = ppn_val >= 0xC0000;
        if is_bad_ppn { bad_ppn_count += 1; }
        
        if i >= 760 && i <= 780 {
            eprintln!("  L1[{}] = 0x{:08X}  PPN=0x{:06X} PA=0x{:08X} leaf={} {}",
                i, pte, ppn_val, ppn_val * 4096, is_leaf, 
                if is_bad_ppn { "<<< BAD PPN" } else { "" });
        }
        
        // If non-leaf, dump L2
        if !is_leaf && v == 1 && ppn_val < 0x100000 {
            let l2_base = (ppn_val as u64) * 4096;
            for j in 0..1024u32 {
                let l2_addr = l2_base + (j as u64) * 4;
                let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                if l2_pte == 0 { continue; }
                l2_count += 1;
                
                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                let l2_bad = l2_ppn >= 0xC0000;
                if l2_bad { bad_ppn_count += 1; }
                
                // Check specific VAs that the kernel might access
                // VA 0xC1400AE8: exception handler table
                let vpn1 = i;
                let vpn0 = j;
                let va = ((vpn1 as u64) << 22) | ((vpn0 as u64) << 12);
                if va >= 0xC1400000 && va <= 0xC1410000 {
                    eprintln!("  L2[{},{}] = 0x{:08X}  PPN=0x{:06X} PA=0x{:08X} VA=0x{:08X} {}",
                        i, j, l2_pte, l2_ppn, l2_ppn * 4096, va,
                        if l2_bad { "<<< BAD PPN" } else { "" });
                }
            }
        }
    }
    
    eprintln!("\nSummary: {} L1 entries, {} L2 entries, {} bad PPNs (>=0xC0000)", 
        l1_count, l2_count, bad_ppn_count);
    
    // Also check: what PA does the MMU translate VA 0xC1400AE8 to?
    // And what's stored at that PA?
    let pa_check = pg_dir_phys; // just a placeholder
    eprintln!("\nDirect read at PA 0x01400AE8: 0x{:08X}", 
        vm.bus.read_word(0x01400AE8).unwrap_or(0));
}
