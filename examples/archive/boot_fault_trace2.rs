//! Diagnostic: Trace the fault at PC=0xC0210F14 to understand what exception occurs.
//! Run: cargo run --example boot_fault_trace2

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).unwrap();
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else {
        None
    };

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_data, initramfs_data.as_deref(), 512, bootargs)
            .expect("boot_linux_setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let max_instr = 200_000u64;
    let mut count = 0u64;

    loop {
        if count >= max_instr {
            break;
        }

        // Check if we're about to hit the fault (PC approaching 0xC0210F14)
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        count += 1;

        match result {
            StepResult::Ok => {}
            StepResult::FetchFault => {
                eprintln!(
                    "[{}] FETCH FAULT: was at PC=0x{:08X}, now PC=0x{:08X}",
                    count, pc_before, vm.cpu.pc
                );
                eprintln!(
                    "  scause=0x{:08X} stval=0x{:08X} sepc=0x{:08X}",
                    vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.sepc
                );
                eprintln!(
                    "  stvec=0x{:08X} privilege={:?}",
                    vm.cpu.csr.stvec, vm.cpu.privilege
                );

                // The kernel's trap handler should handle this. If it faults again
                // at the same address, that's a problem.
                let handler_pc = vm.cpu.pc;
                let mut handler_count = 0u64;
                loop {
                    if handler_count > 500 {
                        eprintln!(
                            "  Trap handler loop detected after {} iterations",
                            handler_count
                        );
                        break;
                    }
                    let hr = vm.step();
                    handler_count += 1;
                    count += 1;
                    if count >= max_instr {
                        break;
                    }

                    match hr {
                        StepResult::Ok => {
                            // Check if we returned from the handler
                            if vm.cpu.pc < 0xC0210000 || vm.cpu.pc > 0xC0220000 {
                                eprintln!(
                                    "  Returned from handler after {} instr to PC=0x{:08X}",
                                    handler_count, vm.cpu.pc
                                );
                                break;
                            }
                        }
                        StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                            eprintln!(
                                "  FAULT IN HANDLER at PC=0x{:08X} count={}",
                                vm.cpu.pc, handler_count
                            );
                            eprintln!(
                                "    scause=0x{:08X} stval=0x{:08X}",
                                vm.cpu.csr.scause, vm.cpu.csr.stval
                            );
                            break;
                        }
                        _ => {}
                    }
                }
                // If we broke out of the handler loop due to 500 iterations or max_instr,
                // the main loop will also break
                if handler_count > 500 || count >= max_instr {
                    break;
                }
            }
            StepResult::LoadFault => {
                eprintln!(
                    "[{}] LOAD FAULT: was at PC=0x{:08X}, now PC=0x{:08X}",
                    count, pc_before, vm.cpu.pc
                );
                eprintln!(
                    "  scause=0x{:08X} stval=0x{:08X} sepc=0x{:08X}",
                    vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.sepc
                );
            }
            StepResult::StoreFault => {
                eprintln!(
                    "[{}] STORE FAULT: was at PC=0x{:08X}, now PC=0x{:08X}",
                    count, pc_before, vm.cpu.pc
                );
                eprintln!(
                    "  scause=0x{:08X} stval=0x{:08X} sepc=0x{:08X}",
                    vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.sepc
                );
            }
            _ => {}
        }

        // Trap handling
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if (mcause >> 31) & 1 == 1 {
                vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = Privilege::Supervisor;
            } else if cause_code == 11 {
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                if a7 == 0x02 && a6 == 0 && a0 != 0 && a0 != 0xFF {
                    eprint!("{}", a0 as u8 as char);
                    use std::io::Write;
                    std::io::stderr().flush().ok();
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                vm.cpu.csr.mstatus = 1u32 << 7;
                let _ = vm.cpu.csr.trap_return(Privilege::Machine);
                vm.cpu.pc = vm.cpu.csr.mepc;
                vm.cpu.privilege = Privilege::Supervisor;
            }
        }

        // DTB watchdog
        if count % 100 == 0 {
            let prb = vm.bus.read_word(0x00C79EACu64).unwrap_or(0);
            if prb == 0 {
                let cur_va = vm.bus.read_word(0x00801008).unwrap_or(0);
                if cur_va != dtb_va {
                    vm.bus.write_word(0x00801008, dtb_va).ok();
                    vm.bus.write_word(0x0080100C, dtb_addr as u32).ok();
                }
            }
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
    }

    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
    eprintln!("priv={:?}", vm.cpu.privilege);
}
