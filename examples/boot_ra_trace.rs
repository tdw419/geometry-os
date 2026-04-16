/// Trace the last 200 steps before PC transitions to low address.
/// Log RA (x1) changes to find where it gets set to 0.

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

    // Run to 177300
    for _ in 0..177300 {
        if vm.bus.sbi.shutdown_requested { break; }
        vm.step();
    }

    eprintln!("Tracing from count=177300...");

    let mut prev_ra: u32 = vm.cpu.x[1];
    let mut prev_pc: u32 = vm.cpu.pc;

    for i in 0..300 {
        let pc = vm.cpu.pc;
        let ra = vm.cpu.x[1];
        
        vm.step();
        let next_pc = vm.cpu.pc;
        let next_ra = vm.cpu.x[1];
        
        // Detect RA changes
        if ra != next_ra && i > 100 {
            eprintln!("[{}] RA changed: 0x{:08X} -> 0x{:08X} at PC=0x{:08X} next=0x{:08X}",
                177300 + i, ra, next_ra, pc, next_pc);
        }
        
        // Detect transition to low address
        if pc >= 0xC0000000 && next_pc < 0xC0000000 {
            eprintln!("\n=== TRANSITION at count={} ===", 177300 + i + 1);
            eprintln!("PC: 0x{:08X} -> 0x{:08X}", pc, next_pc);
            eprintln!("RA=0x{:08X} SP=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]);
            eprintln!("A0=0x{:08X} A1=0x{:08X} A2=0x{:08X} A3=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13]);
            eprintln!("T0=0x{:08X} T1=0x{:08X} T2=0x{:08X} S0=0x{:08X}",
                vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7], vm.cpu.x[8]);
            
            // Disassemble the instruction at PC (the ret)
            // Read the word at the translated PA
            let vpn1 = ((pc >> 22) & 0x3FF) as u64;
            let vpn0 = ((pc >> 12) & 0x3FF) as u64;
            let off = (pc & 0xFFF) as u64;
            let satp_ppn = (vm.cpu.csr.satp & 0x3FFFFF) as u64;
            let root = satp_ppn << 12;
            let l1_pte = vm.bus.read_word(root + vpn1 * 4).unwrap_or(0);
            let l1_ppn_raw = ((l1_pte & 0xFFFF_FC00) >> 10) as u32;
            let page_offset_ppn: u32 = 0xC000_0000 >> 12;
            let l1_ppn = if vm.bus.virtual_satp_fixup && l1_ppn_raw >= page_offset_ppn {
                l1_ppn_raw - page_offset_ppn
            } else {
                l1_ppn_raw
            };
            let l1_rwx = (l1_pte >> 1) & 7;
            let mut pa = 0u64;
            if l1_rwx == 7 {
                let ppn_hi = (l1_ppn >> 10) & 0xFFF;
                pa = ((ppn_hi as u64) << 22) | (vpn0 << 12) | off;
            } else {
                let l2_base = (l1_ppn as u64) << 12;
                let l2_pte = vm.bus.read_word(l2_base + vpn0 * 4).unwrap_or(0);
                let l2_ppn_raw = ((l2_pte & 0xFFFF_FC00) >> 10) as u32;
                let l2_ppn = if vm.bus.virtual_satp_fixup && l2_ppn_raw >= page_offset_ppn {
                    l2_ppn_raw - page_offset_ppn
                } else {
                    l2_ppn_raw
                };
                pa = ((l2_ppn as u64) << 12) | off;
            }
            let word = vm.bus.read_word(pa).unwrap_or(0);
            eprintln!("Instruction at PA 0x{:08X}: 0x{:08X}", pa, word);
            
            // Use objdump to disassemble
            break;
        }
        
        prev_ra = next_ra;
        prev_pc = next_pc;
    }
}
