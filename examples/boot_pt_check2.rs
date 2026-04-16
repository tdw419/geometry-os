use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 512, bootargs,
    ).expect("setup");

    let fw_addr_u32 = fw_addr as u32;
    let mut trampoline_patched = false;
    let mut last_satp: u32 = 0;
    let mut count: u64 = 0;

    // Run to Phase 1 + a few hundred instructions
    while count < 177600 {
        if vm.bus.sbi.shutdown_requested { break; }
        let cur_satp = vm.cpu.csr.satp;
        if !trampoline_patched && vm.cpu.pc == 0x10EE
            && matches!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Supervisor) && cur_satp == 0
        {
            let identity_pte: u32 = 0x0000_00CF;
            let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
            for &idx in l1_entries {
                let pte = identity_pte | (idx << 20);
                vm.bus.write_word(0x0148_4000u64 + (idx * 4) as u64, pte).ok();
                vm.bus.write_word(0x0080_2000u64 + (idx * 4) as u64, pte).ok();
            }
            trampoline_patched = true;
        }
        if trampoline_patched && cur_satp != last_satp && cur_satp != 0 {
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir = (ppn as u64) * 4096;
            let l1_0 = vm.bus.read_word(pg_dir).unwrap_or(0);
            if (l1_0 & 0xCF) != 0xCF {
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[0, 2, 4, 5, 6, 8, 10];
                for &idx in l1_entries {
                    let pte = identity_pte | (idx << 20);
                    vm.bus.write_word(pg_dir + (idx * 4) as u64, pte).ok();
                }
                vm.cpu.tlb.flush_all();
            }
        }
        last_satp = cur_satp;
        if vm.cpu.pc == fw_addr_u32 && matches!(vm.cpu.privilege, geometry_os::riscv::cpu::Privilege::Machine) {
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        let r = vm.step();
        if matches!(r, geometry_os::riscv::cpu::StepResult::Ebreak) { break; }
        count += 1;
    }

    // Now check the page table for the kernel's text region
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_pa = (ppn as u64) * 4096;
    eprintln!("SATP: 0x{:08X}, pg_dir PA: 0x{:08X}", satp, pg_dir_pa);

    // Check L1[0x300] (for VA 0xC0000000-0xC01FFFFF)
    let l1_300 = vm.bus.read_word(pg_dir_pa + (0x300 * 4) as u64).unwrap_or(0);
    eprintln!("L1[0x300] (VA 0xC0000000): 0x{:08X}", l1_300);
    let l1_300_v = l1_300 & 1;
    let l1_300_r = (l1_300 >> 1) & 1;
    let l1_300_w = (l1_300 >> 2) & 1;
    let l1_300_x = (l1_300 >> 3) & 1;
    let l1_300_ppn = (l1_300 >> 10) & 0x3FFFFF;
    eprintln!("  V={} R={} W={} X={} PPN=0x{:06X} -> PA 0x{:08X}", l1_300_v, l1_300_r, l1_300_w, l1_300_x, l1_300_ppn, l1_300_ppn * 4096);

    // Check L2 entries for _printk area (VA 0xC0003000-0xC0003FFF)
    // VPN[1] = 0x300, VPN[0] for 0xC000304C = (0xC000304C >> 12) & 0x3FF = 0x000
    // Actually: 0xC000304C, offset = 0x304C, VPN[0] = 0x304C >> 12 = 3, but that's wrong
    // VPN[0] = (VA >> 12) & 0x3FF = (0xC000304C >> 12) & 0x3FF
    // 0xC000304C >> 12 = 0xC0003, & 0x3FF = 0x003
    if l1_300_v != 0 && l1_300_r == 0 && l1_300_x == 0 {
        // L2 table
        let l2_base = l1_300_ppn as u64 * 4096;
        eprintln!("L2 table at PA 0x{:08X}:", l2_base);
        for i in 0..4 {
            let l2_entry = vm.bus.read_word(l2_base + (i * 4) as u64).unwrap_or(0);
            eprintln!("  L2[{}] = 0x{:08X}", i, l2_entry);
        }
    }

    // Read actual code at PA 0x304C (where VA 0xC000304C should map)
    eprintln!("\nCode at PA 0x304C (VA 0xC000304C):");
    for i in 0..8 {
        let pa = 0x304C + i * 4;
        let word = vm.bus.read_word(pa as u64).unwrap_or(0);
        let file_off = 0x1000 + pa; // p_offset=0x1000, p_paddr=0
        let file_word = if file_off + 4 <= kernel_image.len() {
            u32::from_le_bytes([kernel_image[file_off], kernel_image[file_off+1], kernel_image[file_off+2], kernel_image[file_off+3]])
        } else { 0 };
        let marker = if word != file_word { " *** DIFF" } else { "" };
        eprintln!("  PA 0x{:08X} (VA 0x{:08X}): RAM=0x{:08X} FILE=0x{:08X}{}", pa, pa + 0xC0000000, word, file_word, marker);
    }

    // Also check what the instruction at VA 0xC000304C decodes to
    // We need to read through the MMU, but we can approximate by reading PA
    let word_at_304c = vm.bus.read_word(0x304Cu64).unwrap_or(0);
    eprintln!("\nInstruction at VA 0xC000304C (PA 0x304C): 0x{:08X}", word_at_304c);
    let halfword = (word_at_304c & 0xFFFF) as u16;
    let bits_1_0 = halfword & 3;
    eprintln!("  Halfword: 0x{:04X}, bits[1:0]={:02b}", halfword, bits_1_0);

    // Check stack pointer
    eprintln!("\nSP: 0x{:08X}", vm.cpu.x[2]);
    eprintln!("RA: 0x{:08X}", vm.cpu.x[1]);
}
