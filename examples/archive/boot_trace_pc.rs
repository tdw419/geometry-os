use std::collections::HashMap;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .expect("setup");

    let fw_addr_u32 = fw_addr as u32;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut count: u64 = 0;
    let mut ecall_count: u64 = 0;
    let mut trap_count: u64 = 0;
    let mut pc_history: Vec<(u64, u32)> = Vec::new(); // (count, PC)
    let mut first_low_pc: Option<(u64, u32)> = None; // First time PC < 0x1000 after boot

    let max = 5_000_000u64;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Identity mapping injection (same as boot_linux)
        {
            let cur_satp = vm.cpu.csr.satp;
            if !trampoline_patched
                && vm.cpu.pc == 0x10EE
                && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
                && cur_satp == 0
            {
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                for &idx in l1_entries {
                    let pte = identity_pte | (idx << 20);
                    vm.bus
                        .write_word(0x0148_4000u64 + (idx * 4) as u64, pte)
                        .ok();
                    vm.bus
                        .write_word(0x0080_2000u64 + (idx * 4) as u64, pte)
                        .ok();
                }
                trampoline_patched = true;
                eprintln!("[boot] Phase 1 injected at count={}", count);
            }
            if trampoline_patched && cur_satp != last_satp && cur_satp != 0 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir = (ppn as u64) * 4096;
                let l1_0 = vm.bus.read_word(pg_dir).unwrap_or(0);
                if (l1_0 & 0xCF) != 0xCF {
                    let identity_pte: u32 = 0x0000_00CF;
                    let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                    for &idx in l1_entries {
                        let pte = identity_pte | (idx << 20);
                        vm.bus.write_word(pg_dir + (idx * 4) as u64, pte).ok();
                    }
                    vm.cpu.tlb.flush_all();
                    eprintln!(
                        "[boot] Phase 2 injected into 0x{:08X} at count={}",
                        pg_dir, count
                    );
                }
            }
            last_satp = cur_satp;
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            trap_count += 1;
            if cause_code == 9 {
                // ECALL_S
                ecall_count += 1;
                let a7 = vm.cpu.x[17];
                let a0 = vm.cpu.x[10];
                if ecall_count <= 5 {
                    eprintln!(
                        "[boot] SBI #{}: a7={} a0=0x{:02X} at count={}",
                        ecall_count, a7, a0, count
                    );
                }
            }
            if trap_count <= 5 {
                eprintln!(
                    "[boot] M-mode trap #{}: cause={} mepc=0x{:08X} at count={}",
                    trap_count, cause_code, vm.cpu.csr.mepc, count
                );
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Track PC
        if count % 10000 == 0 {
            pc_history.push((count, vm.cpu.pc));
        }

        // Detect when PC drops to low addresses after MMU was enabled
        if vm.cpu.csr.satp != 0 && vm.cpu.pc < 0x1000 && first_low_pc.is_none() && count > 200000 {
            first_low_pc = Some((count, vm.cpu.pc));
            eprintln!(
                "[!] FIRST LOW PC: count={} PC=0x{:08X} satp=0x{:08X} priv={:?}",
                count, vm.cpu.pc, vm.cpu.csr.satp, vm.cpu.privilege
            );
            // Read instruction at this PC via bus (physical)
            let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            eprintln!("[!] Instruction at PA 0x{:08X}: 0x{:08X}", vm.cpu.pc, inst);
            // Also check what the MMU translates this VA to
            // Can't call translate_va (private), just note the VA
            eprintln!(
                "[!] MMU: VA 0x{:08X} (MMU active, SATP=0x{:08X})",
                vm.cpu.pc, vm.cpu.csr.satp
            );
            // Dump the L1 page table entry
            if vm.cpu.csr.satp != 0 {
                let ppn = vm.cpu.csr.satp & 0x3FFFFF;
                let pg_dir = (ppn as u64) * 4096;
                let vpn1 = (vm.cpu.pc >> 22) & 0x3FF;
                let l1_entry = vm.bus.read_word(pg_dir + (vpn1 * 4) as u64).unwrap_or(0);
                eprintln!(
                    "[!] L1[{}] at PA 0x{:08X} = 0x{:08X}",
                    vpn1,
                    pg_dir + (vpn1 * 4) as u64,
                    l1_entry
                );
            }
        }

        let result = vm.step();
        if matches!(result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        count += 1;
    }

    eprintln!("\n=== SUMMARY ===");
    eprintln!("Instructions: {}", count);
    eprintln!("ECALL (SBI): {}", ecall_count);
    eprintln!("M-mode traps: {}", trap_count);
    eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);
    eprintln!("Final SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("Final priv: {:?}", vm.cpu.privilege);
    eprintln!("CPU ecall_count: {}", vm.cpu.ecall_count);
    eprintln!("UART tx: {} bytes", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        eprintln!("[UART] {}", String::from_utf8_lossy(&vm.bus.uart.tx_buf));
    }

    if let Some((c, pc)) = first_low_pc {
        eprintln!("First low PC at count={}, PC=0x{:08X}", c, pc);
    }

    // Show last 20 PC samples
    eprintln!("\nLast 20 PC samples:");
    for (c, pc) in pc_history.iter().rev().take(20).rev() {
        let marker = if *pc < 0x1000 { " *** LOW" } else { "" };
        eprintln!("  count={}: PC=0x{:08X}{}", c, pc, marker);
    }
}
