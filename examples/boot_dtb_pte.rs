//! Diagnostic: Check page table entries for DTB VA translation at boot time.
//! Run: cargo run --example boot_dtb_pte

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_data = std::fs::read(kernel_path).unwrap();
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs_data = if std::path::Path::new(ir_path).exists() {
        Some(std::fs::read(ir_path).unwrap())
    } else { None };

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data, initramfs_data.as_deref(), 512, bootargs,
    ).expect("boot_linux_setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = (dtb_addr.wrapping_add(0xC0000000)) as u32;
    let max_instr = 500_000u64;
    let mut count = 0u64;

    // The PC where setup_arch loads _dtb_early_va
    let setup_arch_dtb_load = 0xC0403CA6u32;
    let mut checked = false;

    loop {
        if count >= max_instr { break; }
        let result = vm.step();
        match result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("[{}] Fault at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
            _ => {}
        }

        // Right after the kernel loads _dtb_early_va into a0
        if !checked && vm.cpu.pc == setup_arch_dtb_load {
            checked = true;
            // Wait 1 more instruction for the load to complete
            let _ = vm.step();
            count += 1;

            let a0 = vm.cpu.x[10]; // Should be _dtb_early_va
            let satp = vm.cpu.csr.satp;
            eprintln!("[{}] setup_arch loaded a0=0x{:08X} (expect _dtb_early_va=0x{:08X})",
                count, a0, dtb_va);
            eprintln!("[{}] SATP=0x{:08X}", count, satp);

            // Dump page table for the DTB VA
            let ppn = satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            eprintln!("[{}] Page table root at PA 0x{:08X}", count, pg_dir_phys);

            // L1 index for DTB VA (0xC1579000)
            let dtb_l1_idx = ((dtb_va as u64) >> 22) & 0x3FF;
            // L1 index for _dtb_early_va (0xC0801008)
            let deva_l1_idx = ((0xC0801008u64) >> 22) & 0x3FF;
            // L1 index for kernel_map (0xC0C79E90)
            let kmap_l1_idx = ((0xC0C79E90u64) >> 22) & 0x3FF;

            for &idx in &[dtb_l1_idx, deva_l1_idx, kmap_l1_idx] {
                let l1_entry = vm.bus.read_word(pg_dir_phys + idx * 4).unwrap_or(0);
                eprintln!("[{}] L1[{}] = 0x{:08X} (VA 0x{:08X})",
                    count, idx, l1_entry, idx << 22);
                if l1_entry & 1 != 0 {
                    let l1_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                    let is_leaf = ((l1_entry >> 4) & 0xF) != 0; // R|W|X != 0
                    if !is_leaf {
                        // L2 table
                        let l2_idx = ((dtb_va as u64) >> 12) & 0x3FF;
                        let l2_entry = vm.bus.read_word(l1_ppn * 4096 + l2_idx * 4).unwrap_or(0);
                        eprintln!("[{}]   L2[{}] = 0x{:08X}", count, l2_idx, l2_entry);
                    } else {
                        // Megapage
                        eprintln!("[{}]   MEGAPAGE: PA=0x{:08X}", count, l1_ppn << 12);
                    }
                }
            }

            // Also check what the DTB contains at its PA
            let dtb_w0 = vm.bus.read_word(dtb_addr).unwrap_or(0);
            eprintln!("[{}] DTB at PA 0x{:08X}: first word=0x{:08X} (expect 0xD00DFEED)",
                count, dtb_addr as u32, dtb_w0);

            // Read _dtb_early_va from kernel BSS
            let deva = vm.bus.read_word(0x00801008).unwrap_or(0);
            let depa = vm.bus.read_word(0x0080100C).unwrap_or(0);
            eprintln!("[{}] _dtb_early_va (PA 0x00801008) = 0x{:08X}", count, deva);
            eprintln!("[{}] _dtb_early_pa  (PA 0x0080100C) = 0x{:08X}", count, depa);

            // Check phys_ram_base
            let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
            eprintln!("[{}] phys_ram_base (PA 0x00C79EAC) = 0x{:08X}", count, prb);

            // Check kernel_map.phys_addr and va_pa_offset
            let kpa = vm.bus.read_word(0x00C79E90 + 12).unwrap_or(0);
            let kvpo = vm.bus.read_word(0x00C79E90 + 20).unwrap_or(0);
            eprintln!("[{}] kernel_map.phys_addr=0x{:08X} va_pa_offset=0x{:08X}", count, kpa, kvpo);
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
                // ECALL_M = SBI
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

    if !checked {
        eprintln!("Never reached setup_arch DTB load in {} instructions", count);
    }
    eprintln!("\nFinal: {} instr, PC=0x{:08X}", count, vm.cpu.pc);
}
