// Diagnose the percpu crash: capture page table state and memblock state
// at the point of the crash.
//
// The kernel panics in setup_per_cpu_areas() -> pcpu_block_update_hint_alloc
// with stval=0xD77BA368 (load fault). We need to understand:
// 1. What page table entry (if any) covers VA 0xD77BA368?
// 2. What did memblock allocate for the percpu chunk?
// 3. What's in the pcpu metadata structures?

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256, // 256MB RAM (enough for 512MB test)
        "console=ttyS0 earlycon=sbi loglevel=8",
    ).unwrap();

    let fw_addr_u32 = _fw_addr as u32;
    let crash_va: u32 = 0xD77BA368; // The stval from the percpu crash
    let crash_pa: u32 = crash_va - 0xC0000000; // Expected PA = 0x177BA368
    let crash_vpn1 = (crash_va >> 22) & 0x3FF; // L1 index
    let crash_vpn0 = (crash_va >> 12) & 0x3FF; // L2 index

    println!("[diag] Target VA: 0x{:08X}, expected PA: 0x{:08X}", crash_va, crash_pa);
    println!("[diag] VPN1 (L1 index): {}, VPN0 (L2 index): {}", crash_vpn1, crash_vpn0);

    let mut count: u64 = 0;
    let max: u64 = 20_000_000;
    let mut last_ecall_count: u64 = 0;
    let mut fault_captured = false;
    let mut satp_changes = 0u32;
    let mut last_satp = vm.cpu.csr.satp;

    // Panic range (from boot_nosmp.rs)
    let panic_start: u32 = 0xC000252E;
    let panic_end: u32 = 0xC00027A4;

    while count < max {
        let pc = vm.cpu.pc;

        let result = vm.step();
        count += 1;

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_changes += 1;
            if satp_changes <= 5 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_pa = (ppn as u64) * 4096;
                eprintln!("[diag] SATP changed to 0x{:08X} (pg_dir PA 0x{:08X}) at count={}", 
                    cur_satp, pg_dir_pa, count);

                // Dump L1 entry for our crash VA
                let l1_addr = pg_dir_pa + (crash_vpn1 as u64) * 4;
                let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                eprintln!("[diag]   L1[{}] at PA 0x{:08X} = 0x{:08X} (valid={} leaf={})", 
                    crash_vpn1, l1_addr, l1_entry,
                    (l1_entry & 1) != 0,
                    (l1_entry & 0xE) != 0);

                // Also dump L1 entries around the kernel linear mapping
                eprintln!("[diag]   L1[768..800] (kernel linear mapping):");
                for i in (768..800).step_by(4) {
                    let addrs: Vec<u64> = (i..i+4).map(|j| pg_dir_pa + (j as u64) * 4).collect();
                    let entries: Vec<u32> = addrs.iter().map(|&a| vm.bus.read_word(a).unwrap_or(0)).collect();
                    eprintln!("[diag]     [{}..{}] = {:08X} {:08X} {:08X} {:08X}", 
                        i, i+3, entries[0], entries[1], entries[2], entries[3]);
                }

                // Dump wider range to cover the crash VA's L1 entry
                if crash_vpn1 >= 800 {
                    eprintln!("[diag]   L1[800..896] (extended linear mapping):");
                    for i in (800..896).step_by(4) {
                        let addrs: Vec<u64> = (i..i+4).map(|j| pg_dir_pa + (j as u64) * 4).collect();
                        let entries: Vec<u32> = addrs.iter().map(|&a| vm.bus.read_word(a).unwrap_or(0)).collect();
                        eprintln!("[diag]     [{}..{}] = {:08X} {:08X} {:08X} {:08X}", 
                            i, i+3, entries[0], entries[1], entries[2], entries[3]);
                    }
                }
            }
            last_satp = cur_satp;
        }

        // Track ECALLs
        if vm.cpu.ecall_count > last_ecall_count {
            last_ecall_count = vm.cpu.ecall_count;
            let a7 = vm.cpu.x[17]; // extension ID
            let a6 = vm.cpu.x[16]; // function ID
            let a0 = vm.cpu.x[10];
            if last_ecall_count <= 30 {
                let ext_name = match a7 {
                    0x10 => "BASE",
                    0x54494D45 => "TIMER",
                    0x735049 => "IPI",
                    0x52464E43 => "RFNC",
                    0x53525354 => "SRST",
                    0x4442434E => "DBCN",
                    0x48534F => "HSM",
                    _ => "???",
                };
                eprintln!("[ecall] #{} at count={}: ext=0x{:08X}({}) func={} a0=0x{:08X}", 
                    last_ecall_count, count, a7, ext_name, a6, a0);
            }
        }

        // Capture the first load fault at VA 0xD77BA368
        if !fault_captured && vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let mtval = vm.cpu.csr.mtval;
            let mepc = vm.cpu.csr.mepc;
            let cause_code = mcause & !(1u32 << 31);

            // Check for load/store/access fault (5, 7, 13, 15)
            if cause_code == 5 || cause_code == 7 || cause_code == 13 || cause_code == 15 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if u64::from(mtval) == crash_va as u64 || (mtval >= 0xD7000000u32 && mtval < 0xD8000000u32) {
                    fault_captured = true;
                    eprintln!("\n=== FAULT CAPTURED at count={} ===", count);
                    eprintln!("  mcause=0x{:08X} (code={}), mtval=0x{:08X}, mepc=0x{:08X}", 
                        mcause, cause_code, mtval, mepc);
                    eprintln!("  mpp={}, satp=0x{:08X}", mpp, vm.cpu.csr.satp);

                    // Dump registers
                    for i in 0..32 {
                        if vm.cpu.x[i] != 0 {
                            eprintln!("  x[{}] = 0x{:08X}", i, vm.cpu.x[i]);
                        }
                    }

                    // Dump the faulting instruction
                    let fault_pa = if mepc >= 0xC0000000 { mepc - 0xC0000000 } else { mepc };
                    let inst = vm.bus.read_word(fault_pa as u64).unwrap_or(0);
                    eprintln!("  Faulting instruction at PA 0x{:08X}: 0x{:08X}", fault_pa, inst);

                    // Dump page table state for the fault address
                    let satp = vm.cpu.csr.satp;
                    let ppn = satp & 0x3FFFFF;
                    let pg_dir_pa = (ppn as u64) * 4096;
                    let l1_idx = ((mtval as u32) >> 22) & 0x3FF;
                    let l1_addr = pg_dir_pa + (l1_idx as u64) * 4;
                    let l1_entry = vm.bus.read_word(l1_addr).unwrap_or(0);
                    eprintln!("\n  === PAGE TABLE WALK for VA 0x{:08X} ===", mtval as u32);
                    eprintln!("  L1[{}] at PA 0x{:08X} = 0x{:08X}", l1_idx, l1_addr, l1_entry);
                    eprintln!("    valid={} leaf={} ppn={}", 
                        (l1_entry & 1) != 0,
                        (l1_entry & 0xE) != 0,
                        (l1_entry >> 10) & 0x3FFFFF);

                    if (l1_entry & 1) != 0 && (l1_entry & 0xE) == 0 {
                        // Non-leaf, walk L2
                        let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
                        let l2_base = l2_ppn * 4096;
                        let l2_idx = ((mtval as u32) >> 12) & 0x3FF;
                        let l2_addr = l2_base + (l2_idx as u64) * 4;
                        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
                        eprintln!("  L2[{}] at PA 0x{:08X} = 0x{:08X}", l2_idx, l2_addr, l2_entry);
                        eprintln!("    valid={} leaf={} ppn={}", 
                            (l2_entry & 1) != 0,
                            (l2_entry & 0xE) != 0,
                            (l2_entry >> 10) & 0x3FFFFF);

                        // Dump surrounding L2 entries
                        for i in (l2_idx as i64 - 2).max(0)..=(l2_idx as i64 + 2).min(1023) {
                            let addr = l2_base + (i as u64) * 4;
                            let entry = vm.bus.read_word(addr).unwrap_or(0);
                            if i as u32 != l2_idx {
                                eprintln!("    L2[{}] = 0x{:08X} (valid={})", i, entry, (entry & 1) != 0);
                            }
                        }
                    }

                    // Dump memblock state
                    eprintln!("\n  === MEMBLOCK STATE ===");
                    let memblock_pa: u64 = 0x00803448;
                    let mem_cnt = vm.bus.read_word(memblock_pa + 8).unwrap_or(0);
                    let res_cnt = vm.bus.read_word(memblock_pa + 28).unwrap_or(0);
                    eprintln!("  memory.cnt={}, reserved.cnt={}", mem_cnt, res_cnt);

                    // Dump memory regions with 12-byte stride (correct for RV32)
                    let mem_regions_ptr = vm.bus.read_word(memblock_pa + 20).unwrap_or(0);
                    if mem_regions_ptr >= 0xC0000000 {
                        let mem_regions_pa = (mem_regions_ptr - 0xC0000000) as u64;
                        for ri in 0..mem_cnt.min(16) {
                            let base = vm.bus.read_word(mem_regions_pa + (ri as u64) * 12).unwrap_or(0);
                            let size = vm.bus.read_word(mem_regions_pa + (ri as u64) * 12 + 4).unwrap_or(0);
                            let flags = vm.bus.read_word(mem_regions_pa + (ri as u64) * 12 + 8).unwrap_or(0);
                            eprintln!("  memory[{}]: base=0x{:08X} size=0x{:08X} flags=0x{:08X}", ri, base, size, flags);
                        }
                    }

                    // Dump reserved regions with 12-byte stride
                    let res_regions_ptr = vm.bus.read_word(memblock_pa + 40).unwrap_or(0);
                    if res_regions_ptr >= 0xC0000000 {
                        let res_regions_pa = (res_regions_ptr - 0xC0000000) as u64;
                        for ri in 0..res_cnt.min(16) {
                            let base = vm.bus.read_word(res_regions_pa + (ri as u64) * 12).unwrap_or(0);
                            let size = vm.bus.read_word(res_regions_pa + (ri as u64) * 12 + 4).unwrap_or(0);
                            let flags = vm.bus.read_word(res_regions_pa + (ri as u64) * 12 + 8).unwrap_or(0);
                            eprintln!("  reserved[{}]: base=0x{:08X} size=0x{:08X} flags=0x{:08X}", ri, base, size, flags);
                        }
                    }

                    // Also check what physical memory contains at the crash PA
                    eprintln!("\n  === PHYSICAL MEMORY AT CRASH PA ===");
                    let crash_phys = crash_pa;
                    for off in (0..64).step_by(4) {
                        let val = vm.bus.read_word((crash_phys as u64) + off).unwrap_or(0);
                        if val != 0 || off < 16 {
                            eprintln!("  PA 0x{:08X}+{} = 0x{:08X}", crash_phys, off, val);
                        }
                    }

                    // Check kernel_map state
                    eprintln!("\n  === KERNEL_MAP ===");
                    let km_phys: u64 = 0x00C7A098;
                    for (name, offset) in [("page_offset", 0), ("virt_addr", 4), ("virt_offset", 8), 
                        ("phys_addr", 12), ("size", 16), ("va_pa_offset", 20), ("va_kernel_pa_offset", 24)] {
                        let val = vm.bus.read_word(km_phys + offset).unwrap_or(0);
                        eprintln!("  kernel_map.{} (PA 0x{:08X}) = 0x{:08X}", name, km_phys + offset, val);
                    }

                    // Check phys_ram_base
                    let prb = vm.bus.read_word(0x00C7A0B4).unwrap_or(0);
                    eprintln!("  phys_ram_base (PA 0x00C7A0B4) = 0x{:08X}", prb);

                    break; // Stop after capturing the fault
                }
            }
        }

        // Check for panic
        if pc >= panic_start && pc < panic_end {
            let a0 = vm.cpu.x[10];
            if a0 >= 0xC0000000 && a0 < 0xE0000000 {
                let pa = (a0 - 0xC0000000) as u64;
                let mut s = String::new();
                for j in 0..300 {
                    if let Ok(byte) = vm.bus.read_byte(pa + j) {
                        if byte == 0 { break; }
                        if byte >= 0x20 && byte < 0x7F { s.push(byte as char); } else { s.push('.'); }
                    }
                }
                eprintln!("[PANIC] at count={}, PC=0x{:08X}: \"{}\"", count, pc, s);
            }
            break;
        }

        // Progress
        if count % 5_000_000 == 0 {
            eprintln!("[progress] {}M instr, PC=0x{:08X}, ECALLs={}, satp=0x{:08X}",
                count / 1_000_000, vm.cpu.pc, last_ecall_count, vm.cpu.csr.satp);
        }
    }

    println!("\n[done] {} instructions, fault_captured={}", count, fault_captured);
}
