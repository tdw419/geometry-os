/// Check the page table entry for the stack area (VA 0xC1401F0C).
/// Also check what's at the stack address.

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

    // Run to 177562 (just before the lw ra,12(sp))
    for _ in 0..177562 {
        if vm.bus.sbi.shutdown_requested { break; }
        vm.step();
    }

    let sp = vm.cpu.x[2];
    let ra_addr = sp + 12;
    eprintln!("SP=0x{:08X}, RA loaded from VA 0x{:08X}", sp, ra_addr);

    // Check the page table for this VA
    let satp = vm.cpu.csr.satp;
    let satp_ppn = (satp & 0x3FFFFF) as u64;
    let root = satp_ppn << 12;

    let vpn1 = ((ra_addr >> 22) & 0x3FF) as u64;
    let vpn0 = ((ra_addr >> 12) & 0x3FF) as u64;
    let offset = (ra_addr & 0xFFF) as u64;

    let l1_pte = vm.bus.read_word(root + vpn1 * 4).unwrap_or(0);
    eprintln!("SATP=0x{:08X}, root=0x{:08X}", satp, root);
    eprintln!("L1[{}] = 0x{:08X}", vpn1, l1_pte);

    let l1_ppn_raw = ((l1_pte & 0xFFFF_FC00) >> 10) as u32;
    let l1_rwx = (l1_pte >> 1) & 7;
    let page_offset_ppn: u32 = 0xC000_0000 >> 12;
    let l1_ppn = if vm.bus.virtual_satp_fixup && l1_ppn_raw >= page_offset_ppn {
        l1_ppn_raw - page_offset_ppn
    } else {
        l1_ppn_raw
    };
    eprintln!("L1 PPN raw=0x{:06X} fixed=0x{:06X} rwx={}", l1_ppn_raw, l1_ppn, l1_rwx);

    if l1_rwx == 7 {
        // Megapage
        let ppn_hi = (l1_ppn >> 10) & 0xFFF;
        let pa = ((ppn_hi as u64) << 22) | (vpn0 << 12) | offset;
        eprintln!("Megapage: ppn_hi=0x{:03X} PA=0x{:08X}", ppn_hi, pa);
        let val = vm.bus.read_word(pa).unwrap_or(0);
        eprintln!("Value at PA 0x{:08X}: 0x{:08X}", pa, val);
    } else {
        // L2
        let l2_base = (l1_ppn as u64) << 12;
        let l2_addr = l2_base + vpn0 * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        let l2_ppn_raw = ((l2_pte & 0xFFFF_FC00) >> 10) as u32;
        let l2_ppn = if vm.bus.virtual_satp_fixup && l2_ppn_raw >= page_offset_ppn {
            l2_ppn_raw - page_offset_ppn
        } else {
            l2_ppn_raw
        };
        eprintln!("L2 at PA 0x{:08X}: PTE=0x{:08X} PPN raw=0x{:06X} fixed=0x{:06X}",
            l2_addr, l2_pte, l2_ppn_raw, l2_ppn);
        if l2_pte & 1 != 0 {
            let pa = ((l2_ppn as u64) << 12) | offset;
            eprintln!("PA=0x{:08X}", pa);
            let val = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("Value at PA 0x{:08X}: 0x{:08X}", pa, val);
        }
    }

    // Also dump a few words around SP+12
    eprintln!("\nStack dump (VA 0x{:08X} - 0x{:08X}):", sp, sp + 32);
    for i in 0..8u32 {
        let addr = sp + i * 4;
        // Compute PA manually
        let v1 = ((addr >> 22) & 0x3FF) as u64;
        let v0 = ((addr >> 12) & 0x3FF) as u64;
        let off = (addr & 0xFFF) as u64;
        let l1 = vm.bus.read_word(root + v1 * 4).unwrap_or(0);
        let l1p = ((l1 & 0xFFFF_FC00) >> 10) as u32;
        let l1r = (l1 >> 1) & 7;
        let l1f = if vm.bus.virtual_satp_fixup && l1p >= page_offset_ppn { l1p - page_offset_ppn } else { l1p };
        let pa = if l1r == 7 {
            (((l1f >> 10) & 0xFFF) as u64) << 22 | (v0 << 12) | off
        } else {
            let l2b = (l1f as u64) << 12;
            let l2 = vm.bus.read_word(l2b + v0 * 4).unwrap_or(0);
            let l2p = ((l2 & 0xFFFF_FC00) >> 10) as u32;
            let l2f = if vm.bus.virtual_satp_fixup && l2p >= page_offset_ppn { l2p - page_offset_ppn } else { l2p };
            ((l2f as u64) << 12) | off
        };
        let val = vm.bus.read_word(pa).unwrap_or(0);
        let marker = if i * 4 == 12 { " <-- RA saved here" } else { "" };
        eprintln!("  [SP+{:2}] VA=0x{:08X} PA=0x{:08X}: 0x{:08X}{}", i * 4, addr, pa, val, marker);
    }
}
