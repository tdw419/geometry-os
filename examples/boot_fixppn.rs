use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{Privilege, StepResult};

/// Fix virtual PPNs in a page directory. Returns count of entries fixed.
fn fix_virtual_ppns(vm: &mut RiscvVm, pg_dir_phys: u64) -> u32 {
    let page_offset_ppn: u32 = 0xC0000000 >> 12; // 0xC0000
    let mut fixed = 0u32;
    
    for l1_idx in 0..1024u32 {
        let addr = pg_dir_phys + (l1_idx as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        if (entry & 1) == 0 { continue; } // Not valid
        
        let ppn = (entry >> 10) & 0x3FFFFF;
        if ppn >= page_offset_ppn {
            // Virtual PPN detected -- fix it
            let fixed_ppn = ppn - page_offset_ppn;
            let flags = entry & 0x3FF; // Preserve V, R, W, X, U, A, D
            let new_entry = (fixed_ppn << 10) | flags;
            vm.bus.write_word(addr, new_entry).ok();
            fixed += 1;
            
            // If this is a non-leaf entry (points to L2 table), fix L2 entries too
            let is_leaf = (entry & 0xE) != 0; // R|W|X != 0
            if !is_leaf && fixed_ppn > 0 {
                let l2_phys = (fixed_ppn as u64) * 4096;
                for l2_idx in 0..1024u32 {
                    let l2_addr = l2_phys + (l2_idx as u64) * 4;
                    let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                    if (l2_entry & 1) == 0 { continue; }
                    let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
                    if l2_ppn >= page_offset_ppn {
                        let l2_fixed_ppn = l2_ppn - page_offset_ppn;
                        let l2_flags = l2_entry & 0x3FF;
                        let new_l2_entry = (l2_fixed_ppn << 10) | l2_flags;
                        vm.bus.write_word(l2_addr, new_l2_entry).ok();
                        fixed += 1;
                    }
                }
            }
        }
    }
    fixed
}

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

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut panic_found: bool = false;
    let mut total_fixed: u32 = 0;

    while count < 5_000_000 {
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
            _ => {}
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[test] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            
            // Fix ALL virtual PPNs in the page directory
            let fixed = fix_virtual_ppns(&mut vm, pg_dir_phys);
            total_fixed += fixed;
            eprintln!("[test] Fixed {} virtual PPNs in pg_dir at PA 0x{:08X}", fixed, pg_dir_phys);
            
            // Inject identity mappings for low RAM + devices
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
    eprintln!("\n[test] Done: count={} SBI_calls={} panic={} total_fixed={}", 
        count, sbi_count, panic_found, total_fixed);
    eprintln!("[test] PC=0x{:08X} SATP=0x{:08X}", vm.cpu.pc, vm.cpu.csr.satp);
    if !sbi_str.is_empty() {
        eprintln!("[test] SBI output (first 3000 chars):");
        let preview: String = sbi_str.chars().take(3000).collect();
        eprintln!("{}", preview);
    }
}
