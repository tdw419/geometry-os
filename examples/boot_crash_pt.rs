/// Diagnostic: check page table state at the crash point, with detailed L1 dump.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{StepResult, Privilege};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let crash_count = 177_470u64;
    let mut count: u64 = 0;
    let mut trampoline_patched = false;

    while count < crash_count {
        if vm.bus.sbi.shutdown_requested { break; }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            vm.bus.write_word(0x0148_4000u64, identity_pte).ok();
            vm.bus.write_word(0x0080_2000u64, identity_pte).ok();
            trampoline_patched = true;
            eprintln!("[diag] Injected trampoline L1[0]=0xEF at count={}", count);
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if cause_code == csr::CAUSE_ECALL_S {
                    let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                        vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                        &mut vm.bus.uart, &mut vm.bus.clint);
                    if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
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
                    } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
                } else { vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4); }
            } else {
                let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint);
                if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            count += 1;
            continue;
        }

        let result = vm.step();
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && count > 177400 {
                    let fault_type = match result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        StepResult::StoreFault => "store",
                        _ => unreachable!(),
                    };
                    eprintln!("[diag] S-mode {} fault #1 at count={}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} satp=0x{:08X}",
                        fault_type, count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec, vm.cpu.csr.satp);
                    // Continue stepping to see if handle_exception runs or immediately faults again
                    for extra in 0..50 {
                        let r2 = vm.step();
                        let ft2 = match r2 {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            StepResult::Ok => "ok",
                            StepResult::Ebreak => { eprintln!("[diag] EBREAK after {} extra steps", extra); break; }
                            StepResult::Ecall => "ecall",
                        };
                        if !matches!(r2, StepResult::Ok) {
                            eprintln!("[diag]   +{}: {} fault PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} priv={:?}",
                                extra+1, ft2, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.privilege);
                        }
                    }
                    break;
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
    }

    eprintln!("\n=== State at crash ===");
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SP=0x{:08X} RA=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1]);

    let satp = vm.cpu.csr.satp;
    let satp_ppn = (satp & 0x3FFFFF) as u64;
    let l1_base = satp_ppn << 12;
    eprintln!("satp=0x{:08X} PT_base=0x{:08X}", satp, l1_base);
    eprintln!("stvec=0x{:08X} sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
        vm.cpu.csr.stvec, vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);

    // Dump all non-zero L1 entries
    eprintln!("\n=== All non-zero L1 entries (PT at 0x{:08X}) ===", l1_base);
    for i in 0..1024 {
        let addr = l1_base + (i as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        if entry != 0 {
            let v = entry & 1;
            let r = (entry >> 1) & 1;
            let x = (entry >> 3) & 1;
            let ppn = (entry >> 10) as u64;
            let is_mega = r == 1 || x == 1;
            if is_mega {
                eprintln!("L1[{}] = 0x{:08X} MEGAPAGE PA=0x{:08X}-0x{:08X}", i, entry, ppn << 12, (ppn << 12) + 0x3FFFFF);
            } else {
                eprintln!("L1[{}] = 0x{:08X} L2_table at PA=0x{:08X}", i, entry, ppn << 12);
            }
        }
    }

    // Walk the faulting address
    let fault_va = 0xC14809D0u32;
    let vpn1 = (fault_va >> 22) as usize;
    let vpn0 = ((fault_va >> 12) & 0x3FF) as usize;
    eprintln!("\n=== Walk VA 0x{:08X} (VPN1={} VPN0={}) ===", fault_va, vpn1, vpn0);
    let l1_entry = vm.bus.read_word(l1_base + (vpn1 as u64) * 4).unwrap_or(0);
    let l1_ppn = (l1_entry >> 10) as u64;
    let is_mega = (l1_entry & 0xE) != 0;
    if is_mega {
        let pa = (l1_ppn << 12) | ((fault_va as u64) & 0x3FFFFF);
        eprintln!("Megapage -> PA=0x{:08X}", pa);
    } else {
        let l2_base = l1_ppn << 12;
        let l2_entry = vm.bus.read_word(l2_base + (vpn0 as u64) * 4).unwrap_or(0);
        eprintln!("L2[{}] at PA 0x{:08X} = 0x{:08X}", vpn0, l2_base + (vpn0 as u64) * 4, l2_entry);
        if l2_entry & 1 != 0 {
            let l2_ppn = (l2_entry >> 10) as u64;
            let pa = (l2_ppn << 12) | ((fault_va as u64) & 0xFFF);
            eprintln!("L2 valid -> PA=0x{:08X}", pa);
        } else {
            eprintln!("L2 NOT VALID");
        }
    }
}
