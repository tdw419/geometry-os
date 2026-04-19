use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    // Use boot_linux_setup to inspect state BEFORE running
    let result = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    );

    match result {
        Ok((mut vm, fw_addr, entry, dtb_addr)) => {
            eprintln!("[setup] fw_addr: 0x{:08X}", fw_addr);
            eprintln!("[setup] entry: 0x{:08X}", entry);
            eprintln!("[setup] dtb_addr: 0x{:08X}", dtb_addr);
            eprintln!("[setup] ram_base: 0x{:08X}", vm.bus.mem.ram_base);
            eprintln!("[setup] ram_size: {}", vm.bus.mem.size());
            eprintln!("[setup] pc: 0x{:08X}", vm.cpu.pc);
            eprintln!("[setup] privilege: {:?}", vm.cpu.privilege);
            eprintln!("[setup] satp: 0x{:08X}", vm.cpu.csr.satp);

            // Read first 4 words via bus
            eprintln!("[setup] First 4 words via bus.read_word (no MMU - satap=0):");
            for i in 0..4 {
                let addr = (i * 4) as u64;
                match vm.bus.read_word(addr) {
                    Ok(word) => eprintln!("  PA 0x{:08X}: 0x{:08X}", addr, word),
                    Err(e) => eprintln!("  PA 0x{:08X}: ERR {:?}", addr, e),
                }
            }

            // Read kernel file for comparison
            eprintln!("\n[ref] Kernel file at offset 0x1000:");
            for i in 0..4 {
                let off = 0x1000 + i * 4;
                let b0 = kernel_image[off];
                let b1 = kernel_image[off + 1];
                let b2 = kernel_image[off + 2];
                let b3 = kernel_image[off + 3];
                let word = u32::from_le_bytes([b0, b1, b2, b3]);
                eprintln!("  File[0x{:04X}]: 0x{:08X}", off, word);
            }

            // Check: was the kernel loaded at p_paddr=0 or at p_vaddr=0xC0000000?
            // Read at p_vaddr address to see if loader used vaddr instead
            eprintln!("\n[check] Read at VA 0xC0000000 (kernel vaddr):");
            let addr_c = 0xC0000000u64;
            match vm.bus.read_word(addr_c) {
                Ok(word) => eprintln!("  PA 0x{:08X}: 0x{:08X}", addr_c, word),
                Err(e) => eprintln!("  PA 0x{:08X}: ERR {:?}", addr_c, e),
            }

            // Now run 1000 instructions and check again
            eprintln!("\n--- Running 1000 instructions ---");
            for _ in 0..1000 {
                if vm.bus.sbi.shutdown_requested {
                    break;
                }
                vm.step();
            }
            eprintln!("[post-1K] pc: 0x{:08X}", vm.cpu.pc);
            eprintln!("[post-1K] privilege: {:?}", vm.cpu.privilege);
            eprintln!("[post-1K] satp: 0x{:08X}", vm.cpu.csr.satp);
            eprintln!("[post-1K] ecall_count: {}", vm.cpu.ecall_count);

            // Read first 4 words via bus after running
            eprintln!("[post-1K] First 4 words via bus.read_word:");
            for i in 0..4 {
                let addr = (i * 4) as u64;
                match vm.bus.read_word(addr) {
                    Ok(word) => eprintln!("  PA 0x{:08X}: 0x{:08X}", addr, word),
                    Err(e) => eprintln!("  PA 0x{:08X}: ERR {:?}", addr, e),
                }
            }
        }
        Err(e) => {
            eprintln!("[setup] Error: {:?}", e);
        }
    }
}
