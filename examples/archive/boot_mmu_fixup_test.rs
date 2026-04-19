/// Minimal test: check if MMU translate produces correct PA for kernel virtual addresses
/// when using the kernel's own page table (early_pg_dir at PA 0x00802000).
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, _fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    // Run to the second SATP change (kernel switches to early_pg_dir)
    let mut count: u64 = 0;
    let mut satp_changes = 0;
    let target_changes = 2;

    while count < 200_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let old_satp = vm.cpu.csr.satp;
        vm.step();
        count += 1;
        if vm.cpu.csr.satp != old_satp {
            satp_changes += 1;
            eprintln!("[{}] SATP changed to 0x{:08X}", count, vm.cpu.csr.satp);
            if satp_changes >= target_changes {
                break;
            }
        }
    }

    eprintln!("\n=== MMU translation test ===");
    eprintln!("SATP = 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("auto_pte_fixup = {}", vm.bus.auto_pte_fixup);
    eprintln!();

    // Test translation of several kernel virtual addresses
    let test_vas: &[u32] = &[
        0xC0000000, // kernel _start
        0xC000308A, // the ret instruction
        0xC020C710, // in the range that reads as zero
        0xC0001000, // relocate_enable_mmu
        0xC1401F00, // stack pointer (x2)
    ];

    for &va in test_vas {
        // Read through the bus (which goes through MMU when SATP is set)
        let word = vm.bus.read_word(va as u64).unwrap_or(0);

        // Also manually compute what the PA should be
        let vpn1 = ((va >> 22) & 0x3FF) as u64;
        let vpn0 = ((va >> 12) & 0x3FF) as u64;
        let offset = (va & 0xFFF) as u64;
        let satp_ppn = (vm.cpu.csr.satp & 0x3FFFFF) as u64;
        let root_phys = satp_ppn << 12;

        let l1_pte = vm.bus.read_word(root_phys + vpn1 * 4).unwrap_or(0);
        let l1_ppn_raw = ((l1_pte & 0xFFFF_FC00) >> 10) as u32;
        let l1_rwx = (l1_pte >> 1) & 7;
        let l1_v = l1_pte & 1;

        // Apply fixup
        let page_offset_ppn: u32 = 0xC000_0000 >> 12; // 0xC0000
        let l1_ppn_fixed = if vm.bus.auto_pte_fixup && l1_ppn_raw >= page_offset_ppn {
            l1_ppn_raw - page_offset_ppn
        } else {
            l1_ppn_raw
        };

        if l1_v != 0 && l1_rwx == 7 {
            // Megapage
            let pa = ((l1_ppn_fixed as u64) << 22) | (vpn0 << 12) | offset;
            let direct_word = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("VA 0x{:08X} (VPN1={} VPN0={}): L1[{}] PPN 0x{:06X}->0x{:06X} PA=0x{:08X} mmu_read=0x{:08X} direct_read=0x{:08X} match={}",
                va, vpn1, vpn0, vpn1, l1_ppn_raw, l1_ppn_fixed, pa, word, direct_word, word == direct_word);
        } else if l1_v != 0 {
            // L2 pointer
            let l2_base = (l1_ppn_fixed as u64) << 12;
            let l2_pte = vm.bus.read_word(l2_base + vpn0 * 4).unwrap_or(0);
            let l2_ppn_raw = ((l2_pte & 0xFFFF_FC00) >> 10) as u32;
            let l2_ppn_fixed = if vm.bus.auto_pte_fixup && l2_ppn_raw >= page_offset_ppn {
                l2_ppn_raw - page_offset_ppn
            } else {
                l2_ppn_raw
            };
            let pa = ((l2_ppn_fixed as u64) << 12) | offset;
            let direct_word = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("VA 0x{:08X} (VPN1={} VPN0={}): L1[{}] PPN 0x{:06X}->0x{:06X} L2 PPN 0x{:06X}->0x{:06X} PA=0x{:08X} mmu_read=0x{:08X} direct_read=0x{:08X} match={}",
                va, vpn1, vpn0, vpn1, l1_ppn_raw, l1_ppn_fixed, l2_ppn_raw, l2_ppn_fixed, pa, word, direct_word, word == direct_word);
        } else {
            eprintln!(
                "VA 0x{:08X} (VPN1={} VPN0={}): L1[{}] INVALID (V=0)",
                va, vpn1, vpn0, vpn1
            );
        }
    }
}
