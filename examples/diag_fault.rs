// Diagnose the page fault at 0xC006ADD8
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::csr;
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max = 500_000u64;
    let mut count = 0u64;
    let mut fault_count = 0u64;
    let mut last_fault_va = 0u32;
    let mut fetch_fault_count = 0u64;

    while count < max {
        // Firmware handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_S {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
            let result = vm.bus.sbi.handle_ecall(
                vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                &mut vm.bus.uart, &mut vm.bus.clint,
            );
            if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }

        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        match step_result {
            StepResult::FetchFault => {
                fetch_fault_count += 1;
                if fetch_fault_count <= 5 {
                    eprintln!("[{}] FetchFault at PC=0x{:08X} priv={:?}", count, pc_before, vm.cpu.privilege);
                    eprintln!("  scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
                }
                fault_count += 1;
                last_fault_va = pc_before;
            }
            StepResult::LoadFault => {
                fault_count += 1;
                if fault_count <= 5 {
                    eprintln!("[{}] LoadFault at PC=0x{:08X}", count, pc_before);
                }
            }
            StepResult::StoreFault => {
                fault_count += 1;
                if fault_count <= 5 {
                    eprintln!("[{}] StoreFault at PC=0x{:08X}", count, pc_before);
                }
            }
            _ => {}
        }

        // After first few faults, dump page table state
        if fault_count == 3 {
            eprintln!("\n=== State after 3 faults ===");
            eprintln!("satp=0x{:08X} (mode={} asid={} ppn=0x{:05X})",
                vm.cpu.csr.satp,
                (vm.cpu.csr.satp >> 31) & 1,
                (vm.cpu.csr.satp >> 22) & 0x1FF,
                vm.cpu.csr.satp & 0x3FFFF);
            eprintln!("sstatus=0x{:08X}", vm.cpu.csr.mstatus);
            
            // Dump the faulting VA's page table walk
            let fault_va = vm.cpu.csr.stval;
            let satp = vm.cpu.csr.satp;
            let root_ppn = satp & 0x3FFFF;
            let root_addr = (root_ppn as u64) << 12;
            let vpn1 = (fault_va >> 22) & 0x3FF;
            let vpn0 = (fault_va >> 12) & 0x3FF;
            let l1_addr = root_addr | ((vpn1 as u64) << 2);
            eprintln!("\nFaulting VA 0x{:08X}: vpn1=0x{:03X} vpn0=0x{:03X}", fault_va, vpn1, vpn0);
            eprintln!("Root PT at PA 0x{:08X}, L1 entry at PA 0x{:08X}", root_addr, l1_addr);
            
            if let Ok(l1_pte) = vm.bus.read_word(l1_addr) {
                eprintln!("L1 PTE = 0x{:08X} (V={} R={} W={} X={} U={} G={} A={} D={})",
                    l1_pte,
                    (l1_pte >> 0) & 1, (l1_pte >> 1) & 1, (l1_pte >> 2) & 1,
                    (l1_pte >> 3) & 1, (l1_pte >> 4) & 1, (l1_pte >> 5) & 1,
                    (l1_pte >> 6) & 1, (l1_pte >> 7) & 1);
                let ppn = (l1_pte >> 10) & 0x3FFFFF;
                let is_leaf = (l1_pte & 0xE) != 0;
                eprintln!("  PPN=0x{:05X} is_leaf={}", ppn, is_leaf);
                
                if !is_leaf && (l1_pte & 1) != 0 {
                    let fixed_ppn = if ppn >= 0xC0000 { ppn - 0xC0000 } else { ppn };
                    let l2_base = (fixed_ppn as u64) << 12;
                    let l2_addr = l2_base | ((vpn0 as u64) << 2);
                    eprintln!("L2 base at PA 0x{:08X}, L2 entry at PA 0x{:08X}", l2_base, l2_addr);
                    if let Ok(l2_pte) = vm.bus.read_word(l2_addr) {
                        eprintln!("L2 PTE = 0x{:08X} (V={} R={} W={} X={} U={} G={} A={} D={})",
                            l2_pte,
                            (l2_pte >> 0) & 1, (l2_pte >> 1) & 1, (l2_pte >> 2) & 1,
                            (l2_pte >> 3) & 1, (l2_pte >> 4) & 1, (l2_pte >> 5) & 1,
                            (l2_pte >> 6) & 1, (l2_pte >> 7) & 1);
                        let ppn2 = (l2_pte >> 10) & 0x3FFFFF;
                        eprintln!("  PPN=0x{:05X} fixed=0x{:05X}", ppn2, 
                            if ppn2 >= 0xC0000 { ppn2 - 0xC0000 } else { ppn2 });
                    } else {
                        eprintln!("L2 PTE read FAILED");
                    }
                }
            } else {
                eprintln!("L1 PTE read FAILED");
            }
            
            // Dump last 5 MMU events
            let log = &vm.bus.mmu_log;
            let recent = log.len().saturating_sub(5);
            eprintln!("\nLast {} MMU events:", log.len() - recent);
            for i in recent..log.len() {
                eprintln!("  {:?}", log[i]);
            }
            
            // Check UART output
            let uart_bytes: Vec<u8> = vm.bus.uart.tx_buf.iter().copied().collect();
            if !uart_bytes.is_empty() {
                let s = String::from_utf8_lossy(&uart_bytes);
                eprintln!("\nUART output so far ({} bytes):", uart_bytes.len());
                eprintln!("{}", s);
            }
            
            break;
        }
    }
    
    eprintln!("\nTotal faults: {} fetch={}, out of {} instructions", 
        fault_count, fetch_fault_count, count);
}
