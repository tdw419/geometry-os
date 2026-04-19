// Trace the percpu allocation: what does memblock_alloc_from return?
// Watch for the specific memblock_alloc_try_nid call from setup_per_cpu_areas
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    // The setup_per_cpu_areas function is at 0xC040B830
    // The memblock_alloc_try_nid call is at 0xC040B860-0xC040B864
    // memblock_alloc_try_nid returns VA in a0

    // We want to catch the return from this call
    // jal at 0xC040B864 targets memblock_alloc_try_nid
    // Return address = 0xC040B868

    let target_ret_addr: u32 = 0xC040B868;
    let mut count: u64 = 0;
    let mut found = false;
    let limit = 20_000_000;

    while count < limit {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;

        if pc == target_ret_addr && !found {
            let a0 = vm.cpu.x[10];
            let pa = if a0 >= 0xC0000000 {
                a0 - 0xC0000000
            } else {
                a0
            };
            eprintln!(
                "memblock_alloc_from returned: VA=0x{:08X} PA=0x{:08X} (at count={})",
                a0, pa, count
            );

            // Also dump s1 (saved alloc_info)
            let s1 = vm.cpu.x[9];
            eprintln!("  s1 (alloc_info) = 0x{:08X}", s1);

            found = true;
        }

        // Also catch the return from pcpu_setup_first_chunk
        // setup_per_cpu_areas calls pcpu_setup_first_chunk at 0xC040B8...
        // Let me just look for when we enter pcpu_alloc_first_chunk
        // which is at 0xC040AF6C

        // Catch the call to pcpu_block_update_hint_alloc from pcpu_alloc_first_chunk
        // at 0xC040B0B2 (jal to 0xC0080954)
        if pc == 0xC040B0B6 {
            // Just returned from pcpu_block_update_hint_alloc
            // This is after the first call with (chunk, 0, offset_bits)
            let a0 = vm.cpu.x[10];
            let s4 = vm.cpu.x[20];
            let s5 = vm.cpu.x[21];
            let s1 = vm.cpu.x[9];
            eprintln!("After pcpu_block_update_hint_alloc (first call):");
            eprintln!(
                "  a0=0x{:08X} s4=0x{:08X} s5=0x{:08X} s1=0x{:08X}",
                a0, s4, s5, s1
            );

            // s4 = lw s4, 72(s1) = chunk field at offset 72
            let chunk_pa = (s1 - 0xC0000000) as u64;
            let end_offset = vm.bus.read_word(chunk_pa + 72).unwrap_or(0);
            let start_offset = vm.bus.read_word(chunk_pa + 68).unwrap_or(0);
            eprintln!(
                "  chunk.start_offset=0x{:08X} chunk.end_offset=0x{:08X}",
                start_offset, end_offset
            );

            break;
        }
    }

    if !found {
        eprintln!("Never reached target return address!");
    }
    eprintln!("Final count={}, PC=0x{:08X}", count, vm.cpu.pc);
}
