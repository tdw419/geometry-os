// Diagnostic: check pt_ops value and page table mapping for VA 0xC0800000.
// Fault is at 0x804046C8 (should be 0xC04046C8 = alloc_pte_fixmap).
// pt_ops pointer is at VA 0xC0801000. pt_ops[1] (set_pte) is at VA 0xC0801004.

use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    // Run until second SATP change + 5000 instructions (into paging_init)
    let max_instr: u64 = 186_600;
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut second_satp_done = false;

    while count < max_instr {
        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[diag] SATP changed at count={}: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );
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

                if count > 170_000 {
                    second_satp_done = true;
                    eprintln!("[diag] Second SATP change done at count={}", count);
                }
            }
            last_satp = cur_satp;
        }

        // M-mode trap forwarding
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;
                if mpp != 3 && (vm.cpu.csr.stvec & !0x3) != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = vm.cpu.csr.stvec & !0x3;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let step_result = vm.step();

        if second_satp_done && count == 186553 {
            // Just before the fault. Check pt_ops and page table.
            eprintln!("\n[diag] === State just before fault (count={}) ===", count);
            eprintln!("[diag] PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
            eprintln!("[diag] SATP=0x{:08X}", vm.cpu.csr.satp);

            // Get current page table base
            let satp = vm.cpu.csr.satp;
            let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
            eprintln!("[diag] Page table at PA 0x{:08X}", pg_dir_phys);

            // Check L1[770] (VA 0xC0800000 region where pt_ops lives)
            let l1_770_addr = pg_dir_phys + 770 * 4;
            let l1_770 = vm.bus.read_word(l1_770_addr).unwrap_or(0);
            let l1_770_ppn = l1_770 >> 10;
            let l1_770_flags = l1_770 & 0x3FF;
            let is_leaf = (l1_770 & (1 << 1)) != 0 || (l1_770 & (1 << 3)) != 0;
            eprintln!(
                "[diag] L1[770] (VA 0xC0800000): pte=0x{:08X} ppn=0x{:X} flags=0x{:03X} is_leaf={}",
                l1_770, l1_770_ppn, l1_770_flags, is_leaf
            );

            // Check what PA the MMU would translate VA 0xC0801004 to
            // VA 0xC0801004: VPN1=770, VPN0=1, offset=4
            if is_leaf {
                let pa = (l1_770_ppn as u64) << 22 | 0x1004;
                eprintln!(
                    "[diag] VA 0xC0801004 would map to PA 0x{:08X} (megapage)",
                    pa
                );
            } else {
                // L2 table
                let l2_base = (l1_770_ppn as u64) << 12;
                let l2_entry_addr = l2_base + 1 * 4;
                let l2_entry = vm.bus.read_word(l2_entry_addr).unwrap_or(0);
                let l2_ppn = l2_entry >> 10;
                let pa = (l2_ppn as u64) << 12 | 0x004;
                eprintln!(
                    "[diag] L2[1] at PA 0x{:08X}: pte=0x{:08X} ppn=0x{:X}",
                    l2_entry_addr, l2_entry, l2_ppn
                );
                eprintln!("[diag] VA 0xC0801004 would map to PA 0x{:08X} (L2)", pa);
            }

            // Read pt_ops values directly from physical memory
            // pt_ops is at VA 0xC0801000 -> PA 0x00801000
            let pt_ops_pa = 0x00801000u64;
            let pt_ops_ptr = vm.bus.read_word(pt_ops_pa).unwrap_or(0);
            let pt_ops_get = vm.bus.read_word(pt_ops_pa + 4).unwrap_or(0);
            let pt_ops_set = vm.bus.read_word(pt_ops_pa + 8).unwrap_or(0);
            eprintln!(
                "[diag] pt_ops at PA 0x{:08X}: ptr=0x{:08X} get=0x{:08X} set=0x{:08X}",
                pt_ops_pa, pt_ops_ptr, pt_ops_get, pt_ops_set
            );

            // Check if pt_ops points to a struct, and read that struct
            if pt_ops_ptr != 0 {
                // pt_ops_ptr is a VA. Convert to PA: VA - 0xC0000000
                let pt_ops_struct_pa = if pt_ops_ptr >= 0xC0000000 {
                    (pt_ops_ptr as u64) - 0xC0000000
                } else {
                    pt_ops_ptr as u64
                };
                let struct_get = vm.bus.read_word(pt_ops_struct_pa).unwrap_or(0);
                let struct_set = vm.bus.read_word(pt_ops_struct_pa + 4).unwrap_or(0);
                eprintln!(
                    "[diag] pt_ops->get (at PA 0x{:08X}) = 0x{:08X}",
                    pt_ops_struct_pa, struct_get
                );
                eprintln!(
                    "[diag] pt_ops->set (at PA 0x{:08X}) = 0x{:08X}",
                    pt_ops_struct_pa + 4,
                    struct_set
                );

                // Disassemble what functions these should be
                eprintln!("[diag] pt_ops->get should be early_pg_dir_pte_get or alloc_pte_early");
                eprintln!("[diag] pt_ops->set should be early_pg_dir_pte_set or alloc_pte_fixmap (0xC04046C8)");
            }

            // Check the raw value at the PA where pt_ops[1] should be
            // This is pt_ops_ptr + 4 in physical memory
            let raw_val = vm.bus.read_word(pt_ops_pa + 4).unwrap_or(0);
            eprintln!(
                "[diag] Raw word at pt_ops+4 (PA 0x{:08X}): 0x{:08X}",
                pt_ops_pa + 4,
                raw_val
            );

            // Check if the ELF loader correctly loaded the .data section
            // The third segment has paddr=0x00800000, filesz=0xB7B4
            // pt_ops is at paddr 0x00801000, which is within filesz
            eprintln!("[diag] Checking ELF data integrity at PA 0x00801000-0x00801010:");
            for i in 0..4 {
                let addr = pt_ops_pa + (i * 4);
                let val = vm.bus.read_word(addr).unwrap_or(0);
                eprintln!("[diag]   PA 0x{:08X}: 0x{:08X}", addr, val);
            }

            // Check what the kernel binary has at VA 0xC0801000 (file offset)
            let va_offset = 0xC0801000u64;
            // Find the file offset for this VA
            // Segment 3: vaddr=0xC0800000 paddr=0x00800000 filesz=0xB7B4 offset=0x428000
            let file_offset = 0x428000u64 + (va_offset - 0xC0800000);
            if (va_offset - 0xC0800000) < 0xB7B4 {
                if (file_offset as usize) + 16 <= kernel_image.len() {
                    eprintln!(
                        "[diag] ELF file at offset 0x{:X} (for VA 0x{:08X}):",
                        file_offset, va_offset
                    );
                    for i in 0..4 {
                        let off = file_offset + (i * 4) as u64;
                        let b = &kernel_image[off as usize..(off + 4) as usize];
                        let val = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        eprintln!("[diag]   file[0x{:X}]: 0x{:08X}", off, val);
                    }
                }
            }

            // Also check phys_ram_base. It's a global variable in the kernel.
            // Let's search for it by checking known addresses.
            // In Linux 6.14, phys_ram_base is in .data at some VA.
            // Let's check the kernel symbol table for its address.
            eprintln!("\n[diag] Searching for phys_ram_base in kernel...");
            // nm vmlinux 2>/dev/null | grep phys_ram_base
        }

        match step_result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if count >= 186550 {
                    eprintln!(
                        "[diag] Fault at count={}: PC=0x{:08X} scause=0x{:08X} sepc=0x{:08X}",
                        count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.sepc
                    );
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }

        count += 1;
    }

    eprintln!("[diag] Done at count={}", count);
}
