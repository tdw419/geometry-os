use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    // Keep auto_pte_fixup ENABLED (fixes virtual PPNs during MMU walks)
    // vm.bus.auto_pte_fixup = true;  // already set by boot_linux_setup

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut panic_found: bool = false;

    while count < 2_000_000 {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                sbi_count += 1;
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

        // Panic detection
        if !panic_found && vm.cpu.pc == 0xC000252E {
            panic_found = true;
            eprintln!("[test] PANIC at count={}", count);
            let fmt_va = vm.cpu.x[10];
            if fmt_va >= 0xC0000000 {
                let fmt_pa = (fmt_va - 0xC0000000) as u64;
                let mut chars = Vec::new();
                for j in 0..200u64 {
                    let b = vm.bus.read_byte(fmt_pa + j).unwrap_or(0);
                    if b == 0 { break; }
                    if b >= 0x20 && b < 0x7f { chars.push(b as char); } else { break; }
                }
                eprintln!("[test] FMT: \"{}\"", chars.iter().collect::<String>());
            }
        }

        let step_result = vm.step();

        match step_result {
            StepResult::Ebreak => break,
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor && count < 200_000 {
                    let ft = match step_result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        _ => "store",
                    };
                    eprintln!("[test] S-mode {} fault at count={}: PC=0x{:08X} stval=0x{:08X}",
                        ft, count, vm.cpu.pc, vm.cpu.csr.stval);
                }
            }
            _ => {}
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[test] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            let identity_pte: u32 = 0x0000_00CF;
            
            // Inject identity mappings for low RAM + devices ONLY
            // Do NOT touch kernel L1[768..779] entries
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
            eprintln!("[test] Identity-only fixups applied");
            
            // Verify kernel_map
            let km_phys: u64 = 0x00C79E90;
            let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            if km_pa != 0 || km_vapo != 0xC0000000 {
                eprintln!("[test] Re-patching kernel_map");
                vm.bus.write_word(km_phys + 12, 0).ok();
                vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                vm.bus.write_word(km_phys + 24, 0).ok();
            }
            
            last_satp = cur_satp;
        }

        count += 1;
    }

    let sbi_str: String = vm.bus.sbi.console_output.iter().map(|&b| b as char).collect();
    eprintln!("\n[test] Done: count={} SBI_calls={} panic={}", count, sbi_count, panic_found);
    eprintln!("[test] PC=0x{:08X} SATP=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
    if !sbi_str.is_empty() {
        eprintln!("[test] SBI output (first 3000 chars):");
        let preview: String = sbi_str.chars().take(3000).collect();
        eprintln!("{}", preview);
    }
}
