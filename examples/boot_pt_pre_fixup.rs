//! Diagnostic: dump L1 entries of the new page directory BEFORE fixup.
//! This tells us whether the kernel's page table is correct before our fixup touches it.
//! Run: cargo run --example boot_pt_pre_fixup

use std::fs;
use geometry_os::riscv::{cpu::Privilege, cpu::StepResult, RiscvVm};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel not found");
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi earlyprintk loglevel=7";

    // Use boot_linux_setup but DON'T enable auto_pte_fixup
    // so we can see the raw kernel page tables
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    // Disable auto_pte_fixup to see raw kernel PTEs
    // vm.bus.auto_pte_fixup = false;

    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut count: u64 = 0;
    let max_instructions: u64 = 800_000;
    let mut satp_change_count: u32 = 0;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // On SATP change, dump the NEW page directory's L1 entries BEFORE any fixup
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_change_count += 1;
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                eprintln!("\n[pre_fixup] SATP #{} changed to pg_dir PA 0x{:08X} at count={}",
                    satp_change_count, pg_dir_phys, count);
                eprintln!("[pre_fixup] L1 entries (raw kernel PTEs, NO fixup applied):");
                for i in 0..1024u32 {
                    let addr = pg_dir_phys + (i as u64) * 4;
                    if let Ok(pte) = vm.bus.read_word(addr) {
                        if pte != 0 {
                            let pte_ppn = (pte >> 10) & 0x3FFFFF;
                            let flags = pte & 0xFF;
                            let is_leaf = (pte & 0xE) != 0;
                            if is_leaf {
                                eprintln!("  L1[{:4}] = 0x{:08X} MEGAPAGE PPN=0x{:06X} flags=0x{:02X} PA=0x{:08X}",
                                    i, pte, pte_ppn, flags, (pte_ppn as u64) << 22);
                            } else {
                                eprintln!("  L1[{:4}] = 0x{:08X} L2@PA=0x{:08X} flags=0x{:02X}",
                                    i, pte, (pte_ppn as u64) << 12, flags);
                            }
                        }
                    }
                }

                // Don't inject device identity mappings or fixup
                // Just flush TLB
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        // Handle trap at firmware
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;

            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if mpp != 3 {
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
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        match step_result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    eprintln!("[pre_fixup] First fault at count={}: PC=0x{:08X}", count, vm.cpu.pc);
                    break;
                }
            }
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
        }
        count += 1;
    }
    eprintln!("[pre_fixup] Total instructions: {}", count);
}
