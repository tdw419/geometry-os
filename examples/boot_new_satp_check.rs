// Diagnostic: check new page table at PA 0x01485000 (SATP 0x80001485).
// Run: cargo run --example boot_new_satp_check

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

    use geometry_os::riscv::cpu::StepResult;

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
            }
            last_satp = cur_satp;
        }

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
        let step_result = vm.step();

        if let StepResult::FetchFault = step_result {
            if count > 750_000 {
                eprintln!("[fault] count={} FETCH_FAULT: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} SATP=0x{:08X}",
                    count, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.satp);
                break;
            }
        }

        count += 1;
    }

    // Check the new page table at PA 0x01485000
    let new_pg_dir = 0x01485000;
    eprintln!("\n=== New page table at PA 0x{:08X} ===", new_pg_dir);

    // Check kernel VA range L1[768..780]
    for idx in 768..780 {
        let pte = vm.bus.read_word(new_pg_dir + (idx as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let va_base = (idx as u64) << 22;
            let ppn = ((pte >> 10) & 0x3FFFFF) as u64;
            let is_leaf = (pte & 0xE) != 0 || (pte & 0xF0000000) != 0;
            eprintln!("L1[{}] = 0x{:08X} VA 0x{:08X} PPN=0x{:06X} leaf={}", idx, pte, va_base, ppn, is_leaf);
        }
    }

    // Check low RAM L1[0..10]
    for idx in 0..10 {
        let pte = vm.bus.read_word(new_pg_dir + (idx as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let va_base = (idx as u64) << 22;
            eprintln!("L1[{}] = 0x{:08X} VA 0x{:08X}", idx, pte, va_base);
        }
    }

    // Check what the fixmap faulting address 0x9DBFF000 needs
    let fixmap_l1: u32 = 0x9DBFF000 >> 22;
    let fixmap_pte = vm.bus.read_word(new_pg_dir + (fixmap_l1 as u64) * 4).unwrap_or(0);
    eprintln!("\nFixmap VA 0x9DBFF000: L1[{}] = 0x{:08X}", fixmap_l1, fixmap_pte);
}
