//! Diagnostic: Check L1[5] page table entry at the time of the load fault at VA 0x01579004.
//! Run: cargo run --example boot_l5_check

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
        let pc_before = vm.cpu.pc;
        let result = vm.step();
        count += 1;

        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                let scause = vm.cpu.csr.scause;
                let stval = vm.cpu.csr.stval;
                let sepc = vm.cpu.csr.sepc;

                if stval == 0x01579004 || stval == 0x01579000 {
                    eprintln!(
                        "[{}] FAULT at sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                        count, sepc, scause, stval
                    );
                    eprintln!("  PC before step: 0x{:08X}", pc_before);
                    eprintln!("  SATP=0x{:08X}", vm.cpu.csr.satp);

                    let satp = vm.cpu.csr.satp;
                    let ppn = (satp & 0x3FFFFF) as u64;
                    let pg_dir_phys = ppn * 4096;

                    // Check L1[5] for VA 0x01579004
                    let vpn1 = ((0x01579004u32 >> 22) & 0x3FF) as u64;
                    let l5_addr = pg_dir_phys + vpn1 * 4;
                    let l5_entry = vm.bus.read_word(l5_addr).unwrap_or(0);
                    eprintln!(
                        "  L1[{}] at PA 0x{:08X} = 0x{:08X}",
                        vpn1, l5_addr, l5_entry
                    );

                    if l5_entry & 1 != 0 {
                        let rwx = (l5_entry >> 1) & 0x7;
                        if rwx != 0 {
                            let l5_ppn = ((l5_entry >> 10) & 0x3FFFFF) as u64;
                            eprintln!("  L1[{}] MEGAPAGE: PA base = 0x{:08X}", vpn1, l5_ppn << 12);
                            eprintln!(
                                "  VA 0x01579004 -> PA 0x{:08X}",
                                (l5_ppn << 12) + (0x01579004 & 0x3FFFFF)
                            );
                        } else {
                            // L2 table
                            let l2_ppn = ((l5_entry >> 10) & 0x3FFFFF) as u64;
                            let vpn0 = ((0x01579004u32 >> 12) & 0x3FF) as u64;
                            let l2_addr = l2_ppn * 4096 + vpn0 * 4;
                            let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                            eprintln!(
                                "  L2[{}] at PA 0x{:08X} = 0x{:08X}",
                                vpn0, l2_addr, l2_entry
                            );
                        }
                    } else {
                        eprintln!("  L1[{}] NOT VALID - this is the root cause!", vpn1);
                        // Check all L1[0..6]
                        for i in 0..8u64 {
                            let e = vm.bus.read_word(pg_dir_phys + i * 4).unwrap_or(0);
                            eprintln!(
                                "  L1[{}] = 0x{:08X}{}",
                                i,
                                e,
                                if e & 1 != 0 { " [valid]" } else { " [INVALID]" }
                            );
                        }
                    }
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
}
