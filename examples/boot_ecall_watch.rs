/// Diagnostic: watch what the kernel is doing in S-mode after boot transition.
/// Focus on ECALLs, UART output, and instruction patterns.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 5_000_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: u64 = 0;
    let mut trap_count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_ecall_count: u64 = 0;
    let mut last_report: u64 = 0;

    // Track unique PCs to detect spin loops
    let mut pc_set: std::collections::HashSet<u32> = std::collections::HashSet::new();

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            println!("[diag] Shutdown requested at count={}", count);
            break;
        }

        // Handle M-mode trap forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 11 {
                // ECALL_M -> SBI call
                sbi_count += 1;
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
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                if sbi_count <= 20 {
                    println!("[diag] M-mode SBI #{} at count={} PC=0x{:08X} a7=0x{:X} a6=0x{:X} a0=0x{:X}",
                        sbi_count, count, vm.cpu.csr.mepc, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]);
                }
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        trap_count += 1;
                        if trap_count <= 20 {
                            println!("[diag] Forward #{} at count={}: cause={} mepc=0x{:08X} -> stvec=0x{:08X}",
                                trap_count, count, cause_code, vm.cpu.csr.mepc, stvec);
                        }
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _step_result = vm.step();

        // SATP change detection
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            println!(
                "[diag] SATP change #{}: 0x{:08X} -> 0x{:08X} at count={} PC=0x{:08X}",
                satp_changes, last_satp, cur_satp, count, vm.cpu.pc
            );
            last_satp = cur_satp;
        }

        // Detect ECALLs from CPU's internal counter
        if vm.cpu.ecall_count != last_ecall_count {
            let new_ecalls = vm.cpu.ecall_count - last_ecall_count;
            last_ecall_count = vm.cpu.ecall_count;
            println!(
                "[diag] ECALL #{} at count={} PC=0x{:08X} priv={:?} a7=0x{:X} a6=0x{:X} a0=0x{:X}",
                last_ecall_count,
                count,
                vm.cpu.pc,
                vm.cpu.privilege,
                vm.cpu.x[17],
                vm.cpu.x[16],
                vm.cpu.x[10]
            );
        }

        // Track unique PCs in last 100K instructions
        if count > 4_900_000 {
            pc_set.insert(vm.cpu.pc);
        }

        // Periodic status report
        if count - last_report >= 500_000 {
            let uart_len = vm.bus.uart.tx_buf.len();
            let sbi_out = vm.bus.sbi.console_output.len();
            println!("[diag] count={} PC=0x{:08X} priv={:?} SATP=0x{:08X} traps={} sbi_m={} ecall_cpu={} uart={} sbi_out={} unique_pcs={}",
                count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp,
                trap_count, sbi_count, last_ecall_count, uart_len, sbi_out, pc_set.len());
            last_report = count;
        }

        count += 1;
    }

    println!(
        "\n[diag] Final: count={} PC=0x{:08X} priv={:?} SATP=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );
    println!("[diag] Total: satp_changes={} m_traps={} m_sbi={} cpu_ecalls={} uart_chars={} sbi_console={}",
        satp_changes, trap_count, sbi_count, last_ecall_count,
        vm.bus.uart.tx_buf.len(), vm.bus.sbi.console_output.len());

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(2000).collect();
        println!("[diag] SBI console output:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("[diag] UART output:\n{}", preview);
    }

    // Disassemble a few instructions around the final PC
    println!("\n[diag] Disassembly around final PC 0x{:08X}:", vm.cpu.pc);
    for offset in -4i32..=4 {
        let addr = (vm.cpu.pc as i64 + offset as i64 * 4) as u64;
        match vm.bus.read_word(addr) {
            Ok(word) => {
                let marker = if offset == 0 { ">>>" } else { "   " };
                println!("{} 0x{:08X}: 0x{:08X}", marker, addr as u32, word);
            }
            Err(_) => {
                println!(
                    "{} 0x{:08X}: <unreadable>",
                    if offset == 0 { ">>>" } else { "   " },
                    addr as u32
                );
            }
        }
    }
}
