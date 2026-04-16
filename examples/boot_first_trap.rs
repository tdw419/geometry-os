/// Trace the instructions leading up to the first illegal instruction trap.
/// Records the last N PCs before the first M-mode trap.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::{RiscvVm, mmu, cpu, csr};

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut pc_history: Vec<(u64, u32, u8)> = Vec::new(); // (step, pc, priv_level)
    let history_len: usize = 200;
    let mut first_trap = true;
    let mut trap_step: u64 = 0;
    let mut trap_pc_before: u32 = 0;
    let mut trap_pc_history: Vec<(u64, u32)> = Vec::new();

    // Track CLINT state
    let mut last_mtime: u64 = 0;
    let mut timer_fired: bool = false;

    let max_steps: u64 = 200_000;

    for step in 0..max_steps {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[SHUTDOWN] at step {}", step);
            break;
        }

        // Check CLINT timer
        if let Some(mtime_lo) = vm.bus.clint.read(0x0200BFF8) {
            let mtime = mtime_lo as u64;
            if mtime != last_mtime {
                if !timer_fired {
                    timer_fired = true;
                    eprintln!("[CLINT] First mtime change at step {}: mtime_lo={}", step, mtime);
                }
                last_mtime = mtime;
            }
        }

        // Check if sitting at trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.read(csr::MCAUSE);
            let cause_code = mcause & !(1u32 << 31);
            let mstatus = vm.cpu.csr.read(csr::MSTATUS);
            let mpp = (mstatus & 0x300) >> 8;

            if first_trap {
                first_trap = false;
                trap_step = step;
                trap_pc_before = pc_history.last().map(|h| h.1).unwrap_or(0);
                eprintln!("\n=== FIRST TRAP at step {} ===", step);
                eprintln!("mcause=0x{:08X} cause_code={} MPP={}", mcause, cause_code, mpp);
                eprintln!("mepc=0x{:08X} mtval=0x{:08X}", 
                    vm.cpu.csr.read(csr::MEPC), vm.cpu.csr.read(csr::MTVAL));
                eprintln!("sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                    vm.cpu.csr.read(csr::SEPC), vm.cpu.csr.read(csr::SCAUSE),
                    vm.cpu.csr.read(csr::STVAL), vm.cpu.csr.read(csr::STVEC));
                eprintln!("satp=0x{:08X}", vm.cpu.csr.read(csr::SATP));
                eprintln!("PC before trap: 0x{:08X}", trap_pc_before);

                // Print last 30 PCs
                eprintln!("\nLast 30 PCs before trap:");
                let start = pc_history.len().saturating_sub(30);
                for i in start..pc_history.len() {
                    let (s, pc, priv_lvl) = pc_history[i];
                    let priv_name = match priv_lvl {
                        3 => "M",
                        1 => "S",
                        0 => "U",
                        _ => "?",
                    };
                    eprintln!("  step={}: PC=0x{:08X} priv={}", s, pc, priv_name);
                }

                // Try to decode the instruction at trap_pc_before
                eprintln!("\nDecoding instruction at PC=0x{:08X}:", trap_pc_before);
                let satp = vm.cpu.csr.read(csr::SATP);
                if mmu::satp_mode_enabled(satp) {
                    match mmu::translate(trap_pc_before, mmu::AccessType::Fetch,
                        cpu::Privilege::Supervisor, 
                        (mstatus >> 18) & 1 != 0,
                        (mstatus >> 19) & 1 != 0,
                        satp, &mut vm.bus, &mut vm.cpu.tlb) {
                        mmu::TranslateResult::Ok(pa) => {
                            if let Ok(word) = vm.bus.read_word(pa) {
                                eprintln!("  PA=0x{:08X} word=0x{:08X}", pa, word);
                            }
                        }
                        r => eprintln!("  Translate result: {:?}", r),
                    }
                } else if let Ok(word) = vm.bus.read_word(trap_pc_before as u64) {
                    eprintln!("  word=0x{:08X}", word);
                }

                // Print key registers
                eprintln!("\nKey registers at trap:");
                eprintln!("  ra (x1) = 0x{:08X}", vm.cpu.x[1]);
                eprintln!("  sp (x2) = 0x{:08X}", vm.cpu.x[2]);
                eprintln!("  gp (x3) = 0x{:08X}", vm.cpu.x[3]);
                eprintln!("  tp (x4) = 0x{:08X}", vm.cpu.x[4]);
                eprintln!("  t0 (x5) = 0x{:08X}", vm.cpu.x[5]);
                eprintln!("  a0 (x10) = 0x{:08X}", vm.cpu.x[10]);
                eprintln!("  a1 (x11) = 0x{:08X}", vm.cpu.x[11]);

                break; // Stop after first trap analysis
            }

            // Handle the trap (same as boot_linux)
            if cause_code == 9 {
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
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.read(csr::STVEC) & !0x3u32;
                if stvec != 0 && cause_code != 9 {
                    vm.cpu.csr.write(csr::SEPC, vm.cpu.csr.read(csr::MEPC));
                    vm.cpu.csr.write(csr::SCAUSE, mcause);
                    vm.cpu.csr.write(csr::STVAL, vm.cpu.csr.read(csr::MTVAL));
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Record PC before step
        let priv_byte = match vm.cpu.privilege {
            cpu::Privilege::Machine => 3u8,
            cpu::Privilege::Supervisor => 1u8,
            cpu::Privilege::User => 0u8,
        };
        pc_history.push((step, vm.cpu.pc, priv_byte));
        if pc_history.len() > history_len {
            pc_history.remove(0);
        }

        vm.step();

        // Check for privilege changes
        if step > 0 {
            let prev_priv = pc_history[pc_history.len() - 1].2;
            let curr_priv = match vm.cpu.privilege {
                cpu::Privilege::Machine => 3u8,
                cpu::Privilege::Supervisor => 1u8,
                cpu::Privilege::User => 0u8,
            };
            if prev_priv != curr_priv {
                eprintln!("[PRIV_CHANGE] step={}: {:?} -> {:?} PC=0x{:08X}",
                    step, 
                    match prev_priv { 3 => "M", 1 => "S", 0 => "U", _ => "?" },
                    match curr_priv { 3 => "M", 1 => "S", 0 => "U", _ => "?" },
                    vm.cpu.pc);
            }
        }
    }

    if first_trap {
        eprintln!("No traps in {} steps. Final PC=0x{:08X}", max_steps, vm.cpu.pc);
    }

    // UART output
    let mut uart_out = Vec::new();
    loop {
        match vm.bus.uart.read_byte(0) {
            0 => break,
            b => uart_out.push(b),
        }
    }
    if !uart_out.is_empty() {
        let s = String::from_utf8_lossy(&uart_out);
        eprintln!("UART output ({} bytes): {}", uart_out.len(), s);
    } else {
        eprintln!("No UART output");
    }
}
