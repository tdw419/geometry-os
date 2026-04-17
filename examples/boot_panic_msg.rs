// Diagnostic: scan kernel RAM for panic message, trace back from panic() call stack
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        bootargs,
    ).unwrap();
    
    println!("PC: 0x{:08X} SP: 0x{:08X}", vm.cpu.pc, vm.cpu.x[2]);
    
    // panic() saves registers on the stack. Let's dump the stack frame.
    // panic() is at 0xC000252E. The stack should contain saved RA (caller of panic).
    // In RISC-V calling convention, s0 (fp) points to the current frame.
    // Let's look at the stack from SP upward.
    let sp = vm.cpu.x[2] as u64;
    let s0 = vm.cpu.x[8] as u64; // frame pointer
    
    println!("SP: 0x{:08X} s0/fp: 0x{:08X}", sp, s0);
    
    // Dump first 100 words of stack
    println!("\nStack dump (SP to SP+400):");
    for i in 0..100 {
        let addr = sp + (i * 4) as u64;
        let word = vm.bus.read_word(addr).unwrap_or(0);
        if word != 0 {
            // Check if it looks like a kernel VA (in .text or .rodata)
            let note = if word >= 0xC0000000 && word < 0xC0400000 {
                // .text segment
                format!(" [.text] offset 0x{:X}", word - 0xC0000000)
            } else if word >= 0xC0C00000 && word < 0xC1400000 {
                // .rodata segment
                format!(" [.rodata] offset 0x{:X}", word - 0xC0C00000)
            } else if word >= 0xC1400000 && word < 0xC1500000 {
                // .data segment
                format!(" [.data]")
            } else {
                String::new()
            };
            
            // Try to read as string if in .rodata range
            let mut str_note = String::new();
            if word >= 0xC0C00000 && word < 0xC1400000 {
                let mut chars = Vec::new();
                for j in 0..80 {
                    let b = vm.bus.read_byte((word as u64) + j as u64).unwrap_or(0);
                    if b == 0 { break; }
                    if b >= 0x20 && b < 0x7f {
                        chars.push(b as char);
                    } else {
                        chars.push('.');
                    }
                }
                let s: String = chars.iter().collect();
                if s.len() > 3 {
                    str_note = format!(" -> \"{}\"", s);
                }
            }
            
            println!("  SP[{:3}] = 0x{:08X}{}{}{}", i * 4, word, note, str_note, "");
        }
    }
    
    // Also scan the kernel .data/.rodata for common panic strings
    println!("\nSearching for known panic strings in kernel memory...");
    let panic_strings = [
        "BUG: unable to handle",
        "Unable to handle kernel",
        "Kernel panic - not syncing",
        "SBI",
        "earlycon",
        "setup_vm",
        "No mapping",
        "page fault",
        "__pa",
    ];
    
    for ps in &panic_strings {
        let pattern_bytes = ps.as_bytes();
        // Scan .rodata region (VA 0xC0C00000 to VA 0xC1400000, PA 0x00C00000 to 0x01000000)
        let start_pa = 0x00C00000u64;
        let end_pa = 0x01000000u64;
        let mut found = false;
        let mut addr = start_pa;
        while addr < end_pa && !found {
            // Read 4 bytes at a time
            let word = vm.bus.read_word(addr).unwrap_or(0);
            let bytes = word.to_le_bytes();
            for &b in &bytes {
                if b == pattern_bytes[0] {
                    // Check if the full pattern matches
                    let mut match_ok = true;
                    for (j, &pb) in pattern_bytes.iter().enumerate() {
                        let cb = vm.bus.read_byte(addr + j as u64).unwrap_or(0);
                        if cb != pb {
                            match_ok = false;
                            break;
                        }
                    }
                    if match_ok {
                        // Found! Read the full string
                        let va = 0xC0000000u32 + (addr as u32 - 0x00000000);
                        let mut chars = Vec::new();
                        for j in 0..200 {
                            let b = vm.bus.read_byte(addr + j as u64).unwrap_or(0);
                            if b == 0 { break; }
                            if b >= 0x20 && b < 0x7f {
                                chars.push(b as char);
                            } else {
                                break;
                            }
                        }
                        let s: String = chars.iter().collect();
                        println!("  Found at VA ~0x{:08X} (PA 0x{:08X}): \"{}\"", va, addr, s);
                        found = true;
                    }
                }
            }
            addr += 4;
        }
    }
}
