use geometry_os::riscv::cpu::StepResult;
/// Diagnose the illegal instruction loop during Linux boot.
/// The boot gets to ~177K instructions then hits illegal instructions at mepc=0x00000004.
/// This traces the instructions leading up to the first illegal instruction.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 250_000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut trap_log: Vec<String> = Vec::new();
    let mut prev_pc: u32 = 0;
    let mut pc_trace: Vec<(u64, u32, u32)> = Vec::new(); // (count, pc, insn)

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Trampoline patching (same as boot_linux)
        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
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
            eprintln!("[diag] Trampoline patched at count={}", count);
        }

        // TLB pre-warming
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            let new_ppn = (cur_satp & 0x3FFFFF) as u64;
            let pt_base = new_ppn << 12;
            let asid = (cur_satp >> 22) & 0x1FF;

            for l1_idx in 0..1024u64 {
                let l1_addr = pt_base + l1_idx * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                if l1_pte == 0 {
                    continue;
                }
                let is_leaf = (l1_pte & 0xE) != 0;
                if !is_leaf {
                    continue;
                }

                let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u32;
                let flags = (l1_pte & 0xFF) as u32;

                for vpn0 in 0..512u32 {
                    let vpn_combined = ((l1_idx as u32) << 10) | vpn0;
                    let eff_ppn = (ppn_hi << 10) | vpn0;
                    vm.cpu.tlb.insert(vpn_combined, asid as u16, eff_ppn, flags);
                }
            }
            eprintln!(
                "[diag] TLB warmed SATP=0x{:08X} at count={}",
                cur_satp, count
            );
        }
        last_satp = cur_satp;

        // Trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code != 11 {
                // not ECALL_M
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;

                if mpp != 3 {
                    // Forward to S-mode
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();

                        if trap_log.len() < 5 {
                            trap_log.push(format!(
                                "count={} mepc=0x{:08X} cause={} stvec=0x{:08X} mpp={}",
                                count, vm.cpu.csr.sepc, cause_code, stvec, mpp
                            ));
                        }
                        count += 1;
                        continue;
                    }
                }

                // MPP=3: skip
                if cause_code == 2 && trap_log.len() < 5 {
                    let mepc_val = vm.cpu.csr.mepc;
                    let insn = vm.bus.read_word(mepc_val as u64).unwrap_or(0);
                    let raw_bytes = vm.bus.read_byte(mepc_val as u64).unwrap_or(0) as u16
                        | ((vm.bus.read_byte(mepc_val as u64 + 1).unwrap_or(0) as u16) << 8);
                    trap_log.push(format!(
                        "M-mode illegal: count={} mepc=0x{:08X} insn=0x{:08X} raw16=0x{:04X} priv=M mpp={}",
                        count, mepc_val, insn, raw_bytes, mpp
                    ));
                }
            } else {
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
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Track PC transitions in a ring buffer (last 30 steps)
        pc_trace.push((
            count,
            vm.cpu.pc,
            vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0),
        ));
        if pc_trace.len() > 30 {
            pc_trace.remove(0);
        }

        let step_result = vm.step();
        prev_pc = vm.cpu.pc;
        count += 1;

        // Detect infinite illegal instruction loop
        if count > 178000 && count % 1000 == 0 {
            // Check if we're stuck
            let unique_pcs: std::collections::HashSet<u32> =
                pc_trace.iter().map(|(_, pc, _)| *pc).collect();
            if unique_pcs.len() <= 3 {
                eprintln!(
                    "[diag] Stuck at count={}. Unique PCs: {:?}",
                    count, unique_pcs
                );
                eprintln!("[diag] Last 20 PC transitions:");
                for (c, pc, insn) in &pc_trace[pc_trace.len().saturating_sub(20)..] {
                    eprintln!("  count={} PC=0x{:08X} insn=0x{:08X}", c, pc, insn);
                }
                break;
            }
        }
    }

    eprintln!("\n=== DIAGNOSTIC SUMMARY ===");
    eprintln!("Instructions executed: {}", count);
    eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);
    eprintln!("Final privilege: {:?}", vm.cpu.privilege);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("STVEC: 0x{:08X}", vm.cpu.csr.stvec);
    eprintln!("MEPC: 0x{:08X}", vm.cpu.csr.mepc);
    eprintln!("MCAUSE: 0x{:08X}", vm.cpu.csr.mcause);
    eprintln!(
        "SBI console output: {} chars",
        vm.bus.sbi.console_output.len()
    );

    eprintln!("\nTrap log (first 5):");
    for t in &trap_log {
        eprintln!("  {}", t);
    }

    eprintln!("\nLast 30 PC transitions:");
    for (c, pc, insn) in &pc_trace {
        eprintln!("  count={} PC=0x{:08X} insn=0x{:08X}", c, pc, insn);
    }

    // Check what instruction is at the final PC
    let final_insn = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
    let final_raw16 = vm.bus.read_byte(vm.cpu.pc as u64).unwrap_or(0) as u16
        | ((vm.bus.read_byte(vm.cpu.pc as u64 + 1).unwrap_or(0) as u16) << 8);
    eprintln!(
        "\nFinal instruction: 0x{:08X} (raw16=0x{:04X})",
        final_insn, final_raw16
    );

    // Check if this is a compressed instruction
    let bits01 = final_raw16 & 0x3;
    let funct3 = (final_raw16 >> 13) & 0x7;
    eprintln!("  bits[1:0] = 0b{:02b}, funct3 = 0b{:03b}", bits01, funct3);

    // Decode the compressed instruction to see what it should be
    use geometry_os::riscv::decode::{decode, decode_c, is_compressed};
    if is_compressed(final_raw16) {
        let op = decode_c(final_raw16);
        eprintln!("  Compressed decode: {:?}", op);
    } else {
        let op = decode(final_insn);
        eprintln!("  32-bit decode: {:?}", op);
    }
}
