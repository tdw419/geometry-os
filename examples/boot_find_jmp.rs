use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run to 178000 (just before the first fault)
    for i in 0..178000 {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            handle_trap(&mut vm);
            continue;
        }
        vm.step();
    }

    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) << 12;

    // Trace the last 200 steps before the transition
    let mut last_regs: Vec<(u64, u32, [u32; 32])> = Vec::new();
    
    for i in 0..2000 {
        if vm.bus.sbi.shutdown_requested { break; }
        let pc = vm.cpu.pc;
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            handle_trap(&mut vm);
            continue;
        }
        
        // Save registers before step
        let regs = vm.cpu.x;
        let step_num = 178000 + i;
        
        vm.step();
        
        // Check for transition to low address
        if pc >= 0xC0000000 && vm.cpu.pc < 0xC0000000 && vm.cpu.pc != fw_addr_u32 {
            eprintln!("\n*** TRANSITION at step {} ***", step_num);
            eprintln!("PC: 0x{:08X} -> 0x{:08X}", pc, vm.cpu.pc);
            
            // Compute PA of the instruction at VA pc
            let vpn1 = ((pc >> 22) & 0x3FF) as u64;
            let vpn0 = ((pc >> 12) & 0x3FF) as u64;
            let page_off = (pc & 0xFFF) as u64;
            let l1_pte = vm.bus.read_word(pg_dir_phys + vpn1 * 4).unwrap_or(0);
            let l1_ppn = ((l1_pte & 0xFFFF_FC00) >> 10) as u64;
            let pa = if (l1_pte & 0xE) != 0 {
                ((l1_ppn >> 10) << 22) | (vpn0 << 12) | page_off
            } else {
                let l2_base = l1_ppn << 12;
                let l2_pte = vm.bus.read_word(l2_base + vpn0 * 4).unwrap_or(0);
                let l2_ppn = ((l2_pte & 0xFFFF_FC00) >> 10) as u64;
                (l2_ppn << 12) | page_off
            };
            let inst = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("VA 0x{:08X} -> PA 0x{:08X}: inst=0x{:08X}", pc, pa, inst);
            
            // Decode: is it JALR?
            let opcode = inst & 0x7F;
            if opcode == 0x67 {
                let rd = (inst >> 7) & 0x1F;
                let rs1 = (inst >> 15) & 0x1F;
                let imm = (((inst >> 20) as i32) << 20) >> 20;
                eprintln!("  JALR x{}, x{}, +0x{:03X}", rd, rs1, imm as u16);
                eprintln!("  rs1 (x{}) value before step: 0x{:08X}", rs1, regs[rs1 as usize]);
                eprintln!("  target = 0x{:08X} + 0x{:08X} = 0x{:08X}", 
                    regs[rs1 as usize], imm as u32, regs[rs1 as usize].wrapping_add(imm as u32));
            } else if opcode == 0x6F {
                eprintln!("  JAL (unconditional jump)");
            } else if opcode == 0x63 {
                eprintln!("  BRANCH");
            }
            
            // Show last 20 register snapshots for context
            eprintln!("\nLast 10 steps before transition:");
            for (sn, spc, sregs) in last_regs.iter().rev().take(10) {
                eprintln!("  [{}] PC=0x{:08X} x1=0x{:08X} x5=0x{:08X} x6=0x{:08X} x10=0x{:08X}",
                    sn, spc, sregs[1], sregs[5], sregs[6], sregs[10]);
            }
            
            // Where did x6 (t1) get set to something near 0x3FFFF000?
            eprintln!("\nLooking for when T1 changed to near 0x3Fxxxxxx...");
            for (sn, spc, sregs) in last_regs.iter().rev() {
                if sregs[6] >= 0x3F000000 && sregs[6] < 0x40000000 {
                    eprintln!("  [{}] PC=0x{:08X}: T1=0x{:08X} (user-space addr!)", sn, spc, sregs[6]);
                    // What instruction set T1?
                    let vpn1_t = ((spc >> 22) & 0x3FF) as u64;
                    let vpn0_t = ((spc >> 12) & 0x3FF) as u64;
                    let page_off_t = (spc & 0xFFF) as u64;
                    let l1_pte_t = vm.bus.read_word(pg_dir_phys + vpn1_t * 4).unwrap_or(0);
                    let l1_ppn_t = ((l1_pte_t & 0xFFFF_FC00) >> 10) as u64;
                    let pa_t = if (l1_pte_t & 0xE) != 0 {
                        ((l1_ppn_t >> 10) << 22) | (vpn0_t << 12) | page_off_t
                    } else {
                        let l2_base_t = l1_ppn_t << 12;
                        let l2_pte_t = vm.bus.read_word(l2_base_t + vpn0_t * 4).unwrap_or(0);
                        let l2_ppn_t = ((l2_pte_t & 0xFFFF_FC00) >> 10) as u64;
                        (l2_ppn_t << 12) | page_off_t
                    };
                    let inst_t = vm.bus.read_word(pa_t).unwrap_or(0);
                    eprintln!("    inst=0x{:08X} at PA 0x{:08X}", inst_t, pa_t);
                }
            }
            
            break;
        }
        
        last_regs.push((step_num, pc, regs));
        if last_regs.len() > 200 { last_regs.remove(0); }
    }
}

fn handle_trap(vm: &mut RiscvVm) {
    let mcause = vm.cpu.csr.mcause;
    let cause_code = mcause & !(1u32 << 31);
    if cause_code == csr::CAUSE_ECALL_M {
        let r = vm.bus.sbi.handle_ecall(vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
            vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
            &mut vm.bus.uart, &mut vm.bus.clint);
        if let Some((a0, a1)) = r { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
        vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
    } else {
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
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        } else {
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
    }
}
