/// Check page table mappings after 250K instructions of Linux boot.
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
    let max_instructions: u64 = 250_000;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;

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
        }

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp && cur_satp != 0 {
            let new_ppn = (cur_satp & 0x3FFFFF) as u64;
            let pt_base = new_ppn << 12;
            let asid = (cur_satp >> 22) & 0x1FF;
            for l1_idx in 0..1024u64 {
                let l1_addr = pt_base + l1_idx * 4;
                let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                if l1_pte == 0 {
                    continue;
                }
                let is_leaf = (l1_pte & 0xE) != 0;
                if !is_leaf {
                    continue;
                }
                let ppn_hi = ((l1_pte >> 20) & 0xFFF) as u32;
                let flags = (l1_pte & 0xFF) as u32;
                for vpn0 in 0..512u32 {
                    let vpn_combined = ((l1_idx as u32) << 10) | vpn0;
                    let eff_ppn = (ppn_hi << 10) | vpn0;
                    vm.cpu.tlb.insert(vpn_combined, asid as u16, eff_ppn, flags);
                }
            }
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

        let _ = vm.step();
        count += 1;
    }

    eprintln!(
        "Final state: PC=0x{:08X} SATP=0x{:08X}",
        vm.cpu.pc, vm.cpu.csr.satp
    );

    // Manual page table walk
    let satp = vm.cpu.csr.satp;
    let pt_ppn = (satp & 0x3FFFFF) as u64;
    let pt_base = pt_ppn << 12;
    let asid = ((satp >> 22) & 0x1FF) as u16;

    let check_vas = [
        0xC003CEE4u32,
        0xC0000000u32,
        0xC0001000u32,
        0xC0001084u32,
        0xC0210F14u32,
        0xC00010EEu32,
    ];

    for va in &check_vas {
        let vpn = va >> 12;
        let l1_idx = (vpn >> 10) as u64;
        let l2_idx = (vpn & 0x3FF) as u64;

        let l1_addr = pt_base + l1_idx * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        let l1_v = l1_pte & 1;
        let l1_is_leaf = (l1_pte & 0xE) != 0;
        let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;

        eprintln!(
            "\nVA=0x{:08X} VPN={} L1[{}] PTE=0x{:08X} V={} leaf={}",
            va, vpn, l1_idx, l1_pte, l1_v, l1_is_leaf
        );

        if l1_v != 0 && !l1_is_leaf {
            let l2_base = (l1_ppn as u64) << 12;
            let l2_addr = l2_base + l2_idx * 4;
            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
            let l2_v = l2_pte & 1;
            let l2_is_leaf = (l2_pte & 0xE) != 0;
            let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
            let offset = va & 0xFFF;

            eprintln!(
                "  L2[{}] PTE=0x{:08X} V={} leaf={}",
                l2_idx, l2_pte, l2_v, l2_is_leaf
            );

            if l2_v != 0 && l2_is_leaf {
                let pa = ((l2_ppn as u64) << 12) | (offset as u64);
                let val = vm.bus.read_word(pa).unwrap_or(0);
                eprintln!("  -> PA=0x{:08X} mem=0x{:08X}", pa, val);
            }
        } else if l1_v != 0 && l1_is_leaf {
            let offset = va & 0x1FFFFF;
            let pa = ((l1_ppn as u64) << 12) | (offset as u64);
            let val = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("  -> Megapage PA=0x{:08X} mem=0x{:08X}", pa, val);
        }

        // TLB lookup
        if let Some((ppn, flags)) = vm.cpu.tlb.lookup(vpn, asid) {
            let offset = va & 0xFFF;
            let pa = ((ppn as u64) << 12) | (offset as u64);
            let val = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!(
                "  TLB: PPN=0x{:06X} flags=0x{:02X} -> PA=0x{:08X} mem=0x{:08X}",
                ppn, flags, pa, val
            );
        }
    }

    // Physical memory check
    eprintln!("\nPhysical memory:");
    for pa in [0u64, 4, 0x10EE, 0x1000, 0x1084] {
        let val = vm.bus.read_word(pa).unwrap_or(0);
        eprintln!("  PA 0x{:08X}: 0x{:08X}", pa, val);
    }
}
