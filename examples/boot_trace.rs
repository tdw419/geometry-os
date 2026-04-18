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

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut cause_counts: [u64; 32] = [0; 32];
    let mut panic_found: bool = false;
    let mut last_log: u64 = 0;

    while count < 2_000_000 {
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
                    forward_count += 1;
                    if (cause_code as usize) < 32 {
                        cause_counts[cause_code as usize] += 1;
                    }

                    // Log first few forwards in detail
                    if forward_count <= 10 {
                        let stvec = vm.cpu.csr.stvec & !0x3u32;
                        eprintln!("[fwd] #{} count={}: cause={} mepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X} mpp={}",
                            forward_count, count, cause_code, vm.cpu.csr.mepc, vm.cpu.csr.mtval, stvec, mpp);
                    }

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

        if !panic_found && vm.cpu.pc == 0xC000252E {
            panic_found = true;
            eprintln!("[test] PANIC at count={}", count);
        }

        let step_result = vm.step();
        match step_result {
            StepResult::Ebreak => break,
            _ => {}
        }

        // Periodic status
        if count - last_log >= 500_000 {
            last_log = count;
            eprintln!(
                "[status] count={} PC=0x{:08X} priv={:?} forwards={} sbi={}",
                count, vm.cpu.pc, vm.cpu.privilege, forward_count, sbi_count
            );
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[test] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );

            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;

            // Identity mappings
            let identity_pte: u32 = 0x0000_00CF;
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
            if km_pa != 0 || km_vapo != 0xC0000000 {
                vm.bus.write_word(km_phys + 12, 0).ok();
                vm.bus.write_word(km_phys + 20, 0xC0000000).ok();
                vm.bus.write_word(km_phys + 24, 0).ok();
            }

            last_satp = cur_satp;
        }

        count += 1;
    }

    eprintln!(
        "\n[test] Done: count={} forwards={} sbi={}",
        count, forward_count, sbi_count
    );
    eprintln!(
        "[test] PC=0x{:08X} SATP=0x{:08X} panic={}",
        vm.cpu.pc, vm.cpu.csr.satp, panic_found
    );
    eprintln!("[test] Cause counts:");
    for (i, c) in cause_counts.iter().enumerate() {
        if *c > 0 {
            let name = match i {
                2 => "illegal_instruction",
                7 => "timer",
                8 => "ecall_u",
                9 => "ecall_s",
                12 => "fetch_page_fault",
                13 => "load_page_fault",
                15 => "store_page_fault",
                _ => "unknown",
            };
            eprintln!("  cause {} ({}) : {} occurrences", i, name, c);
        }
    }

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    if !sbi_str.is_empty() {
        eprintln!("[test] SBI output (first 3000 chars):");
        let preview: String = sbi_str.chars().take(3000).collect();
        eprintln!("{}", preview);
    }
}
