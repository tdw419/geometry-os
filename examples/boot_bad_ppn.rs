/// Check if new L2 entries with bad PPNs appear after the fixup
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn check_bad_ppns(vm: &mut RiscvVm, pg_dir_phys: u64, label: &str) -> u32 {
    let mut bad = 0u32;
    for i in 0..1024u32 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte == 0 { continue; }
        let ppn = (pte >> 10) & 0x3FFFFF;
        let v = pte & 1;
        let r = (pte >> 1) & 1;
        let w = (pte >> 2) & 1;
        let x = (pte >> 3) & 1;
        let is_leaf = r == 1 || w == 1 || x == 1;
        
        if ppn >= 0xC0000 {
            bad += 1;
            eprintln!("  [{}] L1[{}] = 0x{:08X} PPN=0x{:06X} leaf={}", label, i, pte, ppn, is_leaf);
        }
        
        if !is_leaf && v == 1 && ppn < 0x100000 {
            let l2_base = (ppn as u64) * 4096;
            for j in 0..1024u32 {
                let l2_addr = l2_base + (j as u64) * 4;
                let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                if l2_pte == 0 { continue; }
                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                if l2_ppn >= 0xC0000 {
                    bad += 1;
                    if bad <= 20 {
                        let vpn1 = i;
                        let vpn0 = j;
                        let va = ((vpn1 as u64) << 22) | ((vpn0 as u64) << 12);
                        eprintln!("  [{}] L2[{},{}] = 0x{:08X} PPN=0x{:06X} VA=0x{:08X}", 
                            label, i, j, l2_pte, l2_ppn, va);
                    }
                }
            }
        }
    }
    bad
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Check right after setup
    let early_pg_dir = 0x00802000u64;
    eprintln!("=== Right after boot_linux_setup ===");
    let bad0 = check_bad_ppns(&mut vm, early_pg_dir, "setup");

    // Run 100K steps
    let mut count: u64 = 0;
    while count < 100_000 {
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
    
    eprintln!("\n=== After 100K steps ===");
    eprintln!("PC=0x{:08X} satp=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
    let bad1 = check_bad_ppns(&mut vm, early_pg_dir, "100K");

    // Run to 180K steps  
    while count < 180_000 {
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

    eprintln!("\n=== After 180K steps ===");
    eprintln!("PC=0x{:08X} satp=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
    let bad2 = check_bad_ppns(&mut vm, early_pg_dir, "180K");
    
    eprintln!("\nSummary: bad PPNs at setup={}, 100K={}, 180K={}", bad0, bad1, bad2);
}
