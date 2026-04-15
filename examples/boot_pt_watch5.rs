fn main() {
    // L1[772] maps VA 0xC1000000-0xC1200000 -> PA 0x1000000-0x1200000
    // The page table at PA 0x1002000 is at VA 0xC1002000
    // Something is writing through this mapping to corrupt the PT.
    //
    // Strategy: watch the store address on each step. If a store targets
    // VA 0xC1000000-0xC1200000, log it.
    // Since we can't easily intercept stores in the step loop,
    // we'll check the bus for writes to PA 0x1002000-0x1003000.
    //
    // Alternative: check L1[770] every 100 instructions and when it changes,
    // back up 200 instructions and check what happened.
    
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
    let max_instr = 17_000_000u64;
    let mut count: u64 = 0;

    // Watch L1[770] every 100 instructions
    let start_watch: u64 = 16_900_000;
    let check_interval: u64 = 100;
    let pt_base = 0x1002000u64;
    let watch_addr = pt_base + 770 * 4; // 0x1002C08
    
    // Ring buffer of (count, pc) for recent instructions
    let mut history: Vec<(u64, u32)> = Vec::with_capacity(500);
    let mut history_idx: usize = 0;
    let mut history_len: usize = 0;
    let history_cap = 500;
    
    let mut last_pte: u32 = vm.bus.read_word(watch_addr).unwrap_or(0);
    let mut found = false;

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

        // Record in history ring buffer
        if count >= start_watch {
            if history_len < history_cap {
                history.push((count, vm.cpu.pc));
                history_len += 1;
            } else {
                history[history_idx] = (count, vm.cpu.pc);
            }
            history_idx = (history_idx + 1) % history_cap;
        }

        let _ = vm.step();
        count += 1;

        // Check PTE
        if count >= start_watch && count % check_interval == 0 {
            let pte = vm.bus.read_word(watch_addr).unwrap_or(0);
            if pte != last_pte {
                eprintln!("[!] count={} L1[770] CHANGED: 0x{:08X} -> 0x{:08X}", count, last_pte, pte);
                eprintln!("[!] Current PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
                eprintln!("[!] Registers: a0=0x{:08X} a1=0x{:08X} a2=0x{:08X} a3=0x{:08X}",
                    vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]);
                eprintln!("[!] sp=0x{:08X} ra=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1]);
                
                // Show last 50 unique PCs from history
                eprintln!("[!] Last {} instruction PCs:", history_len.min(50));
                let start = if history_len >= history_cap { history_idx } else { 0 };
                for k in 0..history_len.min(50) {
                    let idx = (start + k) % history_len.min(history_cap);
                    let (c, pc) = history[idx];
                    eprintln!("    count={}: PC=0x{:08X}", c, pc);
                }
                
                // Also check L1[0] to see the pattern
                let l1_0 = vm.bus.read_word(pt_base).unwrap_or(0);
                let l1_1 = vm.bus.read_word(pt_base + 4).unwrap_or(0);
                let l1_2 = vm.bus.read_word(pt_base + 8).unwrap_or(0);
                eprintln!("[!] L1[0]=0x{:08X} L1[1]=0x{:08X} L1[2]=0x{:08X}", l1_0, l1_1, l1_2);
                
                found = true;
                break;
            }
            last_pte = pte;
        }
    }
    
    if !found {
        eprintln!("L1[770] unchanged");
    }
}
