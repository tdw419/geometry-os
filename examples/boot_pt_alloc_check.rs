// Diagnostic: watch writes to new page table at PA 0x01485000.
// The kernel creates this table before csrw satp. Check if entries
// are written with correct PPNs.
// Run: cargo run --example boot_pt_alloc_check

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            bootargs,
        ).expect("boot setup failed");

    let fw_addr_u32 = _fw_addr as u32;
    let max_instr: u64 = 752_000;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    // Known page table physical addresses we care about
    let new_pt_base: u64 = 0x01485000; // The new page table the kernel creates
    let mut saw_new_pt_write = false;
    let mut saw_new_pt_satp = false;

    use geometry_os::riscv::cpu::StepResult;

    // Read initial state of new page table area
    for i in 0..10u32 {
        let val = vm.bus.read_word(new_pt_base + (i as u64) * 4).unwrap_or(0);
        if val != 0 {
            eprintln!("[init] PA 0x{:08X} = 0x{:08X}", new_pt_base + (i as u64) * 4, val);
        }
    }

    while count < max_instr {
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(addr, 0xCF | (l1_idx << 20)).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
                eprintln!("[satp] Changed to 0x{:08X} pg_dir=0x{:08X} at count={}", cur_satp, pg_dir_phys, count);
                if pg_dir_phys == new_pt_base {
                    saw_new_pt_satp = true;
                }
            }
            last_satp = cur_satp;
        }

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x300) >> 4;
            if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1u32 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1u32 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        // Before stepping, sample the new page table
        if !saw_new_pt_write && count > 177_300 {
            for i in 768..776u32 {
                let val = vm.bus.read_word(new_pt_base + (i as u64) * 4).unwrap_or(0);
                if val != 0 && !saw_new_pt_write {
                    saw_new_pt_write = true;
                    eprintln!("[watch] count={} First write to new PT: L1[{}] = 0x{:08X}", count, i, val);
                }
            }
        }

        let step_result = vm.step();

        if let StepResult::FetchFault = step_result {
            if count > 750_000 {
                eprintln!("[fault] count={} FETCH_FAULT: PC=0x{:08X} sepc=0x{:08X}", count, vm.cpu.pc, vm.cpu.csr.sepc);
                // Dump L1 entries of the new page table
                eprintln!("\nNew page table L1[768..776] after SATP switch:");
                for i in 768..776u32 {
                    let val = vm.bus.read_word(new_pt_base + (i as u64) * 4).unwrap_or(0);
                    if val != 0 {
                        let ppn = (val >> 10) & 0x3FFFFF;
                        let flags = val & 0x3FF;
                        eprintln!("  L1[{}] = 0x{:08X} PPN=0x{:06X} flags=0x{:03X}", i, val, ppn, flags);
                    }
                }
                // Check if the fixmap entries are present
                eprintln!("\nNew page table L1[628..635]:");
                for i in 628..635u32 {
                    let val = vm.bus.read_word(new_pt_base + (i as u64) * 4).unwrap_or(0);
                    if val != 0 {
                        let ppn = (val >> 10) & 0x3FFFFF;
                        eprintln!("  L1[{}] = 0x{:08X} PPN=0x{:06X}", i, val, ppn);
                    }
                }
                break;
            }
        }

        count += 1;
    }

    eprintln!("\nDone at count={}", count);
}
