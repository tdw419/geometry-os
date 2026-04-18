//! Check what's at the format string address PA 0x00C5AD80
//! to understand if page tables are overwriting kernel data.
//! Run: cargo run --example boot_data_check

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        128,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .expect("boot setup failed");

    vm.bus.auto_pte_fixup = false;
    let fw_addr_u32 = fw_addr as u32;
    let dtb_va = ((dtb_addr.wrapping_add(0xC0000000)) & 0xFFFFFFFF) as u32;
    let dtb_pa = dtb_addr as u32;
    let mut count: u64 = 0;
    let max_instr = 350_000u64; // Run until after panic at count ~318K
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let time_accel = 1000u64;

    // The format string is at VA 0xC0C5AD80, PA 0x00C5AD80
    // Check what's there initially
    let fmt_pa: u64 = 0x00C5AD80;
    let initial_data: Vec<u8> = (0..64)
        .map(|i| vm.bus.read_byte(fmt_pa + i as u64).unwrap_or(0))
        .collect();
    let initial_str: String = initial_data
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    eprintln!("[check] Format string PA 0x{:08X} BEFORE boot:", fmt_pa);
    eprintln!("[check]   bytes: {:02X?}", &initial_data[..32]);
    eprintln!("[check]   text: \"{}\"", initial_str);

    // Also check the kernel binary directly at that PA
    // The kernel is loaded at paddr, so the file offset = paddr (if loaded at offset 0)
    // Actually we need to check what the ELF loader put at PA 0x00C5AD80
    // Let's just check what's in the vmlinux file at the right offset
    eprintln!("[check]   kernel_phys_end from load_info would be the highest loaded addr");

    while count < max_instr {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // M-mode trap handling (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
            if cause_code == 9 {
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                let result = vm.bus.sbi.handle_ecall(
                    a7,
                    a6,
                    a0,
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((ret_a0, ret_a1)) = result {
                    vm.cpu.x[10] = ret_a0;
                    vm.cpu.x[11] = ret_a1;
                }
            } else if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        for _ in 0..time_accel {
            vm.bus.tick_clint();
        }
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();
        count += 1;

        // Handle S-mode page faults with identity mapping fixup
        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    let fault_addr = vm.cpu.csr.stval;
                    let cause_code = vm.cpu.csr.scause & !(1u32 << 31);
                    let is_pf = cause_code == 12 || cause_code == 13 || cause_code == 15;
                    if is_pf && fault_addr < 0x0200_0000 {
                        let satp = vm.cpu.csr.satp;
                        let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                        if pg_dir_ppn > 0 {
                            let pg_dir_phys = pg_dir_ppn * 4096;
                            let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                            let l1_addr = pg_dir_phys + vpn1 * 4;
                            let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                            if (existing & 1) == 0 {
                                let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                                vm.bus.write_word(l1_addr, pte).ok();
                                vm.cpu.tlb.flush_all();
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[check] SATP: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            for i in 0..64u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (i << 20)).ok();
                }
            }
            for &l1_idx in &[8u32, 48, 64] {
                let addr = pg_dir_phys + (l1_idx as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (l1_idx << 20)).ok();
                }
            }
            for l1_scan in 768..780u32 {
                let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                let is_valid = (entry & 1) != 0;
                let is_non_leaf = is_valid && (entry & 0xE) == 0;
                if is_valid && !is_non_leaf {
                    continue;
                }
                let pa_offset = l1_scan - 768;
                vm.bus
                    .write_word(scan_addr, 0x0000_00CF | (pa_offset << 20))
                    .ok();
            }
            vm.cpu.tlb.flush_all();
            vm.bus.write_word(0x00801008, dtb_va).ok();
            vm.bus.write_word(0x0080100C, dtb_pa).ok();
            last_satp = cur_satp;
        }
    }

    // Check what's at the format string address AFTER boot
    let post_data: Vec<u8> = (0..64)
        .map(|i| vm.bus.read_byte(fmt_pa + i as u64).unwrap_or(0))
        .collect();
    let post_str: String = post_data
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "\n[check] Format string PA 0x{:08X} AFTER {} instructions:",
        fmt_pa, count
    );
    eprintln!("[check]   bytes: {:02X?}", &post_data[..32]);
    eprintln!("[check]   text: \"{}\"", post_str);

    if initial_data != post_data {
        eprintln!("[check] *** DATA CORRUPTED! ***");
        for i in 0..64 {
            if initial_data[i] != post_data[i] {
                eprintln!(
                    "[check]   offset +0x{:02X}: 0x{:02X} -> 0x{:02X}",
                    i, initial_data[i], post_data[i]
                );
            }
        }
    } else {
        eprintln!("[check] Data unchanged - corruption is NOT from page table overwrites");
    }

    // Check the early page table allocations - where does setup_vm put page tables?
    // The trampoline pgdir is at PA 0x01484000, early pgdir at PA 0x00802000
    // Check if any page table writes went to PA 0x00C5AD80's page
    let page_base = 0x00C5A000u64; // 4K page containing the format string
    eprintln!(
        "\n[check] Checking page at PA 0x{:08X} (contains format string):",
        page_base
    );
    // Check if this looks like a page table (4-byte PTE entries)
    let page_words: Vec<u32> = (0..1024)
        .map(|i| vm.bus.read_word(page_base + (i as u64) * 4).unwrap_or(0))
        .collect();
    let non_zero_entries: Vec<(usize, u32)> = page_words
        .iter()
        .enumerate()
        .filter(|(_, &w)| w != 0)
        .map(|(i, &w)| (i, w))
        .collect();
    eprintln!("[check]   Non-zero words: {}/1024", non_zero_entries.len());
    for &(idx, val) in non_zero_entries.iter().take(20) {
        eprintln!(
            "[check]   [0x{:03X}] = 0x{:08X}  V={} RWX={:03b} PPN=0x{:05X}",
            idx * 4,
            val,
            val & 1,
            (val >> 1) & 7,
            val >> 12
        );
    }

    // Check early_alloc start address - the kernel's __pa__ fix sets phys_ram_base=0
    // so memblock_alloc_base starts from PA 0.
    // Check if page tables were allocated at low PAs
    eprintln!("\n[check] Early allocation zones (first 16 pages of PA space):");
    for page_num in 0..16 {
        let pa = (page_num as u64) * 4096;
        let first_word = vm.bus.read_word(pa).unwrap_or(0);
        eprintln!(
            "[check]   PA 0x{:08X} (page {}): first word = 0x{:08X}",
            pa, page_num, first_word
        );
    }
}
