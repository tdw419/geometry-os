/// Diagnostic: Capture the panic message by watching for PC entering panic().
/// panic() is at 0xC000252E in the kernel.
/// The first arg (a0) is the format string pointer, saved to s2.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel = std::fs::read(kernel_path).expect("kernel not found");
    let initramfs = std::fs::read(initramfs_path).expect("initramfs not found");

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        128,
        "console=ttyS0 earlycon=sbi panic=5 quiet nosmp",
    )
    .expect("boot setup failed");

    let panic_va: u32 = 0xC000_252E; // panic() entry
    let mut count: u64 = 0;
    let max: u64 = 10_000_000;
    let mut panic_caught = false;
    let mut last_satap: u32 = vm.cpu.csr.satp;

    while count < max && !panic_caught {
        // Watch for entry into panic()
        if vm.cpu.pc >= panic_va && vm.cpu.pc < 0xC000_27A0 {
            if !panic_caught {
                panic_caught = true;
                let a0 = vm.cpu.x[10]; // format string pointer
                let ra = vm.cpu.x[1]; // return address (caller of panic)
                let sp = vm.cpu.x[2];

                eprintln!("[panic-catch] count={} PC=0x{:08X}", count, vm.cpu.pc);
                eprintln!("[panic-catch] a0 (fmt) = 0x{:08X}", a0);
                eprintln!("[panic-catch] a1       = 0x{:08X}", vm.cpu.x[11]);
                eprintln!("[panic-catch] a2       = 0x{:08X}", vm.cpu.x[12]);
                eprintln!("[panic-catch] ra (caller) = 0x{:08X}", ra);
                eprintln!("[panic-catch] sp       = 0x{:08X}", sp);
                eprintln!("[panic-catch] satp     = 0x{:08X}", vm.cpu.csr.satp);

                // Try to read the format string from the kernel's virtual address
                // Convert VA to PA (subtract PAGE_OFFSET)
                let fmt_pa = if a0 >= 0xC000_0000 {
                    Some((a0 - 0xC000_0000) as u64)
                } else {
                    Some(a0 as u64)
                };

                if let Some(pa) = fmt_pa {
                    let mut chars = Vec::new();
                    for i in 0..256 {
                        if let Ok(word) = vm.bus.read_word(pa + i * 4) {
                            // Read 4 bytes (little-endian)
                            for b in 0..4 {
                                let byte = (word >> (b * 8)) & 0xFF;
                                if byte == 0 {
                                    break;
                                }
                                chars.push(char::from_u32(byte).unwrap_or('?'));
                            }
                            if chars.last() == Some(&'\0') {
                                break;
                            }
                        }
                    }
                    let fmt_str: String = chars.iter().collect();
                    eprintln!("[panic-catch] format string: {:?}", fmt_str);
                }

                // Disassemble the caller to understand context
                let caller_pa = if ra >= 0xC000_0000 {
                    Some((ra - 0xC000_0000) as u64)
                } else {
                    Some(ra as u64)
                };
                if let Some(pa) = caller_pa {
                    let mut caller_bytes = Vec::new();
                    for i in -4..16i32 {
                        if let Ok(word) = vm.bus.read_word((pa as i64 + i as i64 * 4) as u64) {
                            caller_bytes.push(word);
                        }
                    }
                    eprintln!(
                        "[panic-catch] caller bytes around RA: {:08X?}",
                        &caller_bytes[..caller_bytes.len().min(20)]
                    );
                }

                // Print last 20 unique PCs before panic
                eprintln!("\n[panic-catch] Done - panic captured.");
                break;
            }
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satap {
            eprintln!(
                "[diag] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satap, cur_satp, count
            );
            last_satap = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    if !panic_caught {
        eprintln!("[panic-catch] No panic detected in {} instructions", count);
        eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);
    }
}
