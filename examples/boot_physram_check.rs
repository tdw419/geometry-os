// Quick check: read phys_ram_base after setup_vm.
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < 185_000 {
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
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

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
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
                    if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
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
        let _ = vm.step();
        count += 1;
    }

    // Read phys_ram_base at VA 0xC0C79EAC -> PA 0x00C79EAC
    let phys_ram_base = vm.bus.read_word(0x00C79EAC).unwrap_or(0xDEAD);
    eprintln!("phys_ram_base (PA 0x00C79EAC) = 0x{:08X}", phys_ram_base);

    // Also check the DTB to verify memory node
    // The DTB was loaded at dtb_addr. Let's parse the memory node.
    eprintln!("DTB addr = 0x{:08X}", dtb_addr);
    // Read first 256 bytes of DTB to find memory node
    let mut dtb_data = Vec::new();
    for i in 0..512 {
        match vm.bus.read_byte(dtb_addr + i as u64) {
            Ok(b) => dtb_data.push(b),
            Err(_) => break,
        }
    }
    // Search for "memory" string in DTB
    let memory_str = b"memory";
    if let Some(pos) = dtb_data.windows(memory_str.len()).position(|w| w == memory_str) {
        eprintln!("Found 'memory' at DTB offset {}", pos);
        // Dump surrounding bytes
        let start = pos.saturating_sub(16);
        let end = (pos + 64).min(dtb_data.len());
        eprintln!("DTB bytes {}-{}:", start, end);
        for i in (start..end).step_by(16) {
            let hex: String = dtb_data[i..std::cmp::min(i+16, end)]
                .iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>()
                .join(" ");
            eprintln!("  {:04X}: {}", i, hex);
        }
    } else {
        eprintln!("'memory' string not found in first 512 bytes of DTB");
    }

    // Check kernel_map struct values
    let km_phys: u64 = 0x00C79E90;
    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    eprintln!("kernel_map: phys_addr=0x{:X} va_pa_offset=0x{:X} va_kernel_pa_offset=0x{:X}",
        km_pa, km_vapo, km_vkpo);

    // Check early_pg_dir_pte_ops struct (at VA 0xC0404AA8 -> PA 0x00404AA8)
    let ops_get = vm.bus.read_word(0x00404AA8).unwrap_or(0);
    let ops_set = vm.bus.read_word(0x00404AAC).unwrap_or(0);
    eprintln!("early_pg_dir_pte_ops at PA 0x00404AA8: get=0x{:08X} set=0x{:08X}", ops_get, ops_set);

    // Check what's in the ELF at that file offset
    // Segment 2: vaddr=0xC0400000 paddr=0x00400000 filesz=0x27572 offset=0x400000
    // VA 0xC0404AA8 -> file offset = 0x400000 + (0xC0404AA8 - 0xC0400000) = 0x400000 + 0x4AA8 = 0x404AA8
    let file_off = 0x404AA8;
    if file_off as usize + 8 <= kernel_image.len() {
        let b = &kernel_image[file_off..file_off+8];
        let v1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let v2 = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        eprintln!("ELF file at offset 0x{:X}: 0x{:08X} 0x{:08X}", file_off, v1, v2);
    }
}
