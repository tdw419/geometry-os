/// Diagnostic: Trace instructions with proper MMU translation.
/// Only log the last 100 instructions before the first fault.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 200_000u64;
    let mut count: u64 = 0;
    let mut last_satap: u32 = vm.cpu.csr.satp;
    let fw_addr_u32 = fw_addr as u32;

    // Ring buffer of last 100 (count, PC, instruction) pairs
    let mut trace_buf: Vec<(u64, u32, u32)> = Vec::with_capacity(100);

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        // Handle M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
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
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // SATP change + fixup
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satap {
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            const PAGE_OFFSET_PPN: u32 = 0xC000_0000 >> 12;
            const PPN_MASK: u32 = 0xFFFF_FC00;
            const LEAF_FLAGS: u32 = 2 | 4 | 8;
            for i in 0..1024u32 {
                let l1_addr = pg_dir_phys + (i as u64) * 4;
                if let Ok(l1_pte) = vm.bus.read_word(l1_addr) {
                    if (l1_pte & 1) == 0 { continue; }
                    let l1_ppn = (l1_pte & PPN_MASK) >> 10;
                    if l1_ppn >= PAGE_OFFSET_PPN {
                        let fp = l1_ppn - PAGE_OFFSET_PPN;
                        vm.bus.write_word(l1_addr, (l1_pte & !PPN_MASK) | (fp << 10)).ok();
                    }
                    if (l1_pte & LEAF_FLAGS) == 0 {
                        let fp2 = if l1_ppn >= PAGE_OFFSET_PPN { l1_ppn - PAGE_OFFSET_PPN } else { l1_ppn };
                        let l2_base = (fp2 as u64) << 12;
                        if l2_base < 0x1000_0000 {
                            for j in 0..1024u32 {
                                if let Ok(l2_pte) = vm.bus.read_word(l2_base + (j as u64) * 4) {
                                    if (l2_pte & 1) == 0 { continue; }
                                    let l2_ppn = (l2_pte & PPN_MASK) >> 10;
                                    if l2_ppn >= PAGE_OFFSET_PPN {
                                        let f = l2_ppn - PAGE_OFFSET_PPN;
                                        vm.bus.write_word(l2_base + (j as u64) * 4, (l2_pte & !PPN_MASK) | (f << 10)).ok();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let identity_pte: u32 = 0x0000_00CF;
            let entries: &[u32] = &[0,1,2,3,4,5,6,7,8,9,10,16,32,48,64,80,96,112,127];
            let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
            if !((l1_0 & 0xCF) == 0xCF && ((l1_0 >> 20) & 0xFFF) == 0) {
                for &idx in entries {
                    vm.bus.write_word(pg_dir_phys + (idx * 4) as u64, identity_pte | (idx << 20)).ok();
                }
                let ex = vm.bus.read_word(pg_dir_phys + 776 * 4).unwrap_or(0);
                if (ex & 1) == 0 {
                    vm.bus.write_word(pg_dir_phys + 776 * 4, identity_pte | (8u32 << 20)).ok();
                }
                vm.cpu.tlb.flush_all();
            }
            last_satap = cur_satp;
        }

        // Use the MMU's translate function to get the physical address
        let inst_word = {
            use geometry_os::riscv::mmu::{translate, AccessType};
            match translate(vm.cpu.pc, AccessType::Fetch, vm.cpu.privilege, false, false, vm.cpu.csr.satp, &mut vm.bus, &mut vm.cpu.tlb) {
                geometry_os::riscv::mmu::TranslateResult::Ok(pa) => vm.bus.read_word(pa).unwrap_or(0),
                _ => 0xDEAD_DEAD_u32,
            }
        };

        trace_buf.push((count, vm.cpu.pc, inst_word));
        if trace_buf.len() > 100 {
            trace_buf.remove(0);
        }

        let step_result = vm.step();
        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("\n[diag] FAULT at count={}: PC=0x{:08X} sepc=0x{:08X} scause=0x{:X} stval=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval);
                eprintln!("[diag] RA=0x{:08X} SP=0x{:08X}", vm.cpu.x[1], vm.cpu.x[2]);
                eprintln!("[diag] Last 100 instructions before fault:");
                for &(cnt, p, i) in &trace_buf {
                    if i == 0xDEAD_DEAD_u32 {
                        eprintln!("  count={}: PC=0x{:08X} UNMAPPED", cnt, p);
                    } else {
                        eprintln!("  count={}: PC=0x{:08X} inst=0x{:08X}", cnt, p, i);
                    }
                }
                break;
            }
            _ => {}
        }

        count += 1;
    }
}
