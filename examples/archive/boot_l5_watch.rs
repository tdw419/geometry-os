//! Diagnostic: Watch L1[5] at PA 0x00802014 to see when it gets cleared.
//! Run: cargo run --example boot_l5_watch

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

    // Watch L1[5] at PA 0x00802014
    let l5_pa = 0x00802014u64;
    let mut last_l5 = vm.bus.read_word(l5_pa).unwrap_or(0);
    let mut l5_changes = 0u64;
    let mut satp_changes = 0u32;
    let mut last_satp = vm.cpu.csr.satp;

    loop {
        if count >= max_instr {
            break;
        }
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        count += 1;

        // Check SATP change
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            eprintln!(
                "[{}] SATP changed: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );
            // Check L1[5] right after SATP change
            let l5 = vm.bus.read_word(l5_pa).unwrap_or(0);
            eprintln!("  L1[5] immediately after: 0x{:08X}", l5);
            last_satp = cur_satp;
        }

        // Check L1[5] every instruction (only in the range 177000-186000)
        if count >= 177000 && count <= 186000 {
            let l5 = vm.bus.read_word(l5_pa).unwrap_or(0);
            if l5 != last_l5 {
                l5_changes += 1;
                eprintln!(
                    "[{}] L1[5] changed: 0x{:08X} -> 0x{:08X} (PC was 0x{:08X})",
                    count, last_l5, l5, pc_before
                );
                // Also show a few surrounding L1 entries
                for i in 0..8u64 {
                    let e = vm.bus.read_word(0x00802000 + i * 4).unwrap_or(0);
                    if e != 0 {
                        eprintln!("  L1[{}] = 0x{:08X}", i, e);
                    }
                }
                if l5_changes >= 10 {
                    eprintln!("  Too many changes, stopping watch");
                    break;
                }
                last_l5 = l5;
            }
        }

        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                let stval = vm.cpu.csr.stval;
                if stval == 0x01579004 || stval == 0x01579000 {
                    eprintln!(
                        "[{}] FAULT stval=0x{:08X} scause=0x{:08X}",
                        count, stval, vm.cpu.csr.scause
                    );
                    // Check L1[5] at fault time
                    let l5 = vm.bus.read_word(l5_pa).unwrap_or(0);
                    eprintln!("  L1[5] at fault: 0x{:08X}", l5);
                    break;
                }
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
    eprintln!(
        "SATP changes: {}, L1[5] changes: {}",
        satp_changes, l5_changes
    );
}
