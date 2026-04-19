//! Check DTB parsing state during boot.
//! cargo run --example check_dtb_parse

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    // Use boot_linux_setup to get the VM without running
    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi",
    )
    .expect("setup failed");

    eprintln!("DTB at PA: 0x{:08X}", dtb_addr);
    eprintln!(
        "DTB VA (expected): 0x{:08X}",
        dtb_addr.wrapping_add(0xC0000000)
    );

    // Check DTB magic at physical address
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!(
        "DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)",
        dtb_addr, magic
    );

    // Check _dtb_early_va and _dtb_early_pa
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!(
        "_dtb_early_va: 0x{:08X} (expect 0x{:08X})",
        dtb_early_va,
        dtb_addr.wrapping_add(0xC0000000)
    );
    eprintln!(
        "_dtb_early_pa: 0x{:08X} (expect 0x{:08X})",
        dtb_early_pa, dtb_addr
    );

    // Run for 500K instructions (after setup_vm, before DTB parsing completes)
    use geometry_os::riscv::cpu::StepResult;
    let mut count: u64 = 0;
    let max = 500_000u64;
    while count < max {
        // Tick CLINT
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    eprintln!("\nAfter 500K instructions:");
    eprintln!("PC: 0x{:08X}, Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);

    // Check DTB pointers again
    let dtb_early_va2 = vm.bus.read_word(0x00801008).unwrap_or(0);
    let dtb_early_pa2 = vm.bus.read_word(0x0080100C).unwrap_or(0);
    eprintln!("_dtb_early_va: 0x{:08X}", dtb_early_va2);
    eprintln!("_dtb_early_pa: 0x{:08X}", dtb_early_pa2);

    // Check phys_ram_base
    let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!("phys_ram_base: 0x{:08X}", prb);

    // Check memblock at correct offsets
    // struct memblock { bottom_up(4), current_limit(4), memory(20), reserved(20) }
    // memory.cnt at offset 8, memory.regions at offset 20
    // reserved.cnt at offset 28, reserved.regions at offset 40
    let mb = 0x00803448u64;
    let mem_cnt = vm.bus.read_word(mb + 8).unwrap_or(0);
    let mem_max = vm.bus.read_word(mb + 12).unwrap_or(0);
    let mem_regions = vm.bus.read_word(mb + 20).unwrap_or(0);
    let res_cnt = vm.bus.read_word(mb + 28).unwrap_or(0);
    let res_regions = vm.bus.read_word(mb + 40).unwrap_or(0);
    eprintln!(
        "memblock memory: cnt={} max={} regions_ptr=0x{:08X}",
        mem_cnt, mem_max, mem_regions
    );
    eprintln!(
        "memblock reserved: cnt={} regions_ptr=0x{:08X}",
        res_cnt, res_regions
    );

    // Read first few memory regions
    if mem_cnt > 0 && mem_regions >= 0xC0000000 {
        let regions_pa = (mem_regions - 0xC0000000) as u64;
        for i in 0..mem_cnt.min(4) {
            let base = vm.bus.read_word(regions_pa + (i * 8) as u64).unwrap_or(0);
            let size = vm
                .bus
                .read_word(regions_pa + (i * 8 + 4) as u64)
                .unwrap_or(0);
            eprintln!(
                "  memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)",
                i,
                base,
                size,
                size / (1024 * 1024)
            );
        }
    } else {
        eprintln!(
            "  No memory regions (cnt={} regions_ptr=0x{:08X})",
            mem_cnt, mem_regions
        );
    }

    // Read first few reserved regions
    if res_cnt > 0 && res_regions >= 0xC0000000 {
        let regions_pa = (res_regions - 0xC0000000) as u64;
        for i in 0..res_cnt.min(4) {
            let base = vm.bus.read_word(regions_pa + (i * 8) as u64).unwrap_or(0);
            let size = vm
                .bus
                .read_word(regions_pa + (i * 8 + 4) as u64)
                .unwrap_or(0);
            eprintln!(
                "  reserved[{}]: base=0x{:08X} size=0x{:08X} ({}KB)",
                i,
                base,
                size,
                size / 1024
            );
        }
    }

    // Try reading DTB at the VA through the MMU
    // Read DTB magic via VA (using the kernel's page tables)
    let dtb_va = dtb_early_va2;
    if dtb_va >= 0xC0000000 {
        let dtb_pa_check = (dtb_va - 0xC0000000) as u64;
        let magic_via_pa = vm.bus.read_word(dtb_pa_check).unwrap_or(0);
        eprintln!("DTB magic via PA: 0x{:08X}", magic_via_pa);
    }

    // Check max_mapnr
    let max_mapnr = vm.bus.read_word(0xC148247C - 0xC0000000).unwrap_or(0);
    eprintln!("max_mapnr: {}", max_mapnr);

    // Run for 2M more instructions
    eprintln!("\nRunning 2M more instructions...");
    while count < 2_500_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    eprintln!("After 2.5M instructions:");
    eprintln!("PC: 0x{:08X}, Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);
    let prb2 = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!("phys_ram_base: 0x{:08X}", prb2);
    let mem_cnt2 = vm.bus.read_word(mb + 8).unwrap_or(0);
    let mem_regions2 = vm.bus.read_word(mb + 20).unwrap_or(0);
    eprintln!(
        "memblock memory: cnt={} regions_ptr=0x{:08X}",
        mem_cnt2, mem_regions2
    );
    if mem_cnt2 > 0 && mem_regions2 >= 0xC0000000 {
        let regions_pa = (mem_regions2 - 0xC0000000) as u64;
        for i in 0..mem_cnt2.min(4) {
            let base = vm.bus.read_word(regions_pa + (i * 8) as u64).unwrap_or(0);
            let size = vm
                .bus
                .read_word(regions_pa + (i * 8 + 4) as u64)
                .unwrap_or(0);
            eprintln!(
                "  memory[{}]: base=0x{:08X} size=0x{:08X} ({}MB)",
                i,
                base,
                size,
                size / (1024 * 1024)
            );
        }
    }
    let max_mapnr2 = vm.bus.read_word(0xC148247C - 0xC0000000).unwrap_or(0);
    eprintln!("max_mapnr: {}", max_mapnr2);

    // Check SBI calls
    eprintln!(
        "SBI console output: {} bytes",
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s: String = vm
            .bus
            .sbi
            .console_output
            .iter()
            .map(|&b| b as char)
            .collect();
        eprintln!("SBI output: {}", s);
    }
}
