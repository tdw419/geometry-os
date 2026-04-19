// Read the crash state including stack frame for call chain
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
        if vm.bus.sbi.shutdown_requested { break; }
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
            let sp = vm.cpu.x[2];
            let sp_pa = (sp - 0xC0000000) as u64;
            
            eprintln!("CRASH: PC=0x{:08X} stval=0x{:08X}", pc, vm.cpu.csr.stval);
            eprintln!("sp=0x{:08X}", sp);
            
            // Dump stack frame (64 bytes)
            eprintln!("Stack frame:");
            for off in (0..64).step_by(4) {
                let v = vm.bus.read_word(sp_pa + off).unwrap_or(0xDEAD);
                eprintln!("  sp+{}: 0x{:08X}", off, v);
            }
            
            // The saved ra is at sp+60
            let saved_ra = vm.bus.read_word(sp_pa + 60).unwrap_or(0);
            let saved_s0 = vm.bus.read_word(sp_pa + 56).unwrap_or(0);
            eprintln!("\nsaved ra=0x{:08X} (caller)", saved_ra);
            eprintln!("saved s0=0x{:08X} (prev frame)", saved_s0);
            
            // Read chunk from s3
            let s3 = vm.cpu.x[19];
            let chunk_pa = (s3 - 0xC0000000) as u64;
            eprintln!("\nChunk at 0x{:08X} (PA 0x{:08X}):", s3, chunk_pa);
            // pcpu_chunk layout:
            // 0: list_head next (4 bytes)
            // 4: list_head prev (4 bytes)
            // 8: void *base_addr
            // 12: int start_offset
            // 16: int end_offset
            // 20: int nr_pages
            // 24: int nr_populated
            // 28: int nr_empty_pop_pages
            // 32: size_t free_bytes
            // 36: (padding)
            // 40: bool immutable
            // ...
            // 48: unsigned long *alloc_map
            // 52: unsigned long *bound_map
            // 56: struct pcpu_block_md *md_blocks
            for off in (0..64).step_by(4) {
                let v = vm.bus.read_word(chunk_pa + off).unwrap_or(0xDEAD);
                eprintln!("  +{}: 0x{:08X}", off, v);
            }
            
            // Read pcpu_block_md chunk_md (embedded at offset 24? or 32?)
            // Actually chunk_md is inside the chunk struct. Let me find its offset.
            // struct pcpu_chunk has chunk_md as a field. The md_blocks pointer is separate.
            
            break;
        }
    }
}
