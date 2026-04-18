// Quick check: what value should va_kernel_pa_offset be?
// The kernel does: pt_ops[4] = va_kernel_pa_offset + function_va
// For alloc_pte_fixmap at 0xC04046C8, we need pt_ops[4] = 0xC04046C8
// So va_kernel_pa_offset must be 0.
//
// Also check what the kernel_map struct looks like in the original (unpatched) kernel.
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    // Use a raw VM without patching to see what setup_vm writes naturally
    let actual_ram_size: usize = 256 * 1024 * 1024;
    let mut vm = RiscvVm::new_with_base(0, actual_ram_size);
    vm.bus.low_addr_identity_map = false;
    vm.bus.auto_pte_fixup = false; // Don't fixup

    // Load kernel at physical addresses
    let load_info = geometry_os::riscv::loader::load_elf(&mut vm.bus, &kernel_image).unwrap();

    // Load initramfs
    if let Some(initrd_data) = initramfs.as_ref() {
        let initrd_addr = ((load_info.highest_addr + 0xFFF) & !0xFFF) as u64;
        for (i, &byte) in initrd_data.iter().enumerate() {
            let addr = initrd_addr + i as u64;
            if vm.bus.write_byte(addr, byte).is_err() {
                break;
            }
        }
    }

    // Generate DTB
    let dtb_config = geometry_os::riscv::dtb::DtbConfig {
        ram_base: 0,
        ram_size: actual_ram_size as u64,
        initrd_start: None,
        initrd_end: None,
        bootargs: bootargs.to_string(),
        ..Default::default()
    };
    let dtb_blob = geometry_os::riscv::dtb::generate_dtb(&dtb_config);
    let dtb_addr = 0x01579000u64;
    for (i, &byte) in dtb_blob.iter().enumerate() {
        let addr = dtb_addr + i as u64;
        if vm.bus.write_byte(addr, byte).is_err() {
            break;
        }
    }

    // Set up firmware
    let fw_addr = 0x00066000u64;
    // Write MRET instruction
    vm.bus.write_word(fw_addr, 0x30200073).ok();

    // Enter S-mode via MRET
    let entry: u32 = load_info.entry;
    vm.cpu.csr.mepc = entry;
    vm.cpu.csr.mstatus = 1u32 << 11; // MPP = S
    vm.cpu.csr.mstatus |= 1 << 7; // MPIE = 1
    let restored = vm
        .cpu
        .csr
        .trap_return(geometry_os::riscv::cpu::Privilege::Machine);
    vm.cpu.pc = vm.cpu.csr.mepc;
    vm.cpu.privilege = restored;
    vm.cpu.x[10] = 0;
    vm.cpu.x[11] = dtb_addr as u32;

    // Run until setup_vm writes kernel_map (around count 175000)
    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let km_phys: u64 = 0x00C79E90;

    while count < 178000 {
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
        let _ = vm.step();

        // Check kernel_map after the sw instructions (around count ~175000-177000)
        if count == 175000 {
            eprintln!("kernel_map BEFORE setup_vm writes (count={}):", count);
            for i in 0..7 {
                let val = vm.bus.read_word(km_phys + (i * 4)).unwrap_or(0);
                eprintln!("  offset {}: 0x{:08X}", i * 4, val);
            }
        }
        if count >= 175500 && count <= 176000 && count % 100 == 0 {
            let pa_val = vm.bus.read_word(km_phys + 12).unwrap_or(0);
            let vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
            let vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
            eprintln!(
                "count={}: phys_addr=0x{:X} vapo=0x{:X} vkpo=0x{:X}",
                count, pa_val, vapo, vkpo
            );
        }

        count += 1;
    }

    eprintln!("\nkernel_map AFTER setup_vm (count={}):", count);
    for i in 0..7 {
        let val = vm.bus.read_word(km_phys + (i * 4)).unwrap_or(0);
        eprintln!("  offset {}: 0x{:08X}", i * 4, val);
    }

    // Check pt_ops
    let pt_ops_pa = 0x00801000u64;
    eprintln!("\npt_ops at PA 0x{:08X}:", pt_ops_pa);
    for i in 0..4 {
        let val = vm.bus.read_word(pt_ops_pa + (i * 4)).unwrap_or(0);
        eprintln!("  offset {}: 0x{:08X}", i * 4, val);
    }
}
