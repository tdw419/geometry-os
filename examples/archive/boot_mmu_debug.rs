/// Debug MMU translation for a specific VA during boot.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut mmu_log_shown = false;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if !trampoline_patched
            && vm.cpu.pc == 0x10EE
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor
            && vm.cpu.csr.satp == 0
        {
            let identity_pte: u32 = 0x0000_00EF;
            let l1_entries: &[u64] = &[0, 2, 4, 5, 6, 8, 10];
            let trampoline_phys = 0x0148_4000u64;
            let early_pg_dir_phys = 0x0080_2000u64;
            for &l1_idx in l1_entries {
                let pte = identity_pte | ((l1_idx as u32) << 20);
                let addr_offset = (l1_idx * 4) as u64;
                vm.bus.write_word(trampoline_phys + addr_offset, pte).ok();
                vm.bus.write_word(early_pg_dir_phys + addr_offset, pte).ok();
            }
            trampoline_patched = true;
            eprintln!("[mmu] Trampoline patched at count={}", count);
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[mmu] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
        }
        last_satp = cur_satp;

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
                if mpp != 3 {
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
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
        }

        // When we first reach a VA in the 0xC000xxxx range after MMU is on,
        // do a manual page table walk and compare with the bus read
        if count > 177300 && count < 177600 && !mmu_log_shown {
            let pc = vm.cpu.pc;
            if pc >= 0xC0000000 {
                mmu_log_shown = true;
                let satp = vm.cpu.csr.satp;
                let vpn1 = (pc >> 22) & 0x3FF;
                let vpn0 = (pc >> 12) & 0x3FF;
                let offset = pc & 0xFFF;
                let pt_ppn = (satp & 0x3FFFFF) as u64;
                let pt_base = pt_ppn << 12;

                eprintln!("\n[mmu] Manual walk for PC=0x{:08X} at count={}", pc, count);
                eprintln!("[mmu] SATP=0x{:08X} PT_BASE=0x{:08X}", satp, pt_base);
                eprintln!("[mmu] VPN1={} VPN0={} offset=0x{:03X}", vpn1, vpn0, offset);

                let l1_addr = pt_base + (vpn1 as u64) * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                let l1_v = l1_pte & 1;
                let l1_leaf = (l1_pte & 0xE) != 0;
                let l1_ppn_hi = (l1_pte >> 20) & 0xFFF;

                eprintln!(
                    "[mmu] L1[{}] at PA 0x{:08X} = 0x{:08X} (V={} leaf={} PPN_hi=0x{:03X})",
                    vpn1, l1_addr, l1_pte, l1_v, l1_leaf, l1_ppn_hi
                );

                if l1_v != 0 && l1_leaf {
                    let expected_pa =
                        ((l1_ppn_hi as u64) << 22) | ((vpn0 as u64) << 12) | (offset as u64);
                    let mem_val = vm.bus.read_word(expected_pa).unwrap_or(0);
                    eprintln!(
                        "[mmu] Megapage -> expected PA=0x{:08X}, mem=0x{:08X}",
                        expected_pa, mem_val
                    );
                }

                // TLB lookup
                let asid = ((satp >> 22) & 0x1FF) as u16;
                let combined_vpn = (pc >> 12) as u32;
                if let Some((ppn, flags)) = vm.cpu.tlb.lookup(combined_vpn, asid) {
                    let tlb_pa = ((ppn as u64) << 12) | (offset as u64);
                    let tlb_val = vm.bus.read_word(tlb_pa).unwrap_or(0);
                    eprintln!(
                        "[mmu] TLB hit: PPN=0x{:06X} flags=0x{:02X} -> PA=0x{:08X} mem=0x{:08X}",
                        ppn, flags, tlb_pa, tlb_val
                    );
                } else {
                    eprintln!("[mmu] TLB miss (will do page table walk)");
                }

                // Check what the bus reads at the PC directly (physical = virtual since bus doesn't do MMU)
                let direct_val = vm.bus.read_word(pc as u64).unwrap_or(0);
                eprintln!(
                    "[mmu] Direct bus read at VA=0x{:08X} (no MMU) = 0x{:08X}",
                    pc, direct_val
                );
            }
        }

        let _ = vm.step();
        count += 1;
    }

    // After the loop, check what the bus returns for several addresses
    eprintln!("\n[mmu] Post-boot bus reads:");
    let check_pas = [0u64, 0xA618, 0x10EE, 0x1000, 0x1084, 0x3D450];
    for pa in &check_pas {
        let val = vm.bus.read_word(*pa).unwrap_or(0);
        eprintln!("  PA 0x{:08X}: 0x{:08X}", pa, val);
    }

    // Check TLB state
    eprintln!("\n[mmu] Done.");
}
