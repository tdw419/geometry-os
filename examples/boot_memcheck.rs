// Check if the kernel can read the DTB and register memory
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        ).unwrap();

    eprintln!("DTB at PA 0x{:08X}", dtb_addr);

    // Verify DTB is readable at physical address
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    eprintln!("DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)", dtb_addr, magic);

    // Also verify via virtual address (PA + 0xC0000000)
    let dtb_va = dtb_addr + 0xC0000000;
    let magic_va = vm.bus.read_word(dtb_va).unwrap_or(0);
    eprintln!("DTB magic at VA 0x{:08X}: 0x{:08X}", dtb_va, magic_va);

    // Find the memory node in DTB by scanning for "memory" string
    let mut mem_node_found = false;
    let mut mem_reg_addr: u64 = 0;
    let mut mem_reg_size: u64 = 0;
    for offset in 0..2048u64 {
        let addr = dtb_addr + offset;
        let b = vm.bus.read_byte(addr).unwrap_or(0);
        if b == b'm' {
            let mut s = String::new();
            for j in 0..20 {
                let c = vm.bus.read_byte(addr + j).unwrap_or(0);
                if c == 0 { break; }
                s.push(c as char);
            }
            if s == "memory" {
                eprintln!("Found 'memory' string at DTB+0x{:X} (PA 0x{:08X})", offset, addr);
                mem_node_found = true;
                // Look for "device_type" nearby (should be "memory")
                for search in 1..200 {
                    let c = vm.bus.read_byte(addr + search).unwrap_or(0);
                    let mut ds = String::new();
                    for j in 0..30 {
                        let cc = vm.bus.read_byte(addr + search + j).unwrap_or(0);
                        if cc == 0 { break; }
                        ds.push(cc as char);
                    }
                    if ds.contains("device_type") || ds.contains("reg") {
                        eprintln!("  Found '{}' at DTB+0x{:X}", ds, offset + search);
                    }
                }
                break;
            }
        }
    }

    // memblock struct at VA 0xC0803448 (PA 0x00803448)
    let memblock_pa = 0x00803448u64;
    let mb_memory_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
    let mb_reserved_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
    eprintln!("\nBefore boot: memblock memory.cnt={} reserved.cnt={}", mb_memory_cnt, mb_reserved_cnt);

    // Run the kernel, watching memblock
    let max = 2_000_000u64;
    let mut count = 0u64;
    let mut last_mb_cnt = 0u32;
    let mut last_satp = vm.cpu.csr.satp;
    let mut satp_changes = 0u32;
    let mut panic_found = false;

    while count < max {
        let result = vm.step();
        count += 1;

        // Check SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            eprintln!("[{}] SATP changed: 0x{:08X} -> 0x{:08X}", count, last_satp, cur_satp);
            last_satp = cur_satp;
        }

        // Check memblock every 1000 instructions
        if count % 1000 == 0 {
            let mb_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
            if mb_cnt != last_mb_cnt {
                eprintln!("[{}] memblock.memory.cnt changed: {} -> {}", count, last_mb_cnt, mb_cnt);
                if mb_cnt > 0 {
                    // Read the first memory region
                    let base = vm.bus.read_word(memblock_pa).unwrap_or(0);
                    let base_hi = vm.bus.read_word(memblock_pa + 4).unwrap_or(0);
                    let size = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
                    let size_hi = vm.bus.read_word(memblock_pa + 12).unwrap_or(0);
                    let full_base = ((base_hi as u64) << 32) | (base as u64);
                    let full_size = ((size_hi as u64) << 32) | (size as u64);
                    eprintln!("  memory[0]: base=0x{:08X} size=0x{:08X} ({}MB)",
                              full_base, full_size, full_size / (1024*1024));
                }
                last_mb_cnt = mb_cnt;
            }
        }

        // Check for panic (PC in panic region)
        if !panic_found {
            let pc = vm.cpu.pc;
            // __panic is at VA 0xC040E5XX area
            if pc >= 0xC040E000 && pc <= 0xC0410000 && count > 100000 {
                eprintln!("\n[{}] *** PANIC at PC=0x{:08X} ***", count, pc);
                // Read format string from a0
                let fmt_va = vm.cpu.x[10] as u64;
                let mut fmt = String::new();
                for i in 0..80 {
                    let b = vm.bus.read_byte(fmt_va + i).unwrap_or(0);
                    if b == 0 { break; }
                    if b >= 0x20 && b < 0x7f {
                        fmt.push(b as char);
                    } else {
                        fmt.push('.');
                    }
                }
                eprintln!("  a0 (fmt): \"{}\"", fmt);

                // Read memblock state
                let mb_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
                let rb_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
                eprintln!("  memblock: memory.cnt={} reserved.cnt={}", mb_cnt, rb_cnt);

                // Check phys_ram_base
                let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
                eprintln!("  phys_ram_base=0x{:08X}", prb);

                // Check initial_boot_params (a1 was saved somewhere)
                // In the kernel, initial_boot_params is a global variable
                let ibp_addr = 0x00804000u64; // try some BSS addresses
                eprintln!("  initial_boot_params: trying to find...");

                panic_found = true;
                break;
            }
        }

        if let StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault = result {
            eprintln!("[{}] FAULT at PC=0x{:08X}", count, vm.cpu.pc);
            break;
        }
    }

    if !panic_found {
        eprintln!("\n[{}] No panic found in {} instructions ({} SATP changes)", count, max, satp_changes);
        let mb_cnt = vm.bus.read_word(memblock_pa + 48).unwrap_or(0);
        let rb_cnt = vm.bus.read_word(memblock_pa + 52).unwrap_or(0);
        eprintln!("Final: memblock memory.cnt={} reserved.cnt={}", mb_cnt, rb_cnt);
    }
}
