/// Run until 177500 instructions, then trace 100 steps with MMU translation logging.
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

    // Run to 177500
    for _ in 0..177500 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        vm.step();
    }

    eprintln!(
        "At count=177500: PC=0x{:08X} priv={:?} satp=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );

    // Now manually trace: translate VA through MMU and read the instruction
    for i in 0..100 {
        let pc = vm.cpu.pc;
        let satp = vm.cpu.csr.satp;
        let priv_level = vm.cpu.privilege;

        // Manual MMU translation
        let vpn1 = ((pc >> 22) & 0x3FF) as u64;
        let vpn0 = ((pc >> 12) & 0x3FF) as u64;
        let offset = (pc & 0xFFF) as u64;
        let satp_ppn = (satp & 0x3FFFFF) as u64;
        let root_phys = satp_ppn << 12;

        let l1_pte = vm.bus.read_word(root_phys + vpn1 * 4).unwrap_or(0);
        let l1_v = l1_pte & 1;
        let l1_rwx = (l1_pte >> 1) & 7;
        let l1_ppn_raw = ((l1_pte & 0xFFFF_FC00) >> 10) as u32;

        // Apply fixup
        let page_offset_ppn: u32 = 0xC000_0000 >> 12;
        let l1_ppn = if vm.bus.auto_pte_fixup && l1_ppn_raw >= page_offset_ppn {
            l1_ppn_raw - page_offset_ppn
        } else {
            l1_ppn_raw
        };

        let mut computed_pa: u64 = 0;
        let mut pa_desc = String::new();

        if l1_v != 0 && l1_rwx == 7 {
            // Megapage
            let ppn_hi = (l1_ppn >> 10) & 0xFFF;
            computed_pa = ((ppn_hi as u64) << 22) | (vpn0 << 12) | offset;
            pa_desc = format!("megapage ppn_hi=0x{:03X}", ppn_hi);
        } else if l1_v != 0 {
            // L2
            let l2_base = (l1_ppn as u64) << 12;
            let l2_pte = vm.bus.read_word(l2_base + vpn0 * 4).unwrap_or(0);
            let l2_v = l2_pte & 1;
            let l2_ppn_raw = ((l2_pte & 0xFFFF_FC00) >> 10) as u32;
            let l2_ppn = if vm.bus.auto_pte_fixup && l2_ppn_raw >= page_offset_ppn {
                l2_ppn_raw - page_offset_ppn
            } else {
                l2_ppn_raw
            };
            if l2_v != 0 {
                computed_pa = ((l2_ppn as u64) << 12) | offset;
                pa_desc = format!("L2 ppn=0x{:06X}", l2_ppn);
            } else {
                pa_desc = format!("L2 INVALID v={} pte=0x{:08X}", l2_v, l2_pte);
            }
        } else {
            pa_desc = format!("L1 INVALID v={} rwx={}", l1_v, l1_rwx);
        }

        let word_at_pa = vm.bus.read_word(computed_pa).unwrap_or(0);

        // Actually step
        vm.step();
        let next_pc = vm.cpu.pc;

        // Only print if something interesting happens
        if word_at_pa == 0
            || computed_pa > 0x100_000_000
            || !pa_desc.contains("megapage") && !pa_desc.contains("L2 ppn")
        {
            eprintln!("[{}] PC=0x{:08X} priv={:?} SATP=0x{:08X} -> {} PA=0x{:08X} word=0x{:08X} next_pc=0x{:08X}",
                177500 + i, pc, priv_level, satp, pa_desc, computed_pa, word_at_pa, next_pc);
        }
    }
}
