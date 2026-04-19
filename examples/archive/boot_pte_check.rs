// Diagnostic: check page table entries after setup_vm() completes.
// Run: cargo run --example boot_pte_check

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    // Use boot_linux_setup to get the VM without running the loop.
    let (mut vm, _fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        bootargs,
    )
    .expect("boot setup failed");

    // Run until the second SATP change (early_pg_dir).
    // From the boot log, the second change is at ~177334.
    let max_instr: u64 = 180_000;

    // Minimal boot loop - just step, no SBI handling needed for this check.
    use geometry_os::riscv::cpu::StepResult;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_changes: Vec<(u64, u32, u32)> = Vec::new();

    while count < max_instr {
        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes.push((count, last_satp, cur_satp));
            eprintln!(
                "[diag] SATP changed at count={}: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );

            // On second SATP change, inject device mappings (same as boot_linux)
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;

                let device_l1_entries: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                let identity_pte: u32 = 0x0000_00CF;
                for &l1_idx in device_l1_entries {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus.write_word(addr, pte).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        vm.step();
        count += 1;
    }

    eprintln!(
        "[diag] Stopped at count={}, SATP=0x{:08X}, PC=0x{:08X}",
        count, vm.cpu.csr.satp, vm.cpu.pc
    );

    // Now check the page table for excp_vect_table region.
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) * 4096;
    eprintln!("[diag] Page directory at PA 0x{:08X}", pg_dir_phys);

    // Check L1 entries for kernel VA range
    // excp_vect_table is at VA 0xC0C00AA4, VPN1 = 0xC0C00AA4 >> 22 = 771
    let vpn1_exc: u32 = 0xC0C00AA4 >> 22;
    eprintln!("[diag] excp_vect_table VA 0xC0C00AA4, VPN1={}", vpn1_exc);

    // Dump L1[768..780] (kernel VA range)
    eprintln!("[diag] L1 entries for kernel VA range:");
    for i in 768..780 {
        let l1_addr = pg_dir_phys + (i as u64) * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        let v = (l1_pte & 1) != 0;
        let r = (l1_pte >> 1) & 1;
        let w = (l1_pte >> 2) & 1;
        let x = (l1_pte >> 3) & 1;
        let u = (l1_pte >> 4) & 1;
        let ppn1 = (l1_pte >> 20) & 0xFFF;
        let is_leaf = r != 0 || w != 0 || x != 0;

        if v {
            if is_leaf {
                // Megapage: maps VA (i*4MB) to PA (ppn1*4MB)
                let va_start = (i as u64) << 22;
                let pa_start = (ppn1 as u64) << 22;
                eprintln!("  L1[{}] = 0x{:08X} -> megapage VA 0x{:08X} -> PA 0x{:08X} (R={} W={} X={} U={})",
                    i, l1_pte, va_start, pa_start, r, w, x, u);
            } else {
                // Points to L2 page table
                let l2_phys = (l1_pte as u64 & 0xFFFFF000);
                eprintln!(
                    "  L1[{}] = 0x{:08X} -> L2 table at PA 0x{:08X}",
                    i, l1_pte, l2_phys
                );
            }
        } else {
            eprintln!("  L1[{}] = 0x{:08X} -> NOT VALID", i, l1_pte);
        }
    }

    // If L1[771] points to an L2 table, check the relevant L2 entry
    let l1_771 = vm.bus.read_word(pg_dir_phys + 771 * 4).unwrap_or(0);
    let v771 = (l1_771 & 1) != 0;
    let r771 = (l1_771 >> 1) & 1;
    let w771 = (l1_771 >> 2) & 1;
    let x771 = (l1_771 >> 3) & 1;
    let is_leaf_771 = r771 != 0 || w771 != 0 || x771 != 0;

    if v771 && !is_leaf_771 {
        let l2_phys = (l1_771 as u64) & 0xFFFFF000;
        // excp_vect_table VA 0xC0C00AA4 -> VPN2 = (0xC0C00AA4 >> 12) & 0x3FF = 0x0AA4
        let vpn2: u32 = (0xC0C00AA4 >> 12) & 0x3FF;
        eprintln!(
            "[diag] L2 table at PA 0x{:08X}, checking VPN2={} (for excp_vect_table)",
            l2_phys, vpn2
        );
        let l2_addr = l2_phys + (vpn2 as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
        let l2_pa = (l2_ppn as u64) << 12;
        eprintln!(
            "[diag] L2[{}] = 0x{:08X} -> PA 0x{:08X}",
            vpn2, l2_pte, l2_pa
        );

        // Read the value at that physical address
        let value = vm.bus.read_word(l2_pa + (0xAA4 & 0xFFF)).unwrap_or(0);
        eprintln!(
            "[diag] Value at PA 0x{:08X} (excp_vect_table[0]) = 0x{:08X}",
            l2_pa + (0xAA4 & 0xFFF),
            value
        );
        eprintln!("[diag] Expected: 0xC020B7F8 (do_trap_unknown)");

        // Also check excp_vect_table[12] (scause=12, instruction page fault)
        let entry_12 = vm
            .bus
            .read_word(l2_pa + (0xAA4 & 0xFFF) + 12 * 4)
            .unwrap_or(0);
        eprintln!("[diag] excp_vect_table[12] = 0x{:08X}", entry_12);

        // Dump a few surrounding L2 entries
        eprintln!("[diag] L2 entries around VPN2={}:", vpn2);
        for j in (vpn2.saturating_sub(2))..=(vpn2 + 2).min(1023) {
            let addr = l2_phys + (j as u64) * 4;
            let pte = vm.bus.read_word(addr).unwrap_or(0);
            if pte != 0 {
                let p = (pte >> 10) & 0x3FFFFF;
                eprintln!(
                    "  L2[{}] = 0x{:08X} -> PA 0x{:08X}",
                    j,
                    pte,
                    (p as u64) << 12
                );
            }
        }
    }

    // Also check: read directly from PA 0x00C00AA4 (where excp_vect_table should be)
    let direct_value = vm.bus.read_word(0x00C00AA4).unwrap_or(0);
    eprintln!(
        "[diag] Direct read from PA 0x00C00AA4 = 0x{:08X}",
        direct_value
    );
    eprintln!("[diag] Expected: 0xC020B7F8 (do_trap_unknown)");

    // Check kernel_map values
    let km_phys: u64 = 0x00C79E90;
    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    eprintln!(
        "[diag] kernel_map: phys_addr=0x{:X}, va_pa_offset=0x{:X}, va_kernel_pa_offset=0x{:X}",
        km_pa, km_vapo, km_vkpo
    );
}
