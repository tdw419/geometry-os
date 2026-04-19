// Trace pcpu_alloc_first_chunk to find the corruption
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

    // Run to 15.53M (just before setup_per_cpu_areas starts)
    let mut count: u64 = 0;
    while count < 15_530_000 {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        count += 1;
    }
    
    // Now watch for key points in setup_per_cpu_areas
    // 0xC040B830: setup_per_cpu_areas entry
    // 0xC040B868: after memblock_alloc_from returns (fc)
    // 0xC040AF6C: pcpu_alloc_first_chunk entry
    // 0xC040AFC6: chunk allocated, about to set fields
    // 0xC040AFCC: chunk pointer in s1
    
    let mut fc: u32 = 0;
    let mut chunk_calls = 0;
    
    while count < 16_000_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        count += 1;
        
        // Catch fc return
        if pc == 0xC040B868 {
            fc = vm.cpu.x[10];
            eprintln!("[{}] fc = 0x{:08X}", count, fc);
        }
        
        // Catch pcpu_alloc_first_chunk entry
        if pc == 0xC040AF6C {
            let a0 = vm.cpu.x[10]; // tmp_addr
            let a1 = vm.cpu.x[11]; // map_size
            chunk_calls += 1;
            eprintln!("[{}] pcpu_alloc_first_chunk(tmp=0x{:08X}, size=0x{:08X}) call #{}", 
                count, a0, a1, chunk_calls);
        }
        
        // Catch chunk allocation (after memblock_alloc_or_panic for chunk struct)
        // chunk ptr is in a0 at 0xC040AFC6
        if pc == 0xC040AFC6 {
            let chunk = vm.cpu.x[10];
            let s8 = vm.cpu.x[24]; // s8 = aligned base addr
            let s4 = vm.cpu.x[20]; // start_offset
            let s2 = vm.cpu.x[18]; // end_offset
            let s7 = vm.cpu.x[23]; // nr_pages
            eprintln!("[{}] chunk struct at 0x{:08X}", count, chunk);
            eprintln!("  base_addr(s8)=0x{:08X} start_off(s4)=0x{:08X} end_off(s2)=0x{:08X} nr_pages(s7)=0x{:08X}",
                s8, s4, s2, s7);
            
            // Read chunk after init
            let pa = (chunk - 0xC0000000) as u64;
            let ba = vm.bus.read_word(pa + 48).unwrap();
            eprintln!("  base_addr @ +48 = 0x{:08X}", ba);
        }
        
        // Catch calls to pcpu_block_update_hint_alloc from pcpu_alloc_first_chunk
        if pc == 0xC040B0B4 {
            // jal is at 0xC040B0B2, return at 0xC040B0B6
            // Before the call, a0=chunk, a1=0, a2=offset_bits
            let a0 = vm.cpu.x[10];
            let a1 = vm.cpu.x[11];
            let a2 = vm.cpu.x[12];
            eprintln!("[{}] pcpu_block_update_hint_alloc(chunk=0x{:08X}, bit_off={}, bits={})", 
                count, a0, a1, a2);
        }
        
        // Also check for the second call at 0xC040B148 area
        // Let me catch all calls to pcpu_block_update_hint_alloc
        // It's at 0xC0080954
        if pc == 0xC0080954 {
            let a0 = vm.cpu.x[10];
            let a1 = vm.cpu.x[11];
            let a2 = vm.cpu.x[12];
            let ra = vm.cpu.x[1];
            eprintln!("[{}] >> pcpu_block_update_hint_alloc(chunk=0x{:08X}, bit_off=0x{:08X}, bits=0x{:08X}) from 0x{:08X}",
                count, a0, a1, a2, ra);
        }
        
        if vm.bus.sbi.shutdown_requested { break; }
    }
}
