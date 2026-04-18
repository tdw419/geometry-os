//! Diagnostic: Trace what the kernel actually reads from the DTB VA during fdt_check_header.
//! Uses MMU translate to check what the kernel would see.
//! Run: cargo run --example boot_dtb_mmu_trace

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

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
    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        bootargs,
    )
    .expect("boot_linux_setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let max_instr = 500_000u64;
    let mut count = 0u64;

    // Watch for fdt_check_header entry
    let fdt_check_pc = 0xC01EB8E2u32;
    let mut in_fdt_check = false;
    let mut fdt_check_instr_count = 0u64;

    // After setup_arch calls early_init_dt_scan, watch for return
    let early_init_return = 0xC0403CB2u32;
    let mut early_init_result = 0u32;

    loop {
        if count >= max_instr {
            break;
        }
        let result = vm.step();
        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("[{}] FAULT at PC=0x{:08X}", count, vm.cpu.pc);
                // Print scause/stval for context
                eprintln!(
                    "  scause=0x{:08X} stval=0x{:08X} sepc=0x{:08X}",
                    vm.cpu.csr.scause,
                    vm.cpu.csr.stval,
                    vm.cpu.csr.sepc
                );
                break;
            }
            _ => {}
        }

        // Detect entry into fdt_check_header
        if !in_fdt_check && vm.cpu.pc == fdt_check_pc {
            in_fdt_check = true;
            eprintln!(
                "[{}] Entered fdt_check_header(a0=0x{:08X})",
                count, vm.cpu.x[10]
            );
            // Manually do MMU translation
            let va = vm.cpu.x[10] as u32;
            let satp = vm.cpu.csr.satp;
            let vpn1 = ((va >> 22) & 0x3FF) as u64;
            let vpn0 = ((va >> 12) & 0x3FF) as u64;
            let ppn = (satp & 0x3FFFFF) as u64;
            let l1_addr = ppn * 4096 + vpn1 * 4;
            let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
            eprintln!(
                "  SATP=0x{:08X} L1[{}]=0x{:08X} (addr=PA 0x{:08X})",
                satp, vpn1, l1_entry, l1_addr
            );
            if l1_entry & 1 != 0 {
                let rwx = (l1_entry >> 1) & 0x7;
                if rwx != 0 {
                    // Megapage
                    let l1_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                    let pa = (l1_ppn << 12) | (va as u64 & 0x3FFFFF);
                    eprintln!("  MEGAPAGE: VA 0x{:08X} -> PA 0x{:08X}", va, pa);
                    let w0 = vm.bus.read_word(pa).unwrap_or(0);
                    eprintln!("  Word at PA 0x{:08X}: 0x{:08X} (expect 0xEDFE0DD0)", pa, w0);
                } else {
                    // L2 table
                    let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                    let l2_addr = l2_ppn * 4096 + vpn0 * 4;
                    let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                    eprintln!("  L2[{}]=0x{:08X} (addr=PA 0x{:08X})", vpn0, l2_entry, l2_addr);
                    if l2_entry & 1 != 0 {
                        let leaf_ppn = ((l2_entry >> 10) & 0x3FFFFF) as u64;
                        let offset = va as u64 & 0xFFF;
                        let pa = (leaf_ppn << 12) | offset;
                        eprintln!("  LEAF: VA 0x{:08X} -> PA 0x{:08X}", va, pa);
                        let w0 = vm.bus.read_word(pa).unwrap_or(0);
                        eprintln!(
                            "  Word at PA 0x{:08X}: 0x{:08X} (expect 0xEDFE0DD0)",
                            pa, w0
                        );
                    } else {
                        eprintln!("  L2 entry not valid!");
                    }
                }
            } else {
                eprintln!("  L1 entry not valid!");
            }
        }

        if in_fdt_check {
            fdt_check_instr_count += 1;
            if fdt_check_instr_count > 200 {
                eprintln!("[{}] fdt_check_header took too many instructions, bailing", count);
                break;
            }
            // Check if we returned (PC moved past fdt_check_header)
            if vm.cpu.pc == 0xC01EB8E0u32 {
                // ret instruction
                eprintln!(
                    "[{}] fdt_check_header returned: a0=0x{:08X}",
                    count, vm.cpu.x[10]
                );
                in_fdt_check = false;
            }
        }

        // Detect return from early_init_dt_scan
        if vm.cpu.pc == early_init_return && early_init_result == 0 {
            early_init_result = vm.cpu.x[10]; // Return value in a0
            eprintln!(
                "[{}] early_init_dt_scan returned a0={} (0=success)",
                count, early_init_result
            );
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
        count += 1;
    }

    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
}
