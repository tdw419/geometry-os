// Quick crash trace with return address
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

    let mut count: u64 = 0;
    while count < 15_500_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        if vm.cpu.csr.scause != 0 {
            let stval = vm.cpu.csr.stval;
            let sp = vm.cpu.x[2];

            // Read saved ra from stack (pcpu_block_update_hint_alloc saves ra at 60(sp))
            let saved_ra = vm.bus.read_word(sp as u64 + 60).unwrap_or(0);

            // Also read the caller's stack frame to get its return address
            // pcpu_alloc_area frame:
            let sp2 = vm.bus.read_word(sp as u64 + 56).unwrap_or(0); // s0 from pcpu_block_update_hint_alloc

            eprintln!("CRASH at {} instructions", count);
            eprintln!("  PC=0x{:08X} stval=0x{:08X}", pc, stval);
            eprintln!("  sp=0x{:08X} ra=0x{:08X} (current)", sp, vm.cpu.x[1]);
            eprintln!(
                "  saved_ra=0x{:08X} (caller of pcpu_block_update_hint_alloc)",
                saved_ra
            );
            eprintln!(
                "  a0=0x{:08X} a1=0x{:08X} a2=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12]
            );
            eprintln!(
                "  s3(chunk)=0x{:08X} s4(block_idx)={}",
                vm.cpu.x[19], vm.cpu.x[20]
            );

            // Try to read the chunk structure
            let chunk = vm.cpu.x[19]; // s3 was chunk from the s3/a0 area
            if chunk >= 0xC0000000 && chunk < 0xD0000000 {
                let pa = (chunk - 0xC0000000) as u64;
                eprintln!("\n  Chunk at PA 0x{:08X}:", pa);
                for off in (0..64).step_by(4) {
                    let v = vm.bus.read_word(pa + off).unwrap_or(0xDEADDEAD);
                    eprintln!("    +{}: 0x{:08X}", off, v);
                }
            }

            // Walk back to find the full call chain
            // Read caller's stack frame
            if saved_ra > 0xC0000000 {
                eprintln!("\n  Caller chain:");
                eprintln!("    pcpu_block_update_hint_alloc <- 0x{:08X}", saved_ra);
            }

            // Try to read pcpu_chunk from s3 (it was in a0 before pcpu_block_update_hint_alloc)
            // Actually s3 = chunk pointer from the register dump earlier
            // Let me read the chunk's base_addr (offset 0) and free_bytes (offset 16)
            let s3 = vm.cpu.x[19];
            if s3 >= 0xC0000000 && s3 < 0xD0000000 {
                let pa = (s3 - 0xC0000000) as u64;
                let base_addr = vm.bus.read_word(pa).unwrap_or(0);
                let start_offset = vm.bus.read_word(pa + 4).unwrap_or(0);
                let end_offset = vm.bus.read_word(pa + 8).unwrap_or(0);
                let nr_pages = vm.bus.read_word(pa + 12).unwrap_or(0);
                let free_bytes = vm.bus.read_word(pa + 28).unwrap_or(0);
                eprintln!("\n  pcpu_chunk fields:");
                eprintln!("    base_addr=0x{:08X}", base_addr);
                eprintln!("    start_offset=0x{:08X}", start_offset);
                eprintln!("    end_offset=0x{:08X}", end_offset);
                eprintln!("    nr_pages={}", nr_pages);
                eprintln!(
                    "    free_bytes=0x{:08X} ({})",
                    free_bytes, free_bytes as i32
                );

                // md_blocks pointer is at offset 56 (after chunk_md which is 32 bytes of pcpu_block_md)
                let md_blocks_ptr = vm.bus.read_word(pa + 56).unwrap_or(0);
                eprintln!("    md_blocks=0x{:08X}", md_blocks_ptr);

                // alloc_map pointer at offset 48
                let alloc_map_ptr = vm.bus.read_word(pa + 48).unwrap_or(0);
                eprintln!("    alloc_map=0x{:08X}", alloc_map_ptr);
            }

            break;
        }
    }
}
