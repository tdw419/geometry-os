// Diagnostic: trace instructions after the 3rd SATP change to find the panic
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let setup_result = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .unwrap();

    let (mut vm, fw_addr, entry, _dtb_addr) = setup_result;

    // Use boot_linux_setup and run our own loop
    // We need to replicate the MRET setup from boot_linux
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    vm.cpu.csr.write(csr::MEPC, entry);
    vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB;
    vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
    let restored = vm.cpu.csr.trap_return(Privilege::Machine);
    vm.cpu.pc = vm.cpu.csr.mepc;
    vm.cpu.privilege = restored;

    // Setup trap handler at fw_addr
    vm.cpu.csr.write(csr::MTVEC, fw_addr as u32);

    // Delegate exceptions (ECALL_S stays in M-mode for SBI)
    vm.cpu.csr.medeleg = 0xA109;
    vm.cpu.csr.mideleg = 0x222;

    // Set SATP
    let boot_pt_addr = 0x148000u64;
    let satp_val = (1u32 << 31) | ((boot_pt_addr / 4096) as u32);
    vm.cpu.csr.write(csr::SATP, satp_val);

    // Track SATP changes and trace after the 3rd one
    let mut last_satp = satp_val;
    let mut satp_count = 0;
    let mut trace_after_third = false;
    let mut trace_count = 0;
    let max_trace = 500;
    let max_instr = 5_000_000u64;

    let start = Instant::now();
    for count in 1..=max_instr {
        let step_result = vm.step();

        // Check SATP
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_count += 1;
            println!(
                "[{}] SATP changed: 0x{:08X} -> 0x{:08X} (change #{})",
                count, last_satp, cur_satp, satp_count
            );
            last_satp = cur_satp;

            if satp_count == 3 && !trace_after_third {
                println!("[{}] Starting trace after 3rd SATP change", count);
                trace_after_third = true;
                trace_count = 0;
            }
        }

        if trace_after_third && trace_count < max_trace {
            let pc = vm.cpu.pc;
            let inst = vm.bus.read_word(pc as u64).unwrap_or(0);

            // Disassemble a few key patterns
            let desc = if inst == 0x00000013 {
                "NOP".to_string()
            } else if (inst & 0x7F) == 0x73 && ((inst >> 20) & 0xFF) == 0x302 {
                format!("ECALL_S (SBI) a7={}", vm.cpu.x[17])
            } else if (inst & 0x7F) == 0x6F {
                format!("JAL x{}", (inst >> 7) & 0x1F)
            } else if (inst & 0x7F) == 0x67 {
                format!("JALR x{}", (inst >> 7) & 0x1F)
            } else if inst == 0x00008067 {
                "RET".to_string()
            } else {
                format!("0x{:08X}", inst)
            };

            // Log every instruction for the first 200, then only branches/SBI
            if trace_count < 200
                || (inst & 0x7F) == 0x6F
                || (inst & 0x7F) == 0x67
                || (inst & 0x7F) == 0x73
            {
                println!("  [{:6}] PC=0x{:08X} {}", trace_count, pc, desc);
            }

            // Check for UART writes (sw to 0x10000000)
            if (inst & 0x7F) == 0x23 {
                // SW
                let imm = ((inst >> 25) << 5) | ((inst >> 7) & 0x1F);
                let rs1 = (inst >> 15) & 0x1F;
                let rs1_val = vm.cpu.x[rs1 as usize];
                let addr = (rs1_val as i32 as i64 + imm as i64) as u64;
                if addr == 0x10000000 {
                    let rs2 = (inst >> 20) & 0x1F;
                    let byte_val = (vm.cpu.x[rs2 as usize] & 0xFF) as u8;
                    if byte_val >= 0x20 && byte_val < 0x7f {
                        println!(
                            "  [{:6}] *** UART write: '{}' (0x{:02X})",
                            trace_count, byte_val as char, byte_val
                        );
                    } else {
                        println!("  [{:6}] *** UART write: 0x{:02X}", trace_count, byte_val);
                    }
                }
            }

            trace_count += 1;
        }

        if trace_after_third && trace_count >= max_trace {
            println!("[{}] Trace limit reached, stopping", count);
            break;
        }

        // Check for fault
        match step_result {
            StepResult::Ok | StepResult::Ecall | StepResult::Ebreak => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                println!(
                    "[{}] FAULT at PC=0x{:08X} priv={:?}",
                    count, vm.cpu.pc, vm.cpu.privilege
                );
                break;
            }
        }
    }

    let elapsed = start.elapsed();
    println!("\nDone: {} instructions in {:?}", max_instr, elapsed);
    println!(
        "Final PC: 0x{:08X} SATP: 0x{:08X}",
        vm.cpu.pc, vm.cpu.csr.satp
    );

    // Check SBI output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("SBI output: {}", s);
    }
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("UART output: {}", s);
    }
}
