/// Trace every step from 177548 to 177570 with full register dump.
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

    // Run to 177548
    for _ in 0..177548 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        vm.step();
    }

    for i in 0..25 {
        let pc = vm.cpu.pc;
        let sp = vm.cpu.x[2];
        let ra = vm.cpu.x[1];
        let s0 = vm.cpu.x[8];
        let a0 = vm.cpu.x[10];
        let a1 = vm.cpu.x[11];

        // Translate PC to PA and read instruction
        let vpn1 = ((pc >> 22) & 0x3FF) as u64;
        let vpn0 = ((pc >> 12) & 0x3FF) as u64;
        let off = (pc & 0xFFF) as u64;
        let satp_ppn = (vm.cpu.csr.satp & 0x3FFFFF) as u64;
        let root = satp_ppn << 12;
        let l1_pte = vm.bus.read_word(root + vpn1 * 4).unwrap_or(0);
        let l1_ppn_raw = ((l1_pte & 0xFFFF_FC00) >> 10) as u32;
        let l1_rwx = (l1_pte >> 1) & 7;
        let page_offset_ppn: u32 = 0xC000_0000 >> 12;
        let l1_ppn = if vm.bus.auto_pte_fixup && l1_ppn_raw >= page_offset_ppn {
            l1_ppn_raw - page_offset_ppn
        } else {
            l1_ppn_raw
        };
        let pa = if l1_rwx == 7 {
            (((l1_ppn >> 10) & 0xFFF) as u64) << 22 | (vpn0 << 12) | off
        } else if l1_pte & 1 != 0 {
            let l2_base = (l1_ppn as u64) << 12;
            let l2_pte = vm.bus.read_word(l2_base + vpn0 * 4).unwrap_or(0);
            let l2_ppn_raw = ((l2_pte & 0xFFFF_FC00) >> 10) as u32;
            let l2_ppn = if vm.bus.auto_pte_fixup && l2_ppn_raw >= page_offset_ppn {
                l2_ppn_raw - page_offset_ppn
            } else {
                l2_ppn_raw
            };
            ((l2_ppn as u64) << 12) | off
        } else {
            pc as u64
        };
        let word = vm.bus.read_word(pa).unwrap_or(0);

        eprintln!("[{}] PC=0x{:08X} PA=0x{:08X} word=0x{:08X} SP=0x{:08X} RA=0x{:08X} S0=0x{:08X} A0=0x{:08X} A1=0x{:08X}",
            177548 + i, pc, pa, word, sp, ra, s0, a0, a1);

        vm.step();
        if vm.bus.sbi.shutdown_requested {
            break;
        }
    }
}
