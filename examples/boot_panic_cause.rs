// At the panic point, dump the S-mode CSRs to find the original fault
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
    ).unwrap();

    let fw_addr_u32 = _fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 20_000_000;
    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;
    let mut last_satp = vm.cpu.csr.satp;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        let _ = vm.step();
        count += 1;

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[SATP] 0x{:08X} at count={}", cur_satp, count);
            last_satp = cur_satp;
        }

        // Catch panic entry
        if vm.cpu.pc >= panic_start && vm.cpu.pc < panic_end {
            let a0 = vm.cpu.x[10]; // panic message string
            let a1 = vm.cpu.x[11]; // possibly a format arg

            eprintln!("\n=== PANIC at count={}, PC=0x{:08X} ===", count, vm.cpu.pc);

            // Read panic message
            if a0 >= 0xC0000000 && a0 < 0xE0000000 {
                let pa = (a0 - 0xC0000000) as u64;
                let mut s = String::new();
                for j in 0..500 {
                    if let Ok(byte) = vm.bus.read_byte(pa + j) {
                        if byte == 0 { break; }
                        if byte >= 0x20 && byte < 0x7F { s.push(byte as char); }
                        else { s.push('.'); }
                    }
                }
                eprintln!("  Message: \"{}\"", s);
            }

            // Dump S-mode CSRs (these hold the fault that triggered the panic path)
            eprintln!("  S-mode CSRs (from nested trap):");
            eprintln!("    sepc=0x{:08X}", vm.cpu.csr.sepc);
            eprintln!("    scause=0x{:08X}", vm.cpu.csr.scause);
            eprintln!("    stval=0x{:08X}", vm.cpu.csr.stval);

            // Also dump the S-mode trap frame from the stack (kernel saves registers on stack)
            // The kernel's handle_exception saves registers to pt_regs on the stack
            // For RV32, pt_regs starts at the top of the kernel stack

            // Dump kernel_map
            let km_phys: u64 = 0x00C7A098;
            eprintln!("\n  kernel_map:");
            eprintln!("    phys_addr = 0x{:08X}", vm.bus.read_word(km_phys + 12).unwrap_or(0));
            eprintln!("    va_pa_offset = 0x{:08X}", vm.bus.read_word(km_phys + 20).unwrap_or(0));

            // Page table walk for stval
            let fault_va = vm.cpu.csr.stval;
            let ppn = (cur_satp & 0x3FFFFF) as u64;
            let pg_dir_pa = ppn * 4096;
            let vpn1 = ((fault_va >> 22) & 0x3FF) as u64;
            let vpn0 = ((fault_va >> 12) & 0x3FF) as u64;
            let l1_addr = pg_dir_pa + vpn1 * 4;
            let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
            eprintln!("\n  PT walk for stval=0x{:08X}:", fault_va);
            eprintln!("    L1[{}] = 0x{:08X} (valid={} leaf={} ppn=0x{:06X})",
                vpn1, l1_entry, (l1_entry & 1) != 0, (l1_entry & 0xE) != 0,
                (l1_entry >> 10) & 0x3FFFFF);

            if (l1_entry & 1) != 0 && (l1_entry & 0xE) == 0 {
                let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                let l2_base = l2_ppn * 4096;
                let l2_addr = l2_base + vpn0 * 4;
                let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                eprintln!("    L2[{}] = 0x{:08X} (valid={} ppn=0x{:06X})",
                    vpn0, l2_entry, (l2_entry & 1) != 0,
                    (l2_entry >> 10) & 0x3FFFFF);
            }

            // Dump memblock
            let memblock_pa: u64 = 0x00803448;
            let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
            let res_cnt = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
            eprintln!("\n  memblock: memory.cnt={}, reserved.cnt={}", mem_cnt, res_cnt);
            let mem_regions_ptr = vm.bus.read_word(memblock_pa + 20).unwrap_or(0);
            if mem_regions_ptr >= 0xC0000000 {
                let base_pa = (mem_regions_ptr - 0xC0000000) as u64;
                for ri in 0..mem_cnt.min(8) {
                    let b = vm.bus.read_word(base_pa + (ri as u64) * 12).unwrap_or(0);
                    let s = vm.bus.read_word(base_pa + (ri as u64) * 12 + 4).unwrap_or(0);
                    eprintln!("    memory[{}]: base=0x{:08X} size=0x{:08X}", ri, b, s);
                }
            }

            // Dump registers
            eprintln!("\n  Current registers:");
            for i in 0..32 {
                if vm.cpu.x[i] != 0 {
                    eprintln!("    x[{}] = 0x{:08X}", i, vm.cpu.x[i]);
                }
            }

            break;
        }

        if count % 5_000_000 == 0 {
            eprintln!("[progress] {}M, PC=0x{:08X}", count / 1_000_000, vm.cpu.pc);
        }
    }
}
