/// Investigate the spin loop at 0xC08E5D6A.
/// Disassemble the area and watch PC changes at fine granularity.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, bootargs
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;

    // Disassemble around the spin address 0xC08E5D6A
    eprintln!("=== Disassembly around 0xC08E5D6A ===");
    let spin_base = 0xC08E5D40u64;
    for i in 0..20 {
        let addr = spin_base + (i * 4) as u64;
        if let Ok(word) = vm.bus.read_word(addr) {
            let op = geometry_os::riscv::decode::decode(word);
            eprintln!("  0x{:08X}: 0x{:08X}  {:?}", addr, word, op);
        }
    }

    // Disassemble the trap handler at 0xC08EFF1C
    eprintln!("\n=== Disassembly around 0xC08EFF1C (trap handler) ===");
    let handler_base = 0xC08EFF00u64;
    for i in 0..30 {
        let addr = handler_base + (i * 4) as u64;
        if let Ok(word) = vm.bus.read_word(addr) {
            let op = geometry_os::riscv::decode::decode(word);
            eprintln!("  0x{:08X}: 0x{:08X}  {:?}", addr, word, op);
        }
    }

    // Now run and track PC movement at fine granularity
    eprintln!("\n=== Running with PC tracking ===");
    let mut count: u64 = 0;
    let max_instructions: u64 = 20_000_000;
    let mut last_pc: u32 = 0;
    let mut same_pc_count: u64 = 0;
    let mut pc_transitions: Vec<(u64, u32, u64)> = Vec::new(); // (count, old_pc, duration)
    let mut sbi_call_count: u64 = 0;
    let mut forward_count: u64 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[track] SBI shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == csr::CAUSE_ECALL_S {
                sbi_call_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
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
                        forward_count += 1;
                        count += 1;
                        continue;
                    }
                }
            } else {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.step();

        // Track PC transitions
        let cur_pc = vm.cpu.pc;
        if cur_pc != last_pc {
            if same_pc_count > 100 && pc_transitions.len() < 200 {
                pc_transitions.push((count - same_pc_count, last_pc, same_pc_count));
            }
            if pc_transitions.len() < 200 {
                pc_transitions.push((count, cur_pc, 1));
            }
            last_pc = cur_pc;
            same_pc_count = 0;
        } else {
            same_pc_count += 1;
        }

        count += 1;

        // Report every 1M instructions
        if count % 1_000_000 == 0 {
            eprintln!("[track] count={:7}M PC=0x{:08X} same_pc={} ecall={} sbi={} fwd={}",
                count / 1_000_000, vm.cpu.pc, same_pc_count, vm.cpu.ecall_count, sbi_call_count, forward_count);
        }
    }

    // Print PC transitions (major ones)
    eprintln!("\n=== Major PC transitions (stayed >100 instructions) ===");
    for (cnt, pc, dur) in &pc_transitions {
        if *dur > 100 {
            eprintln!("  count={:8}: PC=0x{:08X} stayed {} instructions", cnt, pc, dur);
        }
    }

    // Print final state
    eprintln!("\n=== Final State ===");
    eprintln!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SATP=0x{:08X}", vm.cpu.csr.read(csr::SATP));
    eprintln!("SSTATUS=0x{:08X}", vm.cpu.csr.read(csr::SSTATUS));
    eprintln!("STVEC=0x{:08X}", vm.cpu.csr.read(csr::STVEC));
    eprintln!("ECALL_count={} SBI_calls={} forwards={}", vm.cpu.ecall_count, sbi_call_count, forward_count);
    eprintln!("UART tx: {} chars", vm.bus.uart.tx_buf.len());
    eprintln!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("SBI output: {}", &s[..s.len().min(500)]);
    }
}
