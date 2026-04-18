use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    vm.bus.auto_pte_fixup = false;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut first_fault: bool = true;

    while count < 1_000_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                sbi_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        match step_result {
            StepResult::Ebreak => break,
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && first_fault {
                    first_fault = false;
                    let ft = match step_result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        _ => "store",
                    };
                    eprintln!("[diag] FIRST S-mode {} fault at count={}", ft, count);
                    eprintln!("[diag]   PC=0x{:08X}", vm.cpu.pc);
                    eprintln!("[diag]   scause=0x{:08X}", vm.cpu.csr.scause);
                    eprintln!("[diag]   stval=0x{:08X}", vm.cpu.csr.stval);
                    eprintln!("[diag]   sepc=0x{:08X}", vm.cpu.csr.sepc);
                    eprintln!("[diag]   stvec=0x{:08X}", vm.cpu.csr.stvec);
                    eprintln!("[diag]   mstatus=0x{:08X}", vm.cpu.csr.mstatus);
                    eprintln!("[diag]   SP=0x{:08X} RA=0x{:08X}", vm.cpu.x[2], vm.cpu.x[1]);
                    eprintln!("[diag]   GP=0x{:08X} TP=0x{:08X}", vm.cpu.x[3], vm.cpu.x[4]);
                    eprintln!("[diag]   SATP=0x{:08X}", vm.cpu.csr.satp);

                    // Read instruction at PC
                    let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
                    eprintln!("[diag]   instruction at PC: 0x{:08X}", inst);

                    // Disassemble: check if it's a store
                    let opcode = inst & 0x7F;
                    let funct3 = (inst >> 12) & 7;
                    eprintln!("[diag]   opcode=0x{:02X} funct3={}", opcode, funct3);

                    // Check what L1 index the fault address maps to
                    let fault_va = vm.cpu.csr.stval;
                    let l1_idx = (fault_va >> 22) & 0x3FF;
                    let ppn = vm.cpu.csr.satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_entry = vm
                        .bus
                        .read_word(pg_dir_phys + (l1_idx as u64) * 4)
                        .unwrap_or(0);
                    eprintln!(
                        "[diag]   fault VA L1[{}] = 0x{:08X} (V={})",
                        l1_idx,
                        l1_entry,
                        l1_entry & 1
                    );

                    // Also check the SP's L1 entry
                    let sp_l1_idx = (vm.cpu.x[2] >> 22) & 0x3FF;
                    let sp_l1_entry = vm
                        .bus
                        .read_word(pg_dir_phys + (sp_l1_idx as u64) * 4)
                        .unwrap_or(0);
                    eprintln!(
                        "[diag]   SP VA L1[{}] = 0x{:08X} (V={})",
                        sp_l1_idx,
                        sp_l1_entry,
                        sp_l1_entry & 1
                    );

                    // Run 200 more steps to see the aftermath
                    eprintln!("[diag] --- Running 200 more steps ---");
                    for _ in 0..200 {
                        vm.bus.tick_clint();
                        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
                        let sr = vm.step();
                        if matches!(sr, StepResult::Ebreak) {
                            break;
                        }
                        count += 1;
                    }
                    eprintln!("[diag] --- After 200 steps ---");
                    eprintln!(
                        "[diag]   PC=0x{:08X} SP=0x{:08X} RA=0x{:08X}",
                        vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]
                    );
                    eprintln!(
                        "[diag]   scause=0x{:08X} stval=0x{:08X}",
                        vm.cpu.csr.scause, vm.cpu.csr.stval
                    );
                    break;
                }
            }
            _ => {}
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[boot] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );

            // Inject identity mappings for all of low memory + devices
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            let identity_pte: u32 = 0x0000_00CF;

            // Map ALL of physical RAM as identity: L1[0..64] covers 256MB
            for i in 0..64u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    let pte = identity_pte | (i << 20);
                    vm.bus.write_word(addr, pte).ok();
                }
            }
            for &l1_idx in &[8u32, 48, 64] {
                let addr = pg_dir_phys + (l1_idx as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    let pte = identity_pte | (l1_idx << 20);
                    vm.bus.write_word(addr, pte).ok();
                }
            }
            vm.cpu.tlb.flush_all();

            // Verify kernel_map
            let km_phys: u64 = 0x00C79E90;
            let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
            if km_pa != 0 || km_vapo != 0xC0000000 || km_vkpo != 0 {
                eprintln!("[boot] Re-patching kernel_map");
                vm.bus.write_word(km_phys + 12, 0).ok();
                vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                vm.bus.write_word(km_phys + 24, 0).ok();
            }

            last_satp = cur_satp;
        }

        count += 1;
    }

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!("\n[boot] Done: count={} SBI_calls={}", count, sbi_count);
    if !sbi_str.is_empty() {
        eprintln!("[boot] SBI output: {}", &sbi_str[..sbi_str.len().min(2000)]);
    }
}
