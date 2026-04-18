use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

/// Clean boot test: minimal trap handling, auto_pte_fixup enabled,
/// no SATP-change megapage injection. Tests if the kernel_map binary
/// patch makes the MMU fixup unnecessary.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    // Keep auto_pte_fixup enabled - the kernel_map patch should make
    // PTEs correct, but fixup provides a safety net.
    // vm.bus.auto_pte_fixup = true;  // already true from setup

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut smode_fault_count: u64 = 0;
    let mut ecall_m_count: u64 = 0;

    let max_count: u64 = 2_000_000;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[clean] Shutdown requested at count={}", count);
            break;
        }

        // Handle M-mode trap (fw_addr = MRET instruction)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                // ECALL_S from S-mode = SBI call
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
            } else if cause_code == 11 {
                // ECALL_M
                ecall_m_count += 1;
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
            } else {
                // Other exceptions from S/U-mode: forward to S-mode handler
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        if cause_code == 7 {
                            vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                        }
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        count += 1;
                        continue;
                    }
                }
                // MPP=3 or no stvec: skip instruction
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[clean] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            // Just flush TLB, don't inject megapages
            vm.cpu.tlb.flush_all();
            last_satp = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        // Log S-mode faults
        if vm.cpu.privilege == Privilege::Supervisor {
            let scause = vm.cpu.csr.scause;
            let cause_code = scause & !(1u32 << 31);
            if cause_code >= 12 && cause_code <= 15 {
                smode_fault_count += 1;
                if smode_fault_count <= 5 {
                    let fault_type = match cause_code {
                        12 => "instr",
                        13 => "load",
                        15 => "store",
                        _ => "unknown",
                    };
                    eprintln!(
                        "[clean] S-mode {} fault #{} at count={}: PC=0x{:08X} stval=0x{:08X} sepc=0x{:08X}",
                        fault_type, smode_fault_count, count,
                        vm.cpu.pc, vm.cpu.csr.stval, vm.cpu.csr.sepc
                    );
                }
            }
        }

        match step_result {
            StepResult::Ok => {}
            StepResult::Ebreak => {
                eprintln!("[clean] EBREAK at count={}, PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            StepResult::Ecall => {}
            _ => {}
        }

        count += 1;

        // Progress reports
        if count % 500_000 == 0 {
            eprintln!(
                "[clean] Progress: count={} PC=0x{:08X} priv={:?} SBI={} fwd={} smode_faults={}",
                count, vm.cpu.pc, vm.cpu.privilege, sbi_count, forward_count, smode_fault_count
            );
        }
    }

    eprintln!(
        "[clean] Done: count={} SBI={} ECALL_M={} fwd={} smode_faults={}",
        count, sbi_count, ecall_m_count, forward_count, smode_fault_count
    );

    // Check UART output
    let uart_output = vm.bus.uart.drain_tx();
    if !uart_output.is_empty() {
        let preview: String =
            String::from_utf8_lossy(&uart_output[..uart_output.len().min(500)]).to_string();
        eprintln!(
            "[clean] UART output ({} bytes): {}",
            uart_output.len(),
            preview
        );
    } else {
        eprintln!("[clean] No UART output");
    }
}
