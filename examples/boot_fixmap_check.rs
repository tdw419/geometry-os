// Diagnostic: check PTE for faulting address 0x9DBFF000 after setup_vm.
// Run: cargo run --example boot_fixmap_check

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, _fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        bootargs,
    )
    .expect("boot setup failed");

    let max_instr: u64 = 180_000;
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

        if vm.cpu.pc == (_fw_addr as u32)
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
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
        vm.step();
        count += 1;
    }

    // Check PTE for 0x9DBFF000 in the current page table
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    eprintln!("SATP=0x{:08X} pg_dir at PA 0x{:08X}", satp, pg_dir_phys);

    // Check L1[630] for VA 0x9DBFF000
    let l1_idx: u32 = 0x9DBFF000 >> 22;
    eprintln!("VA 0x9DBFF000: L1 index = {}", l1_idx);
    let l1_pte = vm
        .bus
        .read_word(pg_dir_phys + (l1_idx as u64) * 4)
        .unwrap_or(0);
    eprintln!("L1[{}] = 0x{:08X} (V={})", l1_idx, l1_pte, (l1_pte & 1));

    // Also check nearby indices
    for idx in 628..635 {
        let pte = vm
            .bus
            .read_word(pg_dir_phys + (idx as u64) * 4)
            .unwrap_or(0);
        if pte != 0 {
            let va_base = (idx as u64) << 22;
            let ppn = ((pte >> 10) & 0x3FFFFF) as u64;
            let pa_base = ppn << 2;
            eprintln!(
                "L1[{}] = 0x{:08X} -> VA 0x{:08X} PA 0x{:08X} (V={} R={} W={} X={})",
                idx,
                pte,
                va_base,
                pa_base,
                (pte >> 0) & 1,
                (pte >> 1) & 1,
                (pte >> 2) & 1,
                (pte >> 3) & 1
            );
        }
    }

    // Check pt_ops values
    let pt_ops_0 = vm.bus.read_word(0x00801000).unwrap_or(0);
    let pt_ops_4 = vm.bus.read_word(0x00801004).unwrap_or(0);
    eprintln!("\npt_ops[0] (get_pte_virt) = 0x{:08X}", pt_ops_0);
    eprintln!("pt_ops[4] (alloc_pte)     = 0x{:08X}", pt_ops_4);

    // kernel_map
    let km_phys: u64 = 0x00C79E90;
    eprintln!("\nkernel_map:");
    eprintln!(
        "  phys_addr          = 0x{:08X}",
        vm.bus.read_word(km_phys + 12).unwrap_or(0)
    );
    eprintln!(
        "  va_pa_offset       = 0x{:08X}",
        vm.bus.read_word(km_phys + 20).unwrap_or(0)
    );
    eprintln!(
        "  va_kernel_pa_offset = 0x{:08X}",
        vm.bus.read_word(km_phys + 24).unwrap_or(0)
    );
}
