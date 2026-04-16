use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    
    let (mut vm, fw_addr, _entry, _dtb) = 
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            512,
            bootargs,
        ).expect("boot setup failed");
    
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 300_000;
    
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut satp_change_count: u64 = 0;
    
    // Dump page table BEFORE setup_vm (at a PC just before 0x10EE)
    let mut pre_dump_done = false;
    
    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        
        // Dump page tables BEFORE injection (at PC=0x10EC, just before 0x10EE)
        if !pre_dump_done && vm.cpu.pc == 0x10EC {
            pre_dump_done = true;
            eprintln!("[diag] PRE-INJECTION dump at count={}, PC=0x{:08X}", count, vm.cpu.pc);
            dump_page_table(&mut vm, 0x0148_4000, "trampoline (0x1484000)");
            dump_page_table(&mut vm, 0x0080_2000, "early_pg_dir (0x802000)");
        }
        
        // Inject identity mappings at PC=0x10EE
        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            eprintln!("[diag] POST-setup_vm, PRE-injection dump:");
            dump_page_table(&mut vm, 0x0148_4000, "trampoline (0x1484000)");
            dump_page_table(&mut vm, 0x0080_2000, "early_pg_dir (0x802000)");
            
            // Now inject
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
            
            eprintln!("[diag] POST-injection dump:");
            dump_page_table(&mut vm, 0x0148_4000, "trampoline (0x1484000)");
            dump_page_table(&mut vm, 0x0080_2000, "early_pg_dir (0x802000)");
        }
        
        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            satp_change_count += 1;
            eprintln!(
                "[diag] SATP #{} changed to 0x{:08X} at count={}, PC=0x{:08X}",
                satp_change_count, cur_satp, count, vm.cpu.pc
            );
            
            // Dump page table at the SATP root
            let pt_ppn = cur_satp & 0x003F_FFFF;
            let pt_base = (pt_ppn as u64) << 12;
            eprintln!("[diag] Page table root at PA 0x{:08X}:", pt_base);
            dump_page_table(&mut vm, pt_base, &format!("SATP root (0x{:08X})", pt_base));
        }
        last_satp = cur_satp;
        
        // Forward traps
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
            
            if cause_code != 11 {
                // Forward to S-mode if from S/U-mode
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5))
                            | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5))
                            | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        
                        if cause_code == 2 && satp_change_count < 10 {
                            eprintln!(
                                "[diag] ILLEGAL #{} at count={}: mepc=0x{:08X} sepc=0x{:08X} stvec=0x{:08X} satp=0x{:08X}",
                                satp_change_count, count, vm.cpu.csr.mepc, vm.cpu.csr.sepc, stvec, vm.cpu.csr.satp
                            );
                        }
                        
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            }
            
            // ECALL_M: SBI call
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
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            count += 1;
            continue;
        }
        
        vm.step();
        count += 1;
    }
    
    eprintln!(
        "\n[diag] Final: PC=0x{:08X} priv={:?} satp=0x{:08X} instructions={}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp, count
    );
}

fn dump_page_table(vm: &mut geometry_os::riscv::RiscvVm, base: u64, label: &str) {
    eprintln!("[diag]   {}:", label);
    // Dump identity range
    for l1_idx in 0..=3 {
        let pte = vm.bus.read_word(base + (l1_idx as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let ppn_hi = pte >> 21;
            eprintln!(
                "[diag]     L1[{}] = 0x{:08X} -> PPN[31:21]={} maps VA 0x{:08X}-0x{:08X} -> PA 0x{:08X}",
                l1_idx, pte, ppn_hi,
                l1_idx << 22, ((l1_idx + 1) << 22) - 1,
                ppn_hi << 21
            );
        }
    }
    // Dump 0xC0 range (L1[768+])
    for l1_idx in 768..=775 {
        let pte = vm.bus.read_word(base + (l1_idx as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let ppn_hi = pte >> 21;
            let va_start = l1_idx << 22;
            let pa_start = ppn_hi << 21;
            let is_leaf = (pte & 0xE) != 0;
            eprintln!(
                "[diag]     L1[{}] = 0x{:08X} -> PPN[31:21]={} {} VA 0x{:08X} -> PA 0x{:08X}",
                l1_idx, pte, ppn_hi,
                if is_leaf { "megapage" } else { "non-leaf" },
                va_start, pa_start
            );
        }
    }
}
