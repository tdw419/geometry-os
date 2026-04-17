//! Diagnostic: dump page table state at the third SATP change.

use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::mmu::{self, AccessType};

fn pte_flags(pte: u32) -> String {
    let mut s = String::new();
    s.push(if pte & 1 != 0 { 'V' } else { 'v' });
    s.push(if pte & 2 != 0 { 'R' } else { '-' });
    s.push(if pte & 4 != 0 { 'W' } else { '-' });
    s.push(if pte & 8 != 0 { 'X' } else { '-' });
    s.push(if pte & 16 != 0 { 'U' } else { '-' });
    s.push(if pte & 32 != 0 { 'A' } else { '-' });
    s.push(if pte & 64 != 0 { 'D' } else { '-' });
    s
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 2_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_count: u32 = 0;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_count += 1;
                eprintln!(
                    "[diag] SATP #{} at count={}: 0x{:08X} -> 0x{:08X}",
                    satp_count, count, last_satp, cur_satp
                );

            if satp_count == 3 {
                let new_ppn = cur_satp & 0x003F_FFFF;
                let pg_dir_phys = (new_ppn as u64) * 4096;
                eprintln!("[diag] New pg_dir at PA 0x{:08X}", pg_dir_phys);

                // Dump L1 entries for kernel VA range (L1 indices 768-800)
                for l1_idx in 768..801u32 {
                    let l1_addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                    if l1_pte != 0 {
                        let ppn = (l1_pte >> 10) & 0x3FF_FFF;
                        let is_leaf = (l1_pte & 0xE) != 0;
                        if is_leaf {
                            eprintln!(
                                "[diag] L1[{}] = 0x{:08X} MEGAPAGE PA 0x{:08X} {}",
                                l1_idx, l1_pte, ppn << 22, pte_flags(l1_pte)
                            );
                        } else {
                            eprintln!(
                                "[diag] L1[{}] = 0x{:08X} -> L2 PA 0x{:08X} {}",
                                l1_idx, l1_pte, ppn << 12, pte_flags(l1_pte)
                            );
                        }
                    }
                }

                // Page table walk for VA 0xC0210F14 (handle_exception)
                let test_va: u32 = 0xC0210F14;
                let vpn2 = (test_va >> 22) as u64;
                let vpn1 = ((test_va >> 12) & 0x3FF) as u64;
                eprintln!("\n[diag] Walk VA 0x{:08X} (vpn2={} vpn1={})", test_va, vpn2, vpn1);
                let l1_pte = vm.bus.read_word(pg_dir_phys + vpn2 * 4).unwrap_or(0);
                eprintln!("[diag] L1[{}] = 0x{:08X}", vpn2, l1_pte);
                if l1_pte & 1 == 0 {
                    eprintln!("[diag] NOT MAPPED (L1 V=0)");
                } else if (l1_pte & 0xE) != 0 {
                    eprintln!(
                        "[diag] MEGAPAGE -> PA 0x{:08X}",
                        (((l1_pte >> 10) & 0x3FF_FFF) as u64) << 22 | ((test_va as u64) & 0x3FFFFF)
                    );
                } else {
                    let l2_base = (((l1_pte >> 10) & 0x3FF_FFF) as u64) << 12;
                    let l2_pte = vm.bus.read_word(l2_base + vpn1 * 4).unwrap_or(0);
                    eprintln!("[diag] L2[{}] at PA 0x{:08X} = 0x{:08X}", vpn1, l2_base + vpn1 * 4, l2_pte);
                    if l2_pte & 1 == 0 {
                        eprintln!("[diag] NOT MAPPED (L2 V=0)");
                    } else {
                        let pa = (((l2_pte >> 10) & 0x3FF_FFF) as u64) << 12 | ((test_va as u64) & 0xFFF);
                        eprintln!("[diag] -> PA 0x{:08X}", pa);
                    }
                }

                // MMU translate
                let result = mmu::translate(
                    test_va,
                    AccessType::Fetch,
                    Privilege::Supervisor,
                    false,
                    false,
                    vm.cpu.csr.satp,
                    &mut vm.bus,
                    &mut vm.cpu.tlb,
                );
                match result {
                    mmu::TranslateResult::Ok(pa) => eprintln!("[diag] MMU -> PA 0x{:08X}", pa),
                    mmu::TranslateResult::FetchFault => eprintln!("[diag] MMU fetch fault"),
                    mmu::TranslateResult::LoadFault => eprintln!("[diag] MMU load fault"),
                    mmu::TranslateResult::StoreFault => eprintln!("[diag] MMU store fault"),
                }

                // Compare with old page table
                let old_ppn = last_satp & 0x003F_FFFF;
                let old_pg_dir = (old_ppn as u64) * 4096;
                eprintln!("\n[diag] Old pg_dir at PA 0x{:08X}:", old_pg_dir);
                for l1_idx in [776u32, 784] {
                    let l1_pte = vm.bus.read_word(old_pg_dir + (l1_idx as u64) * 4).unwrap_or(0);
                    eprintln!("[diag] OLD L1[{}] = 0x{:08X}", l1_idx, l1_pte);
                }

                break;
            }
            last_satp = cur_satp;
        }

        // M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
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
        let _ = vm.step();
        count += 1;
    }

    if satp_count < 3 {
        eprintln!("[diag] Only {} SATP changes in {} instructions", satp_count, count);
    }
}
