// Diagnostic: watch pt_ops[4] at VA 0xC0801004 (PA 0x00801004) for writes and reads.
// Run: cargo run --example boot_pt_ops_watch

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        bootargs,
    )
    .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let max_instr: u64 = 190_000;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    // Physical address of pt_ops[4]
    let pt_ops_4_pa: u64 = 0x00801004;
    let mut last_pt_ops_4_val: u32 = 0;

    // Read initial value
    last_pt_ops_4_val = vm.bus.read_word(pt_ops_4_pa).unwrap_or(0);
    eprintln!(
        "[watch] Initial pt_ops[4] at PA 0x{:08X} = 0x{:08X}",
        pt_ops_4_pa, last_pt_ops_4_val
    );

    use geometry_os::riscv::cpu::StepResult;

    while count < max_instr {
        // SATP change handling (same as boot_linux)
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

        // Trap forwarding
        if vm.cpu.pc == fw_addr_u32
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

        let step_result = vm.step();

        // Check pt_ops[4] every 100 instructions for changes
        if count % 100 == 0 {
            let cur_val = vm.bus.read_word(pt_ops_4_pa).unwrap_or(0);
            if cur_val != last_pt_ops_4_val {
                eprintln!(
                    "[watch] count={} pt_ops[4] CHANGED: 0x{:08X} -> 0x{:08X} (PC=0x{:08X} SATP=0x{:08X})",
                    count, last_pt_ops_4_val, cur_val, vm.cpu.pc, vm.cpu.csr.satp
                );
                last_pt_ops_4_val = cur_val;
            }
        }

        match step_result {
            StepResult::FetchFault => {
                let sepc = vm.cpu.csr.sepc;
                if sepc == 0x804046C8 || count > 186_500 {
                    eprintln!(
                        "[fault] count={} FETCH_FAULT: PC=0x{:08X} sepc=0x{:08X}",
                        count, vm.cpu.pc, sepc
                    );
                    eprintln!(
                        "[fault] pt_ops[4] at PA 0x{:08X} = 0x{:08X}",
                        pt_ops_4_pa, last_pt_ops_4_val
                    );
                    break;
                }
            }
            StepResult::LoadFault | StepResult::StoreFault => {
                if count > 186_500 {
                    eprintln!(
                        "[fault] count={} {:?}: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                        count, step_result, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval
                    );
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }

        count += 1;
    }

    eprintln!("\n=== Final State ===");
    eprintln!(
        "count={} PC=0x{:08X} priv={:?}",
        count, vm.cpu.pc, vm.cpu.privilege
    );
    eprintln!("pt_ops[4] = 0x{:08X}", last_pt_ops_4_val);
}
