/// Diagnostic: check kernel_map struct and page tables after setup_vm()
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, "loglevel=0 quiet")
            .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: Vec<(u64, u32, u32)> = Vec::new();

    // Run until we see 3 SATP changes (setup_vm creates page tables)
    let max_steps: u64 = 500_000;
    for count in 0..max_steps {
        // Check for trap at fw_addr
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                // ECALL_M = SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code == 9 {
                // ECALL_S = SBI call from S-mode
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes.push((count, last_satp, cur_satp));
            last_satp = cur_satp;
            if satp_changes.len() >= 3 {
                break;
            }
        }

        let _ = vm.step();
    }

    println!("=== SATP Changes ===");
    for (count, old, new) in &satp_changes {
        println!("  count={}: 0x{:08X} -> 0x{:08X}", count, old, new);
    }

    // Dump kernel_map struct at PA 0x00C79E90
    println!("\n=== kernel_map struct at PA 0x00C79E90 ===");
    let km_base: u64 = 0x00C79E90;
    let page_offset = vm.bus.read_word(km_base).unwrap_or(0);
    let virt_addr = vm.bus.read_word(km_base + 4).unwrap_or(0);
    let virt_offset = vm.bus.read_word(km_base + 8).unwrap_or(0);
    let phys_addr = vm.bus.read_word(km_base + 12).unwrap_or(0);
    let size = vm.bus.read_word(km_base + 16).unwrap_or(0);
    let va_pa_offset = vm.bus.read_word(km_base + 20).unwrap_or(0);
    let va_kernel_pa_offset = vm.bus.read_word(km_base + 24).unwrap_or(0);
    println!("  page_offset     = 0x{:08X}", page_offset);
    println!("  virt_addr       = 0x{:08X}", virt_addr);
    println!("  virt_offset     = 0x{:08X}", virt_offset);
    println!(
        "  phys_addr       = 0x{:08X} (should be 0x00000000)",
        phys_addr
    );
    println!("  size            = 0x{:08X}", size);
    println!(
        "  va_pa_offset    = 0x{:08X} (should be 0xC0000000)",
        va_pa_offset
    );
    println!("  va_kernel_pa_offset = 0x{:08X}", va_kernel_pa_offset);

    // Dump L1 page table entries for the current SATP
    let cur_satp = vm.cpu.csr.satp;
    let ppn = cur_satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) * 4096;
    println!(
        "\n=== L1 page table at PA 0x{:08X} (SATP=0x{:08X}) ===",
        pg_dir_phys, cur_satp
    );

    // Check kernel VA range entries (L1[768..780])
    for i in 768..780 {
        let pte = vm.bus.read_word(pg_dir_phys + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let pfn = (pte >> 10) & 0xFFFFF;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let v = pte & 1;
            let va_start = (i as u64) * 0x400000;
            let pa_start = (pfn as u64) * 0x1000;
            println!(
                "  L1[{}] VA 0x{:08X}: PTE=0x{:08X} PA=0x{:08X} RWX={}{}{} V={}",
                i, va_start, pte, pa_start, r, w, x, v
            );
        }
    }

    // Check low address entries (L1[0..10])
    println!("\n=== Low address L1 entries ===");
    for i in 0..16 {
        let pte = vm.bus.read_word(pg_dir_phys + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let pfn = (pte >> 10) & 0xFFFFF;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let v = pte & 1;
            let va_start = (i as u64) * 0x400000;
            let pa_start = (pfn as u64) * 0x1000;
            println!(
                "  L1[{}] VA 0x{:08X}: PTE=0x{:08X} PA=0x{:08X} RWX={}{}{} V={}",
                i, va_start, pte, pa_start, r, w, x, v
            );
        }
    }

    // Check what's at sepc=0x3FFFF000 - what L1 entry would map it?
    let fault_va = 0x3FFFF000u64;
    let fault_l1_idx = (fault_va >> 22) & 0x3FF;
    let fault_pte = vm
        .bus
        .read_word(pg_dir_phys + (fault_l1_idx as u64) * 4)
        .unwrap_or(0);
    println!("\n=== Fault address 0x{:08X} ===", fault_va);
    println!(
        "  L1 index: {} PTE: 0x{:08X} ({})",
        fault_l1_idx,
        fault_pte,
        if fault_pte == 0 {
            "NOT MAPPED"
        } else {
            "mapped"
        }
    );

    // Run more steps to see where the fault happens
    println!("\n=== Running 50K more steps ===");
    let mut fault_pc: u32 = 0;
    let mut fault_sepc: u32 = 0;
    let mut found_fault = false;
    for count in 0..50_000 {
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
                // Skip mepc+4 (will be handled by MRET)
            } else if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();
        let is_fault = format!("{:?}", step_result).contains("FetchFault");
        if is_fault {
            if !found_fault {
                println!(
                    "  First fetch fault at count+{}: PC=0x{:08X} stval=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.stval
                );
                fault_pc = vm.cpu.pc;
                fault_sepc = vm.cpu.csr.stval;
                found_fault = true;

                // Dump registers at first fault
                println!(
                    "  Registers: SP=0x{:08X} RA=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                    vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3], vm.cpu.x[4]
                );
                println!(
                    "  T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} A0=0x{:08X} A1=0x{:08X}",
                    vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[10], vm.cpu.x[11]
                );

                // Check what's on the stack (SP points to current SP, RA should be above)
                let sp = vm.cpu.x[2] as u64;
                for offset in [
                    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76,
                    80, 84, 88, 92,
                ] {
                    if let Ok(val) = vm.bus.read_word(sp + offset as u64) {
                        if val == 0x3FFFF000 || val == fault_sepc as u32 || val == 0xC0000000 {
                            println!("  ** SP+{} = 0x{:08X} <-- MATCH", offset, val);
                        }
                    }
                }
                break;
            }
        }
    }
}
