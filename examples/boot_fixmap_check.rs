use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 512, bootargs,
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 200_000;
    let mut count: u64 = 0;

    while count < max_count {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;
    }

    // Check page table after 200K instructions
    let satp = vm.cpu.csr.satp;
    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_phys = pg_dir_ppn * 4096;
    eprintln!("SATP=0x{:08X}, pg_dir PA=0x{:08X}", satp, pg_dir_phys);
    
    // Check fixmap VA range: 0x9E000000
    // L1 index = (0x9E000000 >> 22) & 0x3FF = 0x278 = 632
    let fixmap_l1_idx: u32 = (0x9E000000 >> 22) & 0x3FF;
    let fixmap_l1_entry = vm.bus.read_word(pg_dir_phys + fixmap_l1_idx as u64 * 4).unwrap_or(0);
    eprintln!("Fixmap L1[{}] (VA 0x9E000000) = 0x{:08X}", fixmap_l1_idx, fixmap_l1_entry);
    
    // Check some kernel linear mapping entries
    for idx in [768, 769, 770, 771, 772, 773, 774, 775] {
        let pte = vm.bus.read_word(pg_dir_phys + idx as u64 * 4).unwrap_or(0);
        let is_leaf = (pte & 0xE) != 0;
        let is_valid = (pte & 1) != 0;
        let ppn = (pte >> 10) & 0x3FFFFF;
        eprintln!("  L1[{}] = 0x{:08X} valid={} leaf={} ppn=0x{:06X}", idx, pte, is_valid, is_leaf, ppn);
    }
    
    // Check if the trampoline page table maps fixmap
    // trampoline_pg_dir at PA 0x01484000
    let tramp_pt_addr: u64 = 0x01484000;
    let tramp_l1_entry = vm.bus.read_word(tramp_pt_addr + fixmap_l1_idx as u64 * 4).unwrap_or(0);
    eprintln!("Trampoline L1[{}] = 0x{:08X}", fixmap_l1_idx, tramp_l1_entry);
    
    // Also check early_pg_dir at PA 0x00802000
    let early_pt_addr: u64 = 0x00802000;
    let early_l1_entry = vm.bus.read_word(early_pt_addr + fixmap_l1_idx as u64 * 4).unwrap_or(0);
    eprintln!("Early L1[{}] = 0x{:08X}", fixmap_l1_idx, early_l1_entry);
    
    // Check the L2 entries for kernel linear mapping (L1[768] = first 2MB)
    let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
    let l1_768_valid = (l1_768 & 1) != 0;
    let l1_768_leaf = l1_768_valid && (l1_768 & 0xE) != 0;
    if l1_768_valid && !l1_768_leaf {
        let l2_ppn = ((l1_768 >> 10) & 0x3FFFFF) as u64;
        let l2_base = l2_ppn * 4096;
        eprintln!("\nL1[768] -> L2 at PA 0x{:08X}", l2_base);
        // Show first 10 and last 10 L2 entries
        eprintln!("  First 10 L2 entries:");
        for i in 0..10u32 {
            let pte = vm.bus.read_word(l2_base + i as u64 * 4).unwrap_or(0);
            eprintln!("    L2[{}] = 0x{:08X}", i, pte);
        }
        // Check around VA 0xC1483000 (fixmap_pte)
        let fixmap_pte_va: u32 = 0xC1483000;
        let fixmap_vpn0: u32 = (fixmap_pte_va >> 12) & 0x3FF;
        eprintln!("  Around fixmap_pte (VA 0x{:08X}, VPN0={}):", fixmap_pte_va, fixmap_vpn0);
        for i in fixmap_vpn0.saturating_sub(2)..=fixmap_vpn0+2 {
            let pte = vm.bus.read_word(l2_base + i as u64 * 4).unwrap_or(0);
            let marker = if i == fixmap_vpn0 { " <-- fixmap_pte" } else { "" };
            eprintln!("    L2[{}] = 0x{:08X}{}", i, pte, marker);
        }
    }
}
