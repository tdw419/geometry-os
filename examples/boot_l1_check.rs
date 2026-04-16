/// Diagnostic: Check what L1[775] contains in the kernel page table.
/// The stack is at VA 0xC1401E00 -> VPN1=775 -> needs L1[775] to be mapped.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max = 200_000u64; // Past the SATP changes and first fault
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 1)) | (sie << 1);
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

        // SATP change
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X}", count, last_satp, cur_satp);
            last_satp = cur_satp;
        }

        count += 1;
    }

    // Now we're at count ~178000, right before the fault
    eprintln!("\n[diag] At count={}: PC=0x{:08X} priv={:?} satp=0x{:08X}",
        count, vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    eprintln!("[diag] SP=0x{:08X} RA=0x{:08X} GP=0x{:08X}", 
        vm.cpu.x[2], vm.cpu.x[1], vm.cpu.x[3]);

    // Check the kernel page table at early_pg_dir (SATP PPN=0x802)
    let satp_val = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp_val & 0x3FFFFF) as u64) * 4096;
    eprintln!("[diag] Page dir at PA 0x{:08X}", pg_dir_phys);

    // Check L1[768-795] (kernel VA 0xC0000000 - 0xC07C0000)
    eprintln!("\n[diag] Kernel L1 entries (768-795):");
    for idx in 768..796 {
        let pte = vm.bus.read_word(pg_dir_phys + (idx * 4) as u64).unwrap_or(0);
        if pte != 0 {
            let ppn = ((pte >> 10) & 0xFFF_FFFF) as u32;
            let is_leaf = (pte & 0xE) != 0;
            let va_start = (idx as u32) * 0x400000;
            let va_end = va_start + 0x3FFFFF;
            if is_leaf {
                eprintln!("  L1[{}] = 0x{:08X} -> PA 0x{:08X}-0x{:08X} (VA 0x{:08X}-0x{:08X}) LEAF",
                    idx, pte, ppn << 12, (ppn << 12) + 0x3FFFFF, va_start + 0xC0000000, va_end + 0xC0000000);
            } else {
                eprintln!("  L1[{}] = 0x{:08X} -> L2 at PA 0x{:08X} (VA 0x{:08X}-0x{:08X}) NON-LEAF",
                    idx, pte, ppn << 12, va_start + 0xC0000000, va_end + 0xC0000000);
            }
        }
    }

    // SP is at VA 0xC1401E00 -> VPN1 = 0xC1401E00 >> 22 = 775
    // Check if L1[775] exists
    let sp_vpn1 = (vm.cpu.x[2] >> 22) & 0x3FF;
    let l1_775 = vm.bus.read_word(pg_dir_phys + (sp_vpn1 as u64) * 4).unwrap_or(0);
    eprintln!("\n[diag] SP VA 0x{:08X} -> VPN1={}", vm.cpu.x[2], sp_vpn1);
    eprintln!("[diag] L1[{}] = 0x{:08X} (V={} leaf={})", sp_vpn1, l1_775,
        (l1_775 & 1) != 0, (l1_775 & 0xE) != 0);

    if (l1_775 & 1) != 0 && (l1_775 & 0xE) == 0 {
        // Non-leaf: walk L2
        let l2_ppn = ((l1_775 >> 10) & 0xFFF_FFFF) as u64;
        let l2_base = l2_ppn << 12;
        let sp_vpn0 = (vm.cpu.x[2] >> 12) & 0x3FF;
        let l2_pte = vm.bus.read_word(l2_base + (sp_vpn0 as u64) * 4).unwrap_or(0);
        eprintln!("[diag] L2 at PA 0x{:08X}[{}] = 0x{:08X}", l2_base as u32, sp_vpn0, l2_pte);
        if (l2_pte & 1) != 0 {
            let l2_final_ppn = ((l2_pte >> 10) & 0xFFF_FFFF) as u64;
            eprintln!("[diag] SP maps to PA 0x{:08X}", (l2_final_ppn << 12) as u32);
        }
    }

    // Also check: what's the raw data at the stack address through MMU translation?
    // The stack should have the return address
    eprintln!("\n[diag] Reading stack at SP to SP+128:");
    for off in 0..32 {
        let va = vm.cpu.x[2].wrapping_add(off * 4);
        // Read using the bus (bypassing MMU -- raw physical)
        // We need to translate VA -> PA manually
        let vpn1 = (va >> 22) & 0x3FF;
        let vpn0 = (va >> 12) & 0x3FF;
        let page_offset = va & 0xFFF;
        let l1_pte = vm.bus.read_word(pg_dir_phys + (vpn1 as u64) * 4).unwrap_or(0);
        if (l1_pte & 1) == 0 {
            eprintln!("  SP+{} (VA 0x{:08X}): L1[{}] not mapped", off * 4, va, vpn1);
            continue;
        }
        let is_leaf = (l1_pte & 0xE) != 0;
        if is_leaf {
            let pa = (((l1_pte >> 10) & 0xFFF_FFFF) as u64) << 12 | (page_offset as u64);
            let val = vm.bus.read_word(pa).unwrap_or(0xBAD_BAAD);
            eprintln!("  SP+{} (VA 0x{:08X} -> PA 0x{:08X}): 0x{:08X}", off * 4, va, pa as u32, val);
        } else {
            let l2_ppn = ((l1_pte >> 10) & 0xFFF_FFFF) as u64;
            let l2_addr = (l2_ppn << 12) + (vpn0 as u64) * 4;
            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
            if (l2_pte & 1) == 0 {
                eprintln!("  SP+{} (VA 0x{:08X}): L2[{}] at PA 0x{:08X} not mapped", off * 4, va, vpn0, l2_addr as u32);
                continue;
            }
            let pa = (((l2_pte >> 10) & 0xFFF_FFFF) as u64) << 12 | (page_offset as u64);
            let val = vm.bus.read_word(pa).unwrap_or(0xBAD_BAAD);
            eprintln!("  SP+{} (VA 0x{:08X} -> PA 0x{:08X}): 0x{:08X}", off * 4, va, pa as u32, val);
        }
    }

    // Check if the kernel page table has been fixed or still has virtual PPNs
    // by checking the raw PTE at L1[775]
    // Also check: what does the kernel's own page table creation produce?
    // The kernel's setup_vm creates early_pg_dir at 0x00802000
    // Let's dump raw L1 entries before any fixup by checking the raw memory
    eprintln!("\n[diag] Raw early_pg_dir entries (first 20):");
    for i in 0..20 {
        let pte = vm.bus.read_word(0x00802000 + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            eprintln!("  [{}] = 0x{:08X}", i, pte);
        }
    }
    eprintln!("[diag] Raw early_pg_dir kernel entries (768-785):");
    for i in 768..786 {
        let pte = vm.bus.read_word(0x00802000 + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            eprintln!("  [{}] = 0x{:08X}", i, pte);
        }
    }
}
